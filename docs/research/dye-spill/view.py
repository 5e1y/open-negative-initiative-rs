"""`view.py <folder> <out.jpg> <stem> <variant,...> [x0,y0,x1,y1]`: `dye_spill`'s renders side by side, in sRGB.

Each `<stem>-<variant>.f32` is linear Rec. 2020; the optional crop is in shares of the frame. Needs Pillow.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REC2020_TO_SRGB = np.array([[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]])


def read(path):
    with open(path, "rb") as f:
        w, h = map(int, f.readline().split())
        return np.frombuffer(f.read(), dtype="<f4").reshape(h, w, 3)


def srgb(linear):
    v = np.clip(linear @ REC2020_TO_SRGB.T, 0.0, 1.0)
    return np.where(v <= 0.0031308, 12.92 * v, 1.055 * v ** (1 / 2.4) - 0.055)


folder, out, stem, variants = Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4].split(",")
crop = [float(v) for v in sys.argv[5].split(",")] if len(sys.argv) > 5 else [0.0, 0.0, 1.0, 1.0]
tiles = []
for v in variants:
    a = read(folder / f"{stem}-{v}.f32")
    h, w, _ = a.shape
    a = a[int(crop[1] * h) : int(crop[3] * h), int(crop[0] * w) : int(crop[2] * w)]
    tile = Image.fromarray((srgb(a) * 255 + 0.5).astype(np.uint8))
    ImageDraw.Draw(tile).text((8, 6), v, fill=(255, 255, 0))
    tiles.append(tile)
columns = 2 if len(tiles) == 4 else min(len(tiles), 3)
rows = -(-len(tiles) // columns)
w, h = tiles[0].size
sheet = Image.new("RGB", (columns * w, rows * h))
for i, t in enumerate(tiles):
    sheet.paste(t, ((i % columns) * w, (i // columns) * h))
sheet.save(out, quality=92)
