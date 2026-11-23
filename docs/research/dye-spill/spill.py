"""The spill a colour negative's dyes leave on a scan's channels once each channel has its own window, per stock.

A stock's dyes (per wavelength, at unit amount) and base, read by a scan's three channels, give the density each
layer adds to each channel about a mid-grey: J. A window per channel divides each channel by its own neutral
slope, so what is left of J is S, rows summing to 1: the share of a channel's windowed density that each layer
brings. Its inverse M is the matrix that would hand each channel back to its own layer, greys untouched.
"""
import json
import re
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[3]
DATASHEET = ROOT / "crates/oni-pipeline/resources/film/datasheet"
WL = np.arange(380.0, 781.0, 5.0)


def cie(name):
    text = (ROOT / "crates/oni-pipeline/src/film/cie.rs").read_text()
    body = re.search(rf"pub const {name}: \[f64; WAVELENGTHS\] = \[(.*?)\];", text, re.S).group(1)
    return np.array([float(v) for v in body.replace("\n", " ").split(",") if v.strip()])


XYZ = np.stack([cie("X"), cie("Y"), cie("Z")])
D50 = cie("D50")


def gaussian(mean, sigma):
    return np.exp(-0.5 * ((WL - mean) / sigma) ** 2)


# A phone-panel or light-table white LED, drawn: a 450 nm pump and its phosphor. Inferred, not measured.
LED = 1.0 * gaussian(450, 10) + 0.75 * gaussian(555, 55) + 0.25 * gaussian(630, 40)


def xy_xyz(x, y):
    return np.array([x / y, 1.0, (1 - x - y) / y])


def rec2020_to_xyz():
    m = np.stack([xy_xyz(0.708, 0.292), xy_xyz(0.170, 0.797), xy_xyz(0.131, 0.046)]).T
    return m * np.linalg.solve(m, xy_xyz(0.3127, 0.3290))


BRADFORD = np.array([[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]])


def scan_matrix(light):
    """ONI's scan matrix (`bake::scan_matrix`): XYZ under the table's light, Bradford to D65, to Rec. 2020."""
    white = XYZ @ light / (XYZ[1] @ light)
    a, b = BRADFORD @ white, BRADFORD @ xy_xyz(0.3127, 0.3290)
    return np.linalg.inv(rec2020_to_xyz()) @ np.linalg.inv(BRADFORD) @ np.diag(b / a) @ BRADFORD


# A DSLR's three channels, drawn as Gaussians near published Bayer peaks (red's short side cut by the IR
# filter's roll-off left out). Inferred: no measured sensitivity of the owner's camera is at hand.
DSLR = np.stack([gaussian(600, 28), gaussian(532, 38), gaussian(462, 28)])
# The same with a red that opens sharply at 585 nm and runs to the IR filter near 670 nm, as many Bayer reds do.
LONG_RED = np.stack([1 / (1 + np.exp(-(WL - 585) / 6)) / (1 + np.exp((WL - 668) / 10)) * (1 - (WL - 585).clip(0) / 260), DSLR[1], DSLR[2]])
# The channels narrowband RGB light gives (three LEDs): what a scan made for separation reads.
NARROW = np.stack([gaussian(660, 8), gaussian(530, 12), gaussian(450, 8)])

# ISO 5-3 Status M, as scripts/datasheet-profiles.py holds it.
STATUS_M = [
    (620.0, [2.109, 4.479, 5.000, 4.899, 4.578, 4.252, 3.875, 3.491, 3.099, 2.687, 2.269, 1.859, 1.449, 1.054, 0.654, 0.254], 0.260, -0.040),
    (470.0, [1.152, 2.207, 3.156, 3.804, 4.272, 4.626, 4.872, 5.000, 4.995, 4.818, 4.458, 3.915, 3.172, 2.239, 1.070], 0.106, -0.120),
    (410.0, [2.103, 4.111, 4.632, 4.871, 5.000, 4.955, 4.743, 4.343, 3.743, 2.990, 1.852], 0.250, -0.220),
]


def status_m():
    out = []
    for first, logs, before, after in STATUS_M:
        nodes = first + 10.0 * np.arange(len(logs))
        v = np.interp(WL, nodes, logs)
        v = np.where(WL < nodes[0], logs[0] + before * (WL - nodes[0]), v)
        v = np.where(WL > nodes[-1], logs[-1] + after * (WL - nodes[-1]), v)
        out.append(10.0 ** (v - 5.0))
    return np.array(out)


def weights(reader, light=D50):
    """The three channels' weights per wavelength (rows), signed where a matrix follows the sensor."""
    if reader == "status_m":
        return status_m()
    if reader == "narrow":
        return NARROW
    if reader == "xyz":
        return XYZ * light
    if reader == "rec2020":
        return scan_matrix(light) @ (XYZ * light)
    if reader in ("dslr", "long_red"):
        return (DSLR if reader == "dslr" else LONG_RED) * light
    if reader in ("dslr2020", "long_red2020"):
        # The camera's matrix as a raw converter fits it: least squares onto the observer, then the scan's.
        sensor = DSLR if reader == "dslr2020" else LONG_RED
        fit = np.linalg.lstsq(sensor.T, XYZ.T, rcond=None)[0].T
        return scan_matrix(light) @ fit @ (sensor * light)
    raise SystemExit(reader)


class Stock:
    def __init__(self, path):
        d = json.loads(Path(path).read_text())
        data = d["data"]
        self.name = d["info"]["stock"]
        nan = lambda v: np.array([[np.nan if x is None else x for x in row] for row in v], dtype=float)
        dyes = nan(data["channel_density"])
        base = nan([[x] for x in data["base_density"]])[:, 0]
        self.ok = ~np.isnan(dyes).any(axis=1) & ~np.isnan(base)
        self.dyes, self.base = dyes[self.ok], base[self.ok]
        le = np.array(data["log_exposure"])
        curves = np.array(data["density_curves"], dtype=float)
        # Each dye at a Status M density of 1 in its own channel, the amounts' unit on every source.
        w = status_m()[:, self.ok]
        scale = np.ones(3)
        for c in range(3):
            for _ in range(60):
                own = -np.log10((w[c] * 10.0 ** -(self.dyes[:, c] * scale[c])).sum() / w[c].sum())
                scale[c] /= own
        self.dyes = self.dyes * scale
        curves = curves / scale
        # The grey a metered exposure lays, and each layer's rise with log exposure about it.
        at = lambda x: np.array([np.interp(x, le, curves[:, c]) for c in range(3)])
        self.grey = at(0.0)
        self.gamma = (at(0.3) - at(-0.3)) / 0.6
        self.le, self.curves = le, curves

    def density(self, amounts, w):
        """Each channel's density of the film holding `amounts` (…, 3), and its change with each dye."""
        w = w[:, self.ok]
        t = 10.0 ** -(np.asarray(amounts) @ self.dyes.T + self.base)
        light = t @ w.T
        slope = np.einsum("...l,cl,lk->...ck", t, w, self.dyes) / light[..., None]
        return -np.log10(light), slope

    def spill(self, w):
        """S: each channel's windowed density shared among the layers about the grey, rows summing to 1."""
        _, j = self.density(self.grey, w)
        return j * self.gamma / (j @ self.gamma)[:, None]

    def matrix(self, w):
        return np.linalg.inv(self.spill(w))


def datasheet(kind="negative"):
    out = []
    for p in sorted(DATASHEET.glob("*.json")):
        d = json.loads(p.read_text())
        if d["info"].get("type") == kind and "channel_density" in d["data"] and len(d["data"]["density_curves"][0]) == 3:
            data = d["data"]
            if any(v is not None for row in data["channel_density"] for v in row):
                out.append(Stock(p))
    return out


FAMILY = {
    "Vision3 (measured dyes)": lambda n: "vision3" in n,
    "Kodak C-41": lambda n: n.startswith("kodak") and "vision3" not in n and "verita" not in n,
    "Fujifilm C-41": lambda n: n.startswith("fujifilm"),
}


def show(m):
    return "  ".join("[" + " ".join(f"{v:+.3f}" for v in row) + "]" for row in m)
