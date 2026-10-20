# analyse.py <work> <strips>: sensor_colours' renders against the Swift app's own, in Oklab, per camera and face;
# the strips matrix | sensor | Swift per frame; the per-camera fit of each decode onto Apple's.
import glob
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

REC2020_XYZ = np.array([[0.636958, 0.144617, 0.168881], [0.262700, 0.677998, 0.059302], [0.000000, 0.028073, 1.060985]])
XYZ_LMS = np.array([[0.8189330101, 0.3618667424, -0.1288597137], [0.0329845436, 0.9293118715, 0.0361456387], [0.0482003018, 0.2643662691, 0.6338517070]])
LMS_LAB = np.array([[0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099], [0.0259040371, 0.7827717662, -0.8086757660]])
REC2020_SRGB = np.array([[1.660491, -0.587641, -0.072850], [-0.124550, 1.132900, -0.008349], [-0.018150, -0.100579, 1.118730]])
ARMS = ["matrix-swiftsidecar", "matrix-look", "sensor-look", "unbalanced-look", "matrix-prepared", "sensor-prepared",
        "unbalanced-prepared", "matrix-rest", "sensor-rest", "unbalanced-rest"]
SHOWN = [("matrix", "matrix-swiftsidecar"), ("sensor", "sensor-look"), ("swift", None)]


def load(path):
    raw = np.fromfile(path, dtype=np.float32)
    w, h = raw[:2].view(np.int32)
    return raw[2:].reshape(h, w, 4)[..., :3].astype(np.float64)


def oklab(rgb):
    return np.cbrt(np.clip(rgb, 0, 1) @ REC2020_XYZ.T @ XYZ_LMS.T) @ LMS_LAB.T


def lch(lab):
    return lab[:, 0], np.hypot(lab[:, 1], lab[:, 2]), np.degrees(np.arctan2(lab[:, 2], lab[:, 1])) % 360


def turn(a, b):
    return (a - b + 540) % 360 - 180


def srgb_code(x):
    lin = x @ REC2020_SRGB.T
    c = np.clip(lin, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055), lin


def masks(ref):
    ls, cs, hs = lch(ref)
    inside = (ls > 0.02) & (ls < 0.98)
    return {
        "neutral": (cs < 0.025) & (ls > 0.3) & (ls < 0.85),
        "skin": inside & (hs > 35) & (hs < 85) & (cs > 0.03) & (cs < 0.16) & (ls > 0.45) & (ls < 0.9),
        "sky": inside & (hs > 210) & (hs < 275) & (cs > 0.03),
        "foliage": inside & (hs > 100) & (hs < 160) & (cs > 0.03) & (ls > 0.2) & (ls < 0.9),
    }


def compare(x, ref, m):
    """Render x against ref (both Oklab) on the masks laid on the Swift render."""
    lr, cr, hr = lch(ref)
    lx, cx, hx = lch(x)
    out = {}
    for key, k in m.items():
        if k.sum() < 200:
            continue
        d = (x - ref)[k]
        e = {"share": float(k.mean()), "da": 100 * float(d[:, 1].mean()), "db": 100 * float(d[:, 2].mean()), "dL": 100 * float(d[:, 0].mean())}
        if key != "neutral":
            e["dh"] = float(np.median(turn(hx[k], hr[k])))
            e["chroma"] = float(cx[k].mean() / cr[k].mean())
        out[key] = e
    return out


def own(x, lin):
    """What a render holds on its own: blacks, chroma, and what falls outside the display."""
    l, c, _ = lch(x)
    dark = l <= np.quantile(l, 0.05)
    return {
        "L_darkest5": 100 * float(l[dark].mean()),
        "crushed": float((l < 0.03).mean()),
        "chroma": float(c.mean()),
        "a": 100 * float(x[:, 1].mean()),
        "b": 100 * float(x[:, 2].mean()),
        "out_of_srgb": float(((lin < -1e-4) | (lin > 1 + 1e-4)).any(axis=1).mean()),
    }


def strip(path, panels, labels):
    h = min(p.shape[0] for row in panels for p in row)
    w = min(p.shape[1] for row in panels for p in row)
    gap, top = 8, 22
    sheet = Image.new("RGB", (len(labels) * w + (len(labels) - 1) * gap, len(panels) * (h + top)), "white")
    draw = ImageDraw.Draw(sheet)
    for r, row in enumerate(panels):
        for c, p in enumerate(row):
            code, _ = srgb_code(p[:h, :w])
            x, y = c * (w + gap), r * (h + top)
            sheet.paste(Image.fromarray((code * 255).round().astype(np.uint8)), (x, y + top))
            draw.text((x + 4, y + 4), labels[c][r], fill=(0, 0, 0))
    sheet.save(path)


def fit(src, dst):
    """3x3 from src to dst, least squares on the relative error, as decoder_fit."""
    wgt = 1 / np.maximum(dst.mean(axis=1, keepdims=True), 1e-4)
    return np.linalg.lstsq(src * wgt, dst * wgt, rcond=None)[0].T


def blocks(path):
    raw = np.fromfile(path, dtype=np.float32)
    w, h = raw[:2].view(np.int32)
    return raw[2:].reshape(h, w, 4)[..., :3].astype(np.float64)


def camera_fit(work, cam, frames):
    a, m, s = [], [], []
    for f in frames:
        apple = blocks(os.path.join(work, "apple", f + ".apple.f32"))
        mat = blocks(os.path.join(work, "rust", cam, f + ".matrix.blocks.f32"))
        sen = blocks(os.path.join(work, "rust", cam, f + ".sensor.blocks.f32"))
        hh, ww = min(apple.shape[0], mat.shape[0]), min(apple.shape[1], mat.shape[1])
        apple, mat, sen = apple[:hh, :ww].reshape(-1, 3), mat[:hh, :ww].reshape(-1, 3), sen[:hh, :ww].reshape(-1, 3)
        ok = ((apple > 2e-4) & (apple < 0.9)).all(1) & ((mat > 2e-4) & (mat < 0.9)).all(1) & ((sen > 2e-4) & (sen < 0.9)).all(1)
        a.append(apple[ok]), m.append(mat[ok]), s.append(sen[ok])
    a, m, s = np.concatenate(a), np.concatenate(m), np.concatenate(s)
    corr = float(np.corrcoef(a[:, 1], m[:, 1])[0, 1])
    return {"apple": a, "matrix": m, "sensor": s, "M": fit(m, a), "S": fit(s, a), "corr": corr}


def density_gap(x, y):
    """RMS of the per-channel density difference left once each channel's median is taken out: what a
    roll's gains cannot absorb."""
    d = -np.log10(np.clip(x, 1e-5, None)) + np.log10(np.clip(y, 1e-5, None))
    d = d - np.median(d, axis=0)
    return [float(v) for v in np.sqrt((d ** 2).mean(axis=0))]


def main():
    work, strips = sys.argv[1], sys.argv[2]
    os.makedirs(strips, exist_ok=True)
    os.makedirs(os.path.join(strips, "rest"), exist_ok=True)
    result = {}
    names = {"zf": "Zf", "s1r": "S1R"}
    fits = {}
    for cam in ("zf", "s1r"):
        frames = sorted(os.path.basename(p)[: -len(".f32")] for p in glob.glob(os.path.join(work, "swift", cam, "pro", "*.f32")))
        for f in frames:
            rows, rest_rows = [], []
            for face in ("pro", "frontier"):
                swift = load(os.path.join(work, "swift", cam, face, f + ".f32"))
                ref = oklab(swift.reshape(-1, 3))
                m = masks(ref)
                entry = {"swift": own(ref, srgb_code(swift.reshape(-1, 3))[1])}
                arms = {}
                for arm in ARMS:
                    p = os.path.join(work, "rust", cam, f"{f}.{face}.{arm}.f32")
                    x = load(p)
                    hh, ww = min(x.shape[0], swift.shape[0]), min(x.shape[1], swift.shape[1])
                    arms[arm] = x
                    lab = oklab(x[:hh, :ww].reshape(-1, 3))
                    r = ref.reshape(swift.shape)[:hh, :ww].reshape(-1, 3)
                    mm = {k: v.reshape(swift.shape[:2])[:hh, :ww].reshape(-1) for k, v in m.items()}
                    entry[arm] = {"vs_swift": compare(lab, r, mm), "own": own(lab, srgb_code(x.reshape(-1, 3))[1])}
                    if arm != "matrix-swiftsidecar":
                        base = oklab(arms["matrix-swiftsidecar"][:hh, :ww].reshape(-1, 3))
                        entry[arm]["vs_matrix"] = compare(lab, base, mm)
                result[f"{cam}/{f}/{face}"] = entry
                rows.append([arms["matrix-swiftsidecar"], arms["sensor-look"], swift])
                rest_rows.append([arms["matrix-rest"], arms["sensor-rest"], arms["unbalanced-rest"]])
            stem = f"{names[cam]}_{f.rsplit('.', 1)[0]}"
            strip(os.path.join(strips, f"{stem}_matrix-sensor-swift.png"), rows,
                  [[f"matrix (today, Swift sidecar) / {face}" for face in ("Pro", "Frontier")],
                   [f"sensor (roll re-measured, same look) / {face}" for face in ("Pro", "Frontier")],
                   [f"Swift app render / {face}" for face in ("Pro", "Frontier")]])
            strip(os.path.join(strips, "rest", f"{stem}_rest_matrix-sensor-unbalanced.png"), rest_rows,
                  [[f"matrix at rest, unprepared / {face}" for face in ("Pro", "Frontier")],
                   [f"sensor 5000 K diagonal at rest / {face}" for face in ("Pro", "Frontier")],
                   [f"sensor unbalanced at rest / {face}" for face in ("Pro", "Frontier")]])
        fits[cam] = camera_fit(work, cam, frames)
    # Per-camera consistency: one set of negative colours read through each camera's decode.
    colours = np.concatenate([fits["zf"]["apple"], fits["s1r"]["apple"]])
    read = {(cam, mode): colours @ np.linalg.inv(fits[cam][key]).T for cam in fits for mode, key in (("matrix", "M"), ("sensor", "S"))}
    consistency = {
        "fits": {cam: {"M": fits[cam]["M"].round(4).tolist(), "S": fits[cam]["S"].round(4).tolist(), "corr": fits[cam]["corr"],
                       "residual_matrix_vs_apple": density_gap(fits[cam]["matrix"], fits[cam]["apple"]),
                       "residual_sensor_vs_apple": density_gap(fits[cam]["sensor"], fits[cam]["apple"])} for cam in fits},
        "zf_vs_s1r_matrix": density_gap(read[("zf", "matrix")], read[("s1r", "matrix")]),
        "zf_vs_s1r_sensor": density_gap(read[("zf", "sensor")], read[("s1r", "sensor")]),
        "zf_vs_apple_sensor": density_gap(read[("zf", "sensor")], colours),
        "s1r_vs_apple_sensor": density_gap(read[("s1r", "sensor")], colours),
        "zf_vs_apple_matrix": density_gap(read[("zf", "matrix")], colours),
        "s1r_vs_apple_matrix": density_gap(read[("s1r", "matrix")], colours),
    }
    result["consistency"] = consistency
    json.dump(result, open(os.path.join(work, "analysis.json"), "w"), indent=1)
    summarise(result)


def summarise(r):
    def mean(cam, face, arm, path, key, val):
        v = [r[k][arm][path].get(key, {}).get(val) for k in r if k.startswith(cam + "/") and k.endswith("/" + face)]
        v = [x for x in v if x is not None]
        return np.mean(v) if v else float("nan")

    for cam in ("zf", "s1r"):
        for face in ("pro", "frontier"):
            print(f"\n== {cam} {face}: against the Swift app's render (Oklab x100; dh degrees; chroma ratio)")
            for arm in ARMS[:7]:
                n = (mean(cam, face, arm, "vs_swift", "neutral", "da"), mean(cam, face, arm, "vs_swift", "neutral", "db"))
                parts = [f"{arm:22} neutral da {n[0]:+.2f} db {n[1]:+.2f} (|{np.hypot(*n):.2f}|)"]
                for key in ("skin", "sky", "foliage"):
                    parts.append(f"{key} {mean(cam, face, arm, 'vs_swift', key, 'dh'):+5.1f}° x{mean(cam, face, arm, 'vs_swift', key, 'chroma'):.2f}")
                own = [r[k][arm]["own"] for k in r if k.startswith(cam + "/") and k.endswith("/" + face)]
                parts.append(f"darkest5 L {np.mean([o['L_darkest5'] for o in own]):.1f} crushed {100 * np.mean([o['crushed'] for o in own]):.1f}% "
                             f"outside sRGB {100 * np.mean([o['out_of_srgb'] for o in own]):.1f}%")
                print("  " + "  ".join(parts))
            own = [r[k]["swift"] for k in r if k.startswith(cam + "/") and k.endswith("/" + face)]
            print(f"  {'swift':22} darkest5 L {np.mean([o['L_darkest5'] for o in own]):.1f} crushed {100 * np.mean([o['crushed'] for o in own]):.1f}% "
                  f"outside sRGB {100 * np.mean([o['out_of_srgb'] for o in own]):.1f}%")
            for arm in ARMS[7:]:
                own = [r[k][arm]["own"] for k in r if k.startswith(cam + "/") and k.endswith("/" + face)]
                print(f"  {arm:22} at rest: mean a {np.mean([o['a'] for o in own]):+.2f} b {np.mean([o['b'] for o in own]):+.2f} chroma {np.mean([o['chroma'] for o in own]):.3f}")
    print("\n== per-camera consistency: density RMS (R, G, B) left once each channel's median is removed")
    for k, v in r["consistency"].items():
        if k != "fits":
            print(f"  {k:22} {np.round(v, 4)}")
    for cam, f in r["consistency"]["fits"].items():
        print(f"  {cam} corr {f['corr']:.4f}; sensor->Apple {f['S']}; matrix->Apple {f['M']}")


main()
