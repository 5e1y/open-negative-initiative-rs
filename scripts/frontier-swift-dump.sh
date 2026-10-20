#!/bin/zsh
# Frames rendered on the Frontier by the Swift app's own `--render` into $1/<frame>.f32 (look_lab_strip's
# ONI_SWIFT_FRONTIER), from clones whose sidecars pick the Frontier; originals and the Swift build untouched.
set -e
OUT="${1:?usage: frontier-swift-dump.sh <output folder> <frame>…}"
shift
SWIFT="${ONI_SWIFT_BIN:-$HOME/Documents/open-negative-initiative/build/Open Negative Initiative.app/Contents/MacOS/OpenNegative}"
ROLL="${ONI_RAW_DIR:?ONI_RAW_DIR names the roll's folder}"
mkdir -p "$OUT/sources"
: >"$OUT/list.txt"
for frame in "$@"; do
    target="$OUT/sources/$frame"
    [ -e "$target" ] || cp -c "$ROLL/$frame" "$target"
    python3 -I -c 'import json, sys
doc = json.load(open(sys.argv[1]))
doc["settings"].update(face="scanner", scannerMachine="frontier")
json.dump(doc, open(sys.argv[2], "w"), indent=2)' "$ROLL/$frame.json" "$target.json"
    echo "$target" >>"$OUT/list.txt"
done
"$SWIFT" --render "$OUT/list.txt" "$OUT/render" 1000
i=0
for frame in "$@"; do
    mv "$OUT/render/$(printf %05d $i).f32" "$OUT/$frame.f32"
    i=$((i + 1))
done
