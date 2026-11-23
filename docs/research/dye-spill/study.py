"""`study.py [spektrafilm profiles folder]`: the spill each source and each reader leave, and what one matrix mends.

The spektrafilm folder holds the profiles ONI shipped before its data sheets (git show 44fcc040:crates/oni-pipeline/
resources/film/<stock>.json), kept out of the repository; without it the tables on them are left out.
"""
import sys
from pathlib import Path

import numpy as np

import spill
from spill import D50, LED, FAMILY, datasheet, show, weights

np.set_printoptions(precision=3, suppress=True)
SHEETS = datasheet()
SPEKTRA = []
if len(sys.argv) > 1:
    import json
    for p in sorted(Path(sys.argv[1]).glob("[!.]*.json")):
        if json.loads(p.read_text())["info"]["type"] == "negative" and "kodachrome" not in p.name:
            SPEKTRA.append(spill.Stock(p))


def family(stocks, w):
    s = np.array([x.spill(w) for x in stocks])
    return s.mean(0), s.std(0)


def colours(stock, reach=0.6, n=5):
    """Layer amounts for each layer exposed up to `reach` log H off the grey, the others elsewhere: a colour grid."""
    e = np.linspace(-reach, reach, n)
    grid = np.stack(np.meshgrid(e, e, e, indexing="ij"), -1).reshape(-1, 3)
    amounts = np.stack([np.interp(grid[:, c], stock.le, stock.curves[:, c]) for c in range(3)], -1)
    return grid, amounts


def miss(stock, w, m):
    """How far each channel's windowed density, through `m`, lies from its own layer's exposure: rms and worst, in
    stops, over the colour grid, the neutral part (their mean) left out as the windows hold it."""
    grid, amounts = colours(stock)
    d, _ = stock.density(amounts, w)
    d0, j = stock.density(stock.grey, w)
    # A window per channel: the density above the grey's over the channel's own neutral slope, in log H.
    u = (d - d0) / (j @ stock.gamma)
    # The layer's own windowed amount: what a channel reading its layer alone would show.
    t = (amounts - stock.grey) / stock.gamma
    err = (u @ m.T) - t
    err = err - err.mean(1, keepdims=True)
    # A colour whose scan channel a matrix pushed under zero has no density: left out, as a scan clips it.
    err = err[np.isfinite(err).all(1)]
    return np.sqrt((err**2).mean()) / 0.30103, np.abs(err).max() / 0.30103


print("## Spill per family, Rec. 2020 scan under D50 (rows R G B, columns cyan magenta yellow layer)")
w = weights("rec2020")
for name, pick in FAMILY.items():
    stocks = [s for s in SHEETS if pick(s.name)]
    mean, sd = family(stocks, w)
    print(f"{name} ({len(stocks)} sheets)\n  mean {show(mean)}\n  sd   {show(sd)}")
if SPEKTRA:
    mean, sd = family(SPEKTRA, w)
    print(f"spektrafilm ({len(SPEKTRA)} negatives)\n  mean {show(mean)}\n  sd   {show(sd)}")

print("\n## The same on each reader (all data-sheet negatives, then spektrafilm's)")
for reader, light in [("status_m", D50), ("narrow", D50), ("xyz", D50), ("rec2020", D50), ("rec2020", LED), ("dslr", D50),
                      ("dslr2020", D50), ("long_red", D50), ("long_red2020", D50), ("long_red2020", LED)]:
    w = weights(reader, light)
    line = f"{reader:13s} {'LED' if light is LED else 'D50'}  sheets {show(family(SHEETS, w)[0])}"
    if SPEKTRA:
        line += f"\n{'':19s}spektra {show(family(SPEKTRA, w)[0])}"
    print(line)

print("\n## The mending matrices (inverse of the mean spill), Rec. 2020 under D50")
w = weights("rec2020")
generic = {}
for name, stocks in [("sheets", SHEETS), ("spektra", SPEKTRA)]:
    if stocks:
        generic[name] = np.linalg.inv(family(stocks, w)[0])
        print(f"{name:8s} {show(generic[name])}")

print("\n## Miss in stops (rms / worst) over colours two stops apart between layers, Rec. 2020 under D50")
eye = np.eye(3)
rows = [("no matrix", lambda s, w: eye), ("its own matrix", lambda s, w: s.matrix(w))]
for name, m in generic.items():
    for dose in (1.0, 0.5):
        rows.append((f"{name} mean x{dose}", lambda s, w, m=m, dose=dose: eye + dose * (m - eye)))
for truth, stocks in [("sheets", SHEETS), ("spektra", SPEKTRA)]:
    if not stocks:
        continue
    print(f"truth: {truth}")
    for label, make in rows:
        r = np.array([miss(s, w, make(s, w)) for s in stocks])
        print(f"  {label:22s} rms {r[:, 0].mean():.3f}  worst {r[:, 1].max():.3f}")


print("\n## One layer alone over the bare film, Rec. 2020 under D50: the other channels' density per unit of its own")
print("   (cyan: G/R B/R, magenta: R/G B/G, yellow: R/B G/B), at 0.5 and 1.0 of the dye")
for name in ("kodak_gold_200", "kodak_portra_800", "kodak_ultra_max_400", "kodak_ultramax_400"):
    for source, stocks in [("sheet", SHEETS), ("spektra", SPEKTRA)]:
        for s in stocks:
            if s.name == name:
                d0, _ = s.density(np.zeros(3), w)
                out = []
                for amount in (0.5, 1.0):
                    for k in range(3):
                        d = s.density(amount * np.eye(3)[k], w)[0] - d0
                        out.append(" ".join(f"{d[c] / d[k]:+.2f}" for c in range(3) if c != k))
                print(f"{name:20s} {source:8s} " + " | ".join(out))
