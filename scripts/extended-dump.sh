#!/bin/zsh
# The second Swift comparison, into $1/extended: decodes outside LibRaw (a linear DNG, plain images,
# RGB passes), dust corrections generated on the full decode, and exports, all through a build of a
# copy of the Swift sources, `scripts/swift-probe/build.sh`, whose ProbeTools.swift adds `--probe-decode`,
# `--probe-heal` and `--probe-export` (ONI_SWIFT_WHOLE_BIN). Originals are cloned, never written.
set -e
OUT="${1:?usage: extended-dump.sh <ONI_ADVERSARIAL folder>}/extended"
REPO="${0:A:h:h}"
PROBE="${ONI_SWIFT_WHOLE_BIN:-$HOME/Documents/oni-swift-probe/Open Negative Initiative.app/Contents/MacOS/OpenNegative}"
ROLL="${ONI_RAW_DIR:-$HOME/Downloads/Roll #8}"
LUMIX="${ONI_LUMIX_DIR:-$HOME/Downloads/Negs}"
mkdir -p "$OUT/sources"
python3 -I "$REPO/scripts/synthetic-sources.py" "$OUT/sources"
python3 -I - "$OUT" "$ROLL" "$LUMIX" "$@[2,-1]" <<'PY'
import json, os, subprocess, sys
out, roll, lumix, extra = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
src = os.path.join(out, "sources")
base = json.load(open(os.path.join(roll, "DSC_6440.NEF.json")))
def clone(origin, name):
    target = os.path.join(src, name)
    if not os.path.exists(target):
        subprocess.run(["cp", "-c", origin, target], check=True)
    return target
def sidecar(target, settings, like=base):
    doc = json.loads(json.dumps(like))
    doc["settings"].update(settings)
    json.dump(doc, open(target + ".json", "w"), indent=2)
# Decodes: (file, side or 0 for full, what it reaches).
decodes = []
def decode(path, side, branch):
    decodes.append({"index": len(decodes), "file": os.path.basename(path), "side": side, "branch": branch})
    return path
decode(os.path.join(src, "linear.dng"), 0, "linear DNG, its own channels")
sidecar(clone(os.path.join(src, "linear.dng"), "linear-converted.dng"), {"primariesConverted": True})
decode(os.path.join(src, "linear-converted.dng"), 0, "linear DNG, ColorMatrix1 converted")
decode(os.path.join(src, "plain16.png"), 0, "16-bit PNG, untagged sRGB held")
sidecar(clone(os.path.join(src, "plain16.png"), "plain16-converted.png"), {"primariesConverted": True})
decode(os.path.join(src, "plain16-converted.png"), 0, "16-bit PNG, sRGB converted")
for enc in ("displayP3", "linear"):
    sidecar(clone(os.path.join(src, "plain16.png"), f"plain16-{enc}.png"), {"sourceEncoding": enc})
    decode(os.path.join(src, f"plain16-{enc}.png"), 0, f"16-bit PNG chosen {enc}")
names = {c: clone(os.path.join(src, f"pass-{c}.png"), f"trip-{c}.png") for c in "rgb"}
sidecar(names["r"], {"rgbPasses": {"red": "trip-r.png", "green": "trip-g.png", "blue": "trip-b.png",
        "registration": {"red": {"dx": 0.01}, "blue": {"scale": 1.01, "rotation": 0.02, "dy": -0.005}}}})
decode(names["r"], 0, "RGB passes, registered")
for origin, name, side, branch in [(os.path.join(roll, "2026-09-14 — contact sheet.tif"), "sheet.tif", 2000, "16-bit RGB TIFF, a contact sheet")] + [(e, os.path.basename(e), 2000, "plain file " + os.path.basename(e)) for e in extra]:
    if os.path.exists(origin):
        decode(clone(origin, name), side, branch)
open(os.path.join(out, "decode.txt"), "w").write("".join(os.path.join(src, d["file"]) + "\n" for d in decodes if d["side"] == 0))
open(os.path.join(out, "decode-2000.txt"), "w").write("".join(os.path.join(src, d["file"]) + "\n" for d in decodes if d["side"] == 2000))
json.dump(decodes, open(os.path.join(out, "decode.json"), "w"), indent=2)
# Corrections: two strokes a frame, one short and fine, one long and wide.
heal = []
for origin, name in [(os.path.join(roll, "DSC_6440.NEF"), "heal.NEF"), (os.path.join(lumix, "P1072392.RW2"), "heal.RW2")]:
    target = clone(origin, name)
    like = json.load(open(origin + ".json"))
    corrections = [
        {"id": "6F1A2B3C-4D5E-4F60-8172-93A4B5C6D7E%d" % len(heal), "stroke": [[0.31, 0.42], [0.315, 0.425]], "radius": 9},
        {"id": "7A1B2C3D-4E5F-4061-8273-94B5C6D7E8F%d" % len(heal), "stroke": [[0.62, 0.55], [0.66, 0.58], [0.70, 0.60]], "radius": 30},
    ]
    sidecar(target, {"corrections": corrections}, like)
    heal.append(target)
open(os.path.join(out, "heal.txt"), "w").write("\n".join(heal) + "\n")
# Exports: the contact sheet and the linear DNG, each format and space, one reduced.
exports = []
for path in [os.path.join(src, "sheet.tif"), os.path.join(src, "linear.dng"), os.path.join(src, "heal.NEF")]:
    if not os.path.exists(path):
        continue
    for fmt, space, edge in [("jpeg", "srgb", 0), ("jpeg", "p3", 0), ("tiff16", "adobergb", 0), ("tiff16", "prophoto", 0), ("jpeg", "srgb", 800)]:
        exports.append({"index": len(exports), "file": os.path.basename(path), "format": fmt, "space": space, "edge": edge})
open(os.path.join(out, "export.txt"), "w").write("".join(f"{os.path.join(src, e['file'])}\t{e['format']}\t{e['space']}\t{e['edge']}\n" for e in exports))
json.dump(exports, open(os.path.join(out, "export.json"), "w"), indent=2)
print(f"{len(decodes)} decodes, {len(heal)} frames to heal, {len(exports)} exports")
PY
rm -rf "$OUT/decode" "$OUT/heal" "$OUT/caches" "$OUT/export" "$OUT/render"
"$PROBE" --probe-decode "$OUT/decode.txt" "$OUT/decode"
"$PROBE" --probe-decode "$OUT/decode-2000.txt" "$OUT/decode-2000" 2000
ONI_PROBE_CACHES="$OUT/caches" "$PROBE" --probe-heal "$OUT/heal.txt" "$OUT/heal"
# The healed frames rendered with their patches, as the editor shows them, whole chain.
ONI_PROBE_CACHES="$OUT/caches" ONI_RENDER_WHOLE=1 "$PROBE" --render "$OUT/heal.txt" "$OUT/render" 1000
"$PROBE" --probe-export "$OUT/export.txt" "$OUT/export"
