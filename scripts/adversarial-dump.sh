#!/bin/zsh
# Renders the adversarial settings of crates/oni-pipeline/tests/adversarial/branches.json through the
# Swift app's own `--render`, on clones of a roll #8 frame and a Lumix frame, into $1 (ONI_ADVERSARIAL).
# Branches marked `whole` keep the neighbourhood stages, which `--render` drops: they need a build of
# a copy of the Swift sources whose `--render` honours ONI_RENDER_WHOLE (ONI_SWIFT_WHOLE_BIN).
# The originals are read, never written; the Swift build is run, never rebuilt.
set -e
OUT="${1:?usage: adversarial-dump.sh <output folder>}"
REPO="${0:A:h:h}"
SWIFT="${ONI_SWIFT_BIN:-$HOME/Documents/open-negative-initiative/build/Open Negative Initiative.app/Contents/MacOS/OpenNegative}"
ROLL="${ONI_RAW_DIR:-$HOME/Downloads/Roll #8}"
LUMIX="${ONI_LUMIX_DIR:-$HOME/Downloads/Negs}"
WHOLE="${ONI_SWIFT_WHOLE_BIN:-$HOME/Documents/oni-swift-probe/Open Negative Initiative.app/Contents/MacOS/OpenNegative}"
mkdir -p "$OUT/sources"
python3 -I - "$REPO/crates/oni-pipeline/tests/adversarial/branches.json" "$OUT" "$ROLL/DSC_6440.NEF" "$LUMIX/P1072392.RW2" <<'PY'
import json, os, shutil, subprocess, sys
branches, out, frames = json.load(open(sys.argv[1])), sys.argv[2], sys.argv[3:]
def merge(base, over):
    for k, v in over.items():
        if isinstance(v, dict) and isinstance(base.get(k), dict):
            merge(base[k], v)
        else:
            base[k] = v
    return base
lines, manifest = {False: [], True: []}, []
for frame in frames:
    stem, ext = os.path.splitext(os.path.basename(frame))
    sidecar = json.load(open(frame + ".json"))
    # The grid Swift quantises the crop on: ImageIO's width of the RAW, which LibRaw's may not match.
    size = subprocess.run(["sips", "-g", "pixelWidth", frame], check=True, capture_output=True, text=True).stdout
    full_width = int(size.split("pixelWidth:")[1].split()[0])
    rest = {"name": "rest", "branch": "the frame's own sidecar", "settings": {}}
    for b in [rest] + [x for x in branches if not x.get("whole")] + [dict(rest, name="rest-whole", whole=True)] + [x for x in branches if x.get("whole")]:
        whole = bool(b.get("whole"))
        target = os.path.join(out, "sources", f"{stem}-{b['name']}{ext}")
        if not os.path.exists(target):
            subprocess.run(["cp", "-c", frame, target], check=True)
        doc = json.loads(json.dumps(sidecar))
        merge(doc["settings"], b["settings"])
        json.dump(doc, open(target + ".json", "w"), indent=2)
        manifest.append({"index": len(lines[whole]), "whole": whole, "frame": os.path.basename(frame), "name": b["name"], "branch": b["branch"], "fullWidth": full_width})
        lines[whole].append(target)
open(os.path.join(out, "list.txt"), "w").write("\n".join(lines[False]) + "\n")
open(os.path.join(out, "whole.txt"), "w").write("\n".join(lines[True]) + "\n")
json.dump(manifest, open(os.path.join(out, "manifest.json"), "w"), indent=2)
print(f"{len(lines[False])} per-pixel and {len(lines[True])} whole renders listed")
PY
"$SWIFT" --render "$OUT/list.txt" "$OUT/render" 1000
ONI_RENDER_WHOLE=1 "$WHOLE" --render "$OUT/whole.txt" "$OUT/render-whole" 1000
