#!/usr/bin/env python3
"""The welcome gifts' wordmark: "OPEN NEGATIVE INITIATIVE" once, in Zalando Sans Expanded Black, on a tile
that repeats sideways without a seam; the stage embosses it into each foil (`light.wgsl`, `wordmark`)."""
import pathlib
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
FONT = ROOT / "crates/oni-ui/fonts/ZalandoSans_Expanded-Black.ttf"
OUT = ROOT / "crates/oni-app/src/stage/wordmark.jpg"
# A row's height in pixels, the letters' share of it, and the gap after the phrase in row heights.
ROW, LETTERS, GAP = 64, 0.56, 0.9

font = ImageFont.truetype(str(FONT), int(ROW * LETTERS / 0.72))
text = "OPEN NEGATIVE INITIATIVE"
left, top, right, bottom = font.getbbox(text)
width = right - left + int(GAP * ROW)
tile = Image.new("L", (width, ROW), 0)
draw = ImageDraw.Draw(tile)
draw.text((-left + int(GAP * ROW / 2), (ROW - (bottom - top)) / 2 - top), text, font=font, fill=255)
tile.save(OUT, quality=95, optimize=True)
print(f"{OUT.relative_to(ROOT)}: {width}x{ROW}, {OUT.stat().st_size} bytes", file=sys.stderr)
