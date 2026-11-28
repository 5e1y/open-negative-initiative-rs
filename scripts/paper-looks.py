#!/usr/bin/env python3
"""Bakes every paper's look into its file, the app solving nothing: a paper is three curves from the window's share to each
layer's dye amount, the dyes' 3x3 crosstalk in density, its floor and the light's primaries, then black and white held.

A data-sheet paper: its Status A curves unmixed through its crosstalk, laid under the window as the enlarger laid it, the
bare film on 0.9 Dmax of its neutral and the window's white on D-min + 0.04 (ISO 6846), each layer offset by the filtration
that prints a grey neutral along the window. The Frontier SP-3000: its measured table's neutral line from its black to its
white, greys exactly as it prints them, and a crosstalk fitted on the 1,440 nodes its measurements constrain.

Run: scripts/paper-looks.py, which rewrites each paper file's `look` from its `profile`, or the Frontier's from its table.
"""
import json
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
STOCKS = ROOT / "crates/oni-settings/stocks"
SCANNER = ROOT / "crates/oni-pipeline/resources/scanner"

# The look's nodes over the window's share, past either end by half the window.
NODES, LOW, HIGH = 256, -0.5, 1.5
SHARES = np.linspace(LOW, HIGH, NODES)
# The useful range on a paper's neutral and the window shares a grey is weighed at.
WHITE_OVER_DMIN, BLACK_OF_DMAX, SAMPLES = 0.04, 0.9, 33
SRGB_TO_2020 = [[0.627404, 0.329283, 0.043313], [0.069097, 0.919540, 0.011362], [0.016391, 0.088013, 0.895595]]
IDENTITY = np.eye(3).tolist()


class Sheet:
    """A data sheet's curves as amounts over its D-min, each never falling with exposure."""

    def __init__(self, data):
        self.log = np.array(data["log_exposure"], float)
        densities = np.array(data["density_curves"], float)
        self.crosstalk = np.array(data["crosstalk"], float)
        self.floor = densities[0]
        amounts = (densities - self.floor) @ np.linalg.inv(self.crosstalk).T
        self.curves = np.maximum.accumulate(amounts, axis=0)

    def amounts(self, log):
        return np.stack([np.interp(log[..., c], self.log, self.curves[:, c]) for c in range(3)], -1)

    def density(self, log):
        return self.floor + self.amounts(log) @ self.crosstalk.T

    def useful_range(self, offsets):
        """Green's log exposure at the neutral's white end and black end, the layers at `offsets`."""
        neutral = np.maximum.accumulate(self.density(self.log[:, None] + offsets).mean(1))

        def at(target):
            hit = np.nonzero(neutral >= target)[0]
            i = max(int(hit[0]) if len(hit) else len(neutral) - 1, 1)
            low, high = neutral[i - 1], neutral[i]
            f = np.clip((target - low) / (high - low), 0, 1) if high > low else 1.0
            return self.log[0] + (self.log[1] - self.log[0]) * (i - 1 + f)

        return at(neutral[0] + WHITE_OVER_DMIN), at(BLACK_OF_DMAX * neutral[-1])

    def strays(self, offsets):
        white, black = self.useful_range(offsets)
        light = lambda s: 10.0 ** -self.density(black - s * (black - white) + offsets)
        dark, bright = light(0.0), light(1.0)
        seen = np.cbrt((light(np.linspace(0, 1, SAMPLES)[:, None]) - dark) / (bright - dark))
        return np.stack([seen[:, 0] - seen[:, 1], seen[:, 2] - seen[:, 1]], 1).ravel()

    def placed(self):
        """Red's and blue's offsets over green's that keep a grey neutral along the window, by damped Gauss-Newton."""
        at = lambda u: np.array([u[0], 0.0, u[1]])
        squared = lambda r: float(r @ r)
        u = np.zeros(2)
        for _ in range(40):
            r = self.strays(at(u))
            J = np.stack([(self.strays(at(u + np.eye(2)[j] * 1e-3)) - r) / 1e-3 for j in range(2)], 1)
            step = np.linalg.solve(J.T @ J, J.T @ r)
            step *= min(1.0, *(0.5 / np.abs(step[np.abs(step) > 0])))
            if np.all(np.abs(step) < 1e-6):
                break
            for t in 0.5 ** np.arange(30):
                if squared(self.strays(at(u - t * step))) < squared(r):
                    u = u - t * step
                    break
            else:
                break
        offsets = at(u)
        white, black = self.useful_range(offsets)
        return offsets, black, black - white


def sheet_look(data):
    sheet = Sheet(data)
    offsets, black, latitude = sheet.placed()
    log = black + offsets - latitude * SHARES[:, None]
    return {
        "axis": [LOW, HIGH],
        "curves": [[round(float(v), 7) for v in row] for row in sheet.amounts(log)],
        "crosstalk": sheet.crosstalk.tolist(),
        "floor": sheet.floor.tolist(),
        "primaries": IDENTITY,
        "latitude": round(float(latitude), 7),
        "filtration": [round(float(v), 4) for v in (offsets.max() - offsets) * 100.0],
    }


class Table:
    """The Frontier's measured table: display sRGB code over each channel's density above its base."""

    def __init__(self):
        meta = json.loads((SCANNER / "frontier.json").read_text())
        self.n = meta["render"]["nodes"]
        self.lo, self.hi = meta["render"]["densityRange"]
        self.reach = np.array(meta["render"]["reached"])
        raw = np.fromfile(SCANNER / "frontier.cube", dtype="<f4").reshape(self.n, self.n, self.n, 4)
        self.cube = raw[..., :3].astype(float)

    def code(self, x):
        u = np.clip((x - self.lo) / (self.hi - self.lo), 0, 1) * (self.n - 1)
        i = np.minimum(np.floor(u).astype(int), self.n - 2)
        f = u - i
        out = 0
        for db in (0, 1):
            for dg in (0, 1):
                for dr in (0, 1):
                    w = (f[..., 2] if db else 1 - f[..., 2]) * (f[..., 1] if dg else 1 - f[..., 1]) * (f[..., 0] if dr else 1 - f[..., 0])
                    out = out + w[..., None] * self.cube[i[..., 2] + db, i[..., 1] + dg, i[..., 0] + dr]
        return out

    def constrained(self, x):
        """The nodes every measurement reads: inside the reach, the channels within 0.6 of one another."""
        inside = np.all((x >= self.reach[0] - 1e-9) & (x <= self.reach[1] + 1e-9), -1)
        return inside & (x.max(-1) - x.min(-1) < 0.6)

    def neutral(self, greens):
        """Per green density, the red and blue densities where the table prints red, green and blue alike."""
        rb = np.stack([greens, greens], -1).astype(float)
        for _ in range(60):
            x = np.stack([rb[:, 0], greens, rb[:, 1]], -1)
            c = self.code(x)
            res = np.stack([c[:, 0] - c[:, 1], c[:, 2] - c[:, 1]], -1)
            J = np.zeros((len(greens), 2, 2))
            for j, k in ((0, 0), (1, 2)):
                moved = x.copy()
                moved[:, k] += 1e-3
                cm = self.code(moved)
                J[:, :, j] = (np.stack([cm[:, 0] - cm[:, 1], cm[:, 2] - cm[:, 1]], -1) - res) / 1e-3
            rb -= np.clip(np.linalg.solve(J + 1e-9 * np.eye(2), res[..., None])[..., 0], -0.2, 0.2)
        return rb


def decoded(code):
    code = np.clip(code, 0, 1)
    return np.where(code <= 0.04045, code / 12.92, ((code + 0.055) / 1.055) ** 2.4)


def oklab(lin2020):
    to_709 = np.linalg.inv(np.array(SRGB_TO_2020))
    lin = np.clip(lin2020 @ to_709.T, 0, 1)
    m1 = np.array([[0.4122214708, 0.5363325363, 0.0514459929], [0.2119034982, 0.6806995451, 0.1073969566], [0.0883024619, 0.2817188376, 0.6299787005]])
    m2 = np.array([[0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099], [0.0259040371, 0.7827717662, -0.8086757660]])
    return np.cbrt(lin @ m1.T) @ m2.T


def frontier_look():
    table = Table()
    # Its neutral line between 0.15 D, under which it prints black, and its reach; held outside.
    greens = np.linspace(0.15, table.reach[1][1], 163)
    rb = table.neutral(greens)
    off = rb - greens[:, None]
    grey_code = table.code(np.stack([rb[:, 0], greens, rb[:, 1]], -1)).mean(1)
    white = float(greens[np.argmax(grey_code)])
    to_2020 = np.array(SRGB_TO_2020)

    def densities(s):
        g = np.clip(s, 0, 1) * white
        return np.stack([g[..., 0] + np.interp(g[..., 0], greens, off[:, 0]), g[..., 1], g[..., 2] + np.interp(g[..., 2], greens, off[:, 1])], -1)

    def shares(x):
        out = []
        for c, o in ((0, off[:, 0]), (1, np.zeros(len(greens))), (2, off[:, 1])):
            xs = greens + o
            order = np.argsort(xs)
            out.append(np.interp(x[..., c], xs[order], greens[order]) / white)
        return np.stack(out, -1)

    def ended(light, black, top):
        return np.where(light <= black, 0.0, (light - black) / (top - black))

    grey = -np.log10(np.maximum(decoded(table.code(densities(np.repeat(SHARES[:, None], 3, 1)))).mean(1), 1e-6))
    # A grey never prints darker on a thinner window's share: the table's noise near its black is held.
    grey = np.minimum.accumulate(grey)

    def printed(mix, s):
        curves = np.linalg.solve(mix, np.stack([grey] * 3)).T
        amounts = lambda s: np.stack([np.interp(s[..., c], SHARES, curves[:, c]) for c in range(3)], -1)
        light = lambda a: (10.0 ** -(a @ mix.T)) @ to_2020.T
        black, top = light(amounts(np.zeros(3))), light(amounts(np.ones(3)))
        return ended(light(amounts(s)), black, top), curves

    def measured(x):
        light = lambda x: decoded(table.code(x)) @ to_2020.T
        ends = densities(np.array([[0.0] * 3, [1.0] * 3]))
        return ended(light(x), light(ends[0]), light(ends[1]))

    axis = np.linspace(table.lo, table.hi, table.n)
    nodes = np.stack(np.meshgrid(axis, axis, axis, indexing="ij"), -1).reshape(-1, 3)[:, ::-1]
    nodes = nodes[table.constrained(nodes)]
    s_nodes, want = shares(nodes), oklab(measured(nodes))
    residual = lambda p: (oklab(printed(p.reshape(3, 3), s_nodes)[0]) - want).ravel() * 100
    p, lam = np.eye(3).ravel(), 1.0
    for _ in range(80):
        r = residual(p)
        J = np.stack([(residual(p + np.eye(9)[j] * 1e-4) - r) / 1e-4 for j in range(9)], 1)
        JtJ, Jtr = J.T @ J, J.T @ r
        while lam < 1e8:
            step = np.linalg.solve(JtJ + lam * np.diag(np.diag(JtJ) + 1e-9), Jtr)
            if residual(p - step) @ residual(p - step) < r @ r:
                p, lam = p - step, lam / 3
                break
            lam *= 4
        else:
            break
    mix = p.reshape(3, 3) / p.reshape(3, 3)[1, 1]
    got, curves = printed(mix, s_nodes)
    errors = np.linalg.norm(oklab(got) - want, axis=1) * 100
    figures = {"nodes": int(len(nodes)), "median": round(float(np.median(errors)), 3), "p90": round(float(np.percentile(errors, 90)), 3), "max": round(float(errors.max()), 3)}
    print(f"Frontier: white at {white:.3f} D green, against its table on the measured nodes {figures}")
    look = {
        "axis": [LOW, HIGH],
        "curves": [[round(float(v), 7) for v in row] for row in curves],
        "crosstalk": mix.tolist(),
        "floor": [0.0, 0.0, 0.0],
        "primaries": SRGB_TO_2020,
        "latitude": round(white, 7),
    }
    look["provenance"] = {
        "table": "crates/oni-pipeline/resources/scanner/frontier.cube",
        "baked": "scripts/paper-looks.py",
        "measured": ["the SP-3000's printed colours over the synthetic negatives' densities, frontier.cube"],
        "inferred": ["the crosstalk, fitted on the nodes every measurement reads; Oklab distance x 100 to the table there"],
        "figures": figures,
        "neutral": {
            "white": round(white, 7),
            "green": [round(float(v), 7) for v in greens],
            "red": [round(float(v), 7) for v in rb[:, 0]],
            "blue": [round(float(v), 7) for v in rb[:, 1]],
        },
    }
    return look


def write(path, paper):
    text = json.dumps(paper, indent=1, ensure_ascii=False) + "\n"
    path.write_text(text)


def main():
    for path in sorted(p for p in STOCKS.glob("*.json") if not p.name.startswith(".")):
        paper = json.loads(path.read_text())
        if paper.get("oni") != "paper":
            continue
        if "profile" in paper:
            paper["look"] = sheet_look(paper["profile"]["data"])
            print(f"{paper['name']}: latitude {paper['look']['latitude']:.3f}, filtration {paper['look']['filtration']} CC")
        else:
            paper.pop("frontier", None)
            paper["look"] = frontier_look()
        write(path, paper)


if __name__ == "__main__":
    main()
