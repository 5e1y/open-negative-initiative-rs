"""`hull.py <folder of *-above.f32>...`: how pure a layer real scans show, a bound on the spill from pictures alone.

Every pixel's density over the bare film is a sum of the three layers' columns, none negative, so no pixel can lie
outside their cone: the smallest G/R a scan ever shows is at least the cyan layer's own G/R, and so on for the six
ratios. `dye_spill` writes the `above` files (density over the sidecar's bare film, Rec. 2020 channels).
"""
import sys
from pathlib import Path

import numpy as np

PAIRS = [("cyan", 0, 1, "G/R"), ("cyan", 0, 2, "B/R"), ("magenta", 1, 0, "R/G"), ("magenta", 1, 2, "B/G"), ("yellow", 2, 0, "R/B"), ("yellow", 2, 1, "G/B")]


def read(path):
    with open(path, "rb") as f:
        w, h = map(int, f.readline().split())
        return np.frombuffer(f.read(), dtype="<f4").reshape(h, w, 3)


def block(a, n):
    """`a` averaged over n x n blocks: grain and dust leave the ratios."""
    h, w = a.shape[0] // n * n, a.shape[1] // n * n
    return a[:h, :w].reshape(h // n, n, w // n, n, 3).mean((1, 3))


for folder in sys.argv[1:]:
    pixels = []
    files = sorted(Path(folder).glob("[!.]*-above.f32"))
    for p in files:
        a = read(p)
        h, w, _ = a.shape
        # The holder's edge and the rebate left out: the middle of the frame.
        pixels.append(block(a[h // 8 : -h // 8, w // 8 : -w // 8], 4).reshape(-1, 3))
    px = np.concatenate(pixels)
    print(f"{folder}: {len(files)} frames, {len(px)} blocks; least density over the base (0.1 %): {np.round(np.quantile(px, 0.001, axis=0), 3)}")
    for layer, own, other, label in PAIRS:
        # A layer well developed: its own channel 0.5 over the base or more.
        dense = px[px[:, own] > 0.5]
        ratio = dense[:, other] / dense[:, own]
        q = np.quantile(ratio, [0.0005, 0.005, 0.05, 0.5]) if len(ratio) else [np.nan] * 4
        print(f"  {layer:8s} {label}: lowest 0.05 % {q[0]:+.3f}  0.5 % {q[1]:+.3f}  5 % {q[2]:+.3f}  median {q[3]:+.3f}  ({len(ratio)} blocks)")
