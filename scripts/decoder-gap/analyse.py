# analyse.py <renders> <sidecars>: render_pairs' renders read in Oklab, Rust (LibRaw) against Swift (Apple):
# hue and chroma on neutrals, skin and sky, the blacks, and the gains and clicks that cancel the neutral cast.
import glob
import json
import os
import sys

import numpy as np

# Linear Rec. 2020 to XYZ (D65), then Oklab's XYZ to LMS.
REC2020_XYZ = np.array([[0.636958, 0.144617, 0.168881], [0.262700, 0.677998, 0.059302], [0.000000, 0.028073, 1.060985]])
XYZ_LMS = np.array([[0.8189330101, 0.3618667424, -0.1288597137], [0.0329845436, 0.9293118715, 0.0361456387], [0.0482003018, 0.2643662691, 0.6338517070]])
LMS_LAB = np.array([[0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099], [0.0259040371, 0.7827717662, -0.8086757660]])
GAIN_STEP, CLICK_STEP = 0.05, 2


def load(path):
    raw = np.fromfile(path, dtype=np.float32)
    w, h = raw[:2].view(np.int32)
    return raw[2:].reshape(h, w, 4)[..., :3].reshape(-1, 3).astype(np.float64)


def oklab(rgb):
    lms = np.cbrt(np.clip(rgb, 0, 1) @ REC2020_XYZ.T @ XYZ_LMS.T)
    return lms @ LMS_LAB.T


def lch(lab):
    c = np.hypot(lab[:, 1], lab[:, 2])
    h = np.degrees(np.arctan2(lab[:, 2], lab[:, 1])) % 360
    return lab[:, 0], c, h


def turn(a, b):
    return (a - b + 540) % 360 - 180


def neutral(ls, cs):
    """Greys in the midtones, where neither the clip nor the black point flattens a move."""
    return (cs < 0.025) & (ls > 0.3) & (ls < 0.85)


def metrics(x, s):
    """Hue, chroma and blacks of render x against Swift's s, both Oklab."""
    ls, cs, hs = lch(s)
    lx, cx, hx = lch(x)
    inside = (ls > 0.02) & (ls < 0.98)
    masks = {
        "neutral": neutral(ls, cs),
        "skin": inside & (hs > 35) & (hs < 85) & (cs > 0.03) & (cs < 0.16) & (ls > 0.45) & (ls < 0.9),
        "sky": inside & (hs > 210) & (hs < 275) & (cs > 0.03),
        "colour": inside & (cs > 0.03),
    }
    out = {}
    for key, m in masks.items():
        if m.sum() < 200:
            continue
        d = (x - s)[m]
        out[key] = {"share": float(m.mean()), "da": 100 * float(d[:, 1].mean()), "db": 100 * float(d[:, 2].mean()),
                    "dL": 100 * float(d[:, 0].mean())}
        if key != "neutral":
            out[key]["dh"] = float(np.median(turn(hx[m], hs[m])))
            out[key]["chroma"] = float(cx[m].mean() / cs[m].mean())
    dark = ls <= np.quantile(ls, 0.05)
    out["blacks"] = {
        "dL_darkest5": 100 * float((lx - ls)[dark].mean()),
        "L_darkest5": [float(ls[dark].mean()), float(lx[dark].mean())],
        "crushed": [float((ls < 0.03).mean()), float((lx < 0.03).mean())],
    }
    return out


def frame(renders, name):
    arm = lambda a: os.path.join(renders, f"{name}.{a}.f32")
    s, r = oklab(load(arm("swift"))), oklab(load(arm("rust")))
    out = {"rust": metrics(r, s)}
    for extra in ("rust-swiftsidecar", "rust-gains", "rust-matrix", "fix"):
        if os.path.exists(arm(extra)):
            out[extra] = metrics(oklab(load(arm(extra))), s)
    # The neutral cast moved by each probe, solved for the move that cancels it.
    ls, cs, _ = lch(s)
    n = neutral(ls, cs)
    cast = np.array([(r - s)[n][:, 1].mean(), (r - s)[n][:, 2].mean()])
    for probe, (one, two), step in (("gains", ("dr", "db"), GAIN_STEP), ("clicks", ("dc", "dy"), CLICK_STEP)):
        if not os.path.exists(arm(one)):
            continue
        cols = [(oklab(load(arm(p))) - r)[n][:, 1:].mean(axis=0) / step for p in (one, two)]
        out[probe] = [float(v) for v in np.linalg.solve(np.array(cols).T, -cast)]
        out[probe + "_per_unit"] = [[100 * float(v) for v in col] for col in cols]
    return out


def main():
    renders, sidecars = sys.argv[1], sys.argv[2]
    names = sorted(os.path.basename(p)[: -len(".swift.f32")] for p in glob.glob(os.path.join(renders, "*.swift.f32")))
    result = {}
    for name in names:
        side = json.load(open(os.path.join(sidecars, name + ".json")))["settings"]
        result[name] = frame(renders, name) | {"face": side["face"], "machine": side.get("scannerMachine"), "mode": side["mode"]}
        f = result[name]
        nt = f["rust"].get("neutral", {})
        seen = f.get("fix", f["rust"])
        print(f"{name} {f['face']:7} neutral da {nt.get('da', 0):+.2f} db {nt.get('db', 0):+.2f}"
              f" gains {np.round(f.get('gains', [np.nan] * 2), 3)} clicks {np.round(f.get('clicks', [np.nan] * 2), 1)}"
              f" chroma x{seen.get('colour', {}).get('chroma', np.nan):.3f} blacks dL {seen['blacks']['dL_darkest5']:+.2f}")
    json.dump(result, open(os.path.join(renders, "analysis.json"), "w"), indent=1)


main()
