#!/usr/bin/env python3
"""Folds a film's frames into the app's format (`app/help/reels.rs`): identical frames merged into one held longer,
each kept frame a JPEG of its own, all end to end in one file, and a list of each one's hold and length in bytes.
`fold.py <frames> <out stem>`: a frame held long is kept finer than one the eye only passes."""
import io
import pathlib
import sys

from PIL import Image, ImageChops

FPS = 25
frames, stem = pathlib.Path(sys.argv[1]), sys.argv[2]
kept, holds = [], []
for f in sorted(frames.glob('[0-9]*.png')):
    im = Image.open(f).convert('RGB')
    if kept and ImageChops.difference(im, kept[-1]).getbbox() is None:
        holds[-1] += 1 / FPS
    else:
        kept.append(im)
        holds.append(1 / FPS)
blobs = []
for im, hold in zip(kept, holds):
    out = io.BytesIO()
    im.save(out, 'JPEG', quality=84 if hold >= 0.3 else 64, optimize=True)
    blobs.append(out.getvalue())
w, h = kept[0].size
pathlib.Path(stem + '.jpgs').write_bytes(b''.join(blobs))
pathlib.Path(stem + '.txt').write_text(f"{w} {h}\n" + "".join(f"{x:.3f} {len(b)}\n" for x, b in zip(holds, blobs)))
total = sum(len(b) for b in blobs)
print(f"{pathlib.Path(stem).name}: {len(kept)} frames kept, {sum(holds):.2f} s, {total / 1e6:.2f} MB", file=sys.stderr)
