#!/usr/bin/env python3
"""ONI's text face, written once from Zalando Sans into crates/oni-ui/fonts.

ONI Sans is Zalando Sans at its normal width, four weights, with two changes Slint cannot ask for: its
figures laid tabular in the default map, so a value keeps its width as it
changes, and the few signs the interface shows that Zalando Sans lacks (the key chords' glyphs, a check, Δ, σ)
taken from Inter at the same weight. A modified Zalando Sans may not carry the reserved name "Zalando": it is
renamed, its copyright kept (OFL 1.1, crates/oni-ui/fonts/ZALANDO-SANS-OFL.txt). The display face, Zalando
Sans Expanded Black, ships as it is.

    scripts/oni-fonts.py <folder holding Zalando Sans's static/ files>
"""

import sys
from pathlib import Path

from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

FAMILY = "ONI Sans"
WEIGHTS = {"Regular": 400, "Medium": 500, "SemiBold": 600, "Bold": 700}
# The signs the interface shows that Zalando Sans lacks, taken from Inter.
BORROWED = "↔⌘⇧⌥⌫✓Δσ"
DISPLAY = "ZalandoSans_Expanded-Black.ttf"
# What a value's width holds still: the figures; + − × are already one width in Zalando Sans.
FIGURES = "0123456789"

ROOT = Path(__file__).resolve().parent.parent
FONTS = ROOT / "crates" / "oni-ui" / "fonts"


def tabular(font: TTFont) -> int:
    """Points the figures of the default map at their `tnum` glyphs, nothing else (`tnum` also widens the
    space to a figure's width); returns how many codes moved."""
    subs = {}
    gsub = font["GSUB"].table
    for record in gsub.FeatureList.FeatureRecord:
        if record.FeatureTag != "tnum":
            continue
        for index in record.Feature.LookupListIndex:
            for table in gsub.LookupList.Lookup[index].SubTable:
                table = getattr(table, "ExtSubTable", table)
                subs.update(getattr(table, "mapping", {}) or {})
    moved = 0
    for cmap in font["cmap"].tables:
        for code, glyph in list(cmap.cmap.items()):
            if chr(code) in FIGURES and glyph in subs:
                cmap.cmap[code] = subs[glyph]
                moved += 1
    return moved


def borrow(font: TTFont, weight: int) -> int:
    """Draws into `font` the borrowed signs from Inter at `weight`, outlines decomposed and scaled to its
    units; returns how many it took."""
    inter = TTFont(FONTS / "Inter.ttf")
    instantiateVariableFont(inter, {"wght": weight, "opsz": 14}, inplace=True)
    scale = font["head"].unitsPerEm / inter["head"].unitsPerEm
    source, glyphs = inter.getGlyphSet(), inter.getBestCmap()
    order = list(font.getGlyphOrder())
    taken = 0
    for sign in BORROWED:
        code = ord(sign)
        if code in font.getBestCmap() or code not in glyphs:
            continue
        name = f"oni.uni{code:04X}"
        recording = DecomposingRecordingPen(source)
        source[glyphs[code]].draw(recording)
        pen = TTGlyphPen(None)
        recording.replay(TransformPen(pen, (scale, 0, 0, scale, 0, 0)))
        font["glyf"][name] = pen.glyph()
        width = round(inter["hmtx"][glyphs[code]][0] * scale)
        font["glyf"][name].recalcBounds(font["glyf"])
        font["hmtx"][name] = (width, font["glyf"][name].xMin if font["glyf"][name].numberOfContours else 0)
        order.append(name)
        for cmap in font["cmap"].tables:
            if cmap.isUnicode():
                cmap.cmap[code] = name
        taken += 1
    font.setGlyphOrder(order)
    font["glyf"].glyphOrder = order
    font["maxp"].numGlyphs = len(order)
    return taken


def rename(font: TTFont, style: str, weight: int) -> None:
    names = font["name"]
    full = f"{FAMILY} {style}" if style != "Regular" else FAMILY
    post = f"ONISans-{style}"
    for record in list(names.names):
        if record.nameID in (1, 2, 3, 4, 6, 16, 17, 21, 22, 25):
            names.removeNames(nameID=record.nameID)
    legacy = FAMILY if style in ("Regular", "Bold") else f"{FAMILY} {style}"
    legacy_style = style if style in ("Regular", "Bold") else "Regular"
    for nid, value in ((1, legacy), (2, legacy_style), (3, f"{post};ONI"), (4, full), (6, post), (16, FAMILY), (17, style)):
        names.setName(value, nid, 3, 1, 0x409)
    # The borrowed signs keep their authors' notice beside the face's own.
    notice = names.getDebugName(0) or ""
    names.setName(f"{notice} Signs borrowed from Inter: Copyright 2020 The Inter Project Authors (https://github.com/rsms/inter).", 0, 3, 1, 0x409)
    font["OS/2"].usWeightClass = weight
    # The style table names the source's own instances, which this file no longer is.
    if "STAT" in font:
        del font["STAT"]


def main() -> None:
    static = Path(sys.argv[1]) / "static"
    for style, weight in WEIGHTS.items():
        font = TTFont(static / f"ZalandoSans-{style}.ttf")
        moved = tabular(font)
        taken = borrow(font, weight)
        rename(font, style, weight)
        target = FONTS / f"ONISans-{style}.ttf"
        font.save(target)
        print(f"{target.name}: {moved} codes tabular, {taken} of {len(BORROWED)} signs borrowed")
    TTFont(static / DISPLAY).save(FONTS / DISPLAY)
    (FONTS / "ZALANDO-SANS-OFL.txt").write_text((Path(sys.argv[1]) / "OFL.txt").read_text())
    print(f"{DISPLAY}: as it is")


if __name__ == "__main__":
    main()
