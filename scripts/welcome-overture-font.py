#!/usr/bin/env python3
"""The 1.6 interface's face for the welcome's overture, and for it alone: Inter, the face 1.6 set its words in, pinned
bold and cut to the glyphs of "ONI 1.6", under a family of its own so nothing else in the app can take it."""
import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "crates/oni-ui/fonts/Inter.ttf"
OUT = ROOT / "crates/oni-ui/fonts/ONI16Mark.ttf"
FAMILY = "ONI 1.6 Mark"

font = instancer.instantiateVariableFont(TTFont(SOURCE), {"wght": 700, "opsz": 32})
options = subset.Options()
options.name_IDs = [0, 1, 2, 3, 4, 5, 6]
options.layout_features = []
cut = subset.Subsetter(options)
cut.populate(text="ONI 1.6")
cut.subset(font)
for record in font["name"].names:
    if record.nameID in (1, 4, 16):
        record.string = FAMILY
    elif record.nameID == 6:
        record.string = FAMILY.replace(" ", "")
    elif record.nameID == 2:
        record.string = "Bold"
font.save(OUT)
print(f"{OUT.relative_to(ROOT)}: {OUT.stat().st_size} bytes", file=sys.stderr)
