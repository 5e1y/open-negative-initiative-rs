#!/bin/zsh
# Builds the probe the parity dumps run: a copy of the Swift sources in $1 (the Swift repository is
# only read), with ProbeTools.swift added, `--render` keeping the whole chain under ONI_RENDER_WHOLE,
# and the caches under ONI_PROBE_CACHES; the app bundle is cloned beside it with the probe binary.
set -e
OUT="${1:?usage: build.sh <probe folder>}"
SWIFT="${ONI_APP:-$HOME/Documents/open-negative-initiative}"
HERE="${0:A:h}"
mkdir -p "$OUT"
for d in Sources Kernels Resources Assets Package.swift Package.resolved VERSION; do rm -rf "$OUT/$d"; cp -cR "$SWIFT/$d" "$OUT/"; done
[ -d "$OUT/.build-arm64" ] || cp -cR "$SWIFT/.build-arm64" "$OUT/.build-arm64"
rm -rf "$OUT/.build-arm64/arm64-apple-macosx/release/ModuleCache"
cp "$HERE/ProbeTools.swift" "$OUT/Sources/OpenNegative/Pipeline/"
python3 -I - "$OUT" <<'PY'
import sys
root = sys.argv[1] + "/Sources/OpenNegative/Pipeline/"
def edit(name, a, b):
    p = root + name; s = open(p).read()
    assert s.count(a) == 1, (name, a[:50]); open(p, "w").write(s.replace(a, b))
edit("RenderDump.swift", "settings: settings.perPixelOnly(),",
     'settings: ProcessInfo.processInfo.environment["ONI_RENDER_WHOLE"] != nil ? settings : settings.perPixelOnly(),')
edit("AppFolders.swift", 'private static let cachesStandIn: URL? = isStandingIn ? sandbox("app-caches") : nil',
     'private static let cachesStandIn: URL? = ProcessInfo.processInfo.environment["ONI_PROBE_CACHES"].map { URL(fileURLWithPath: $0) } ?? (isStandingIn ? sandbox("app-caches") : nil)')
edit("CommandLineTools.swift", '    if args.contains("--report-keygen") { return ReportSeal.generate() }',
     '''    if args.contains("--probe-decode") || args.contains("--probe-heal") || args.contains("--probe-export") {
        let given = Array(args.dropFirst().filter { !$0.hasPrefix("--") })
        guard given.count >= 2 else { return 2 }
        if args.contains("--probe-decode") { return ProbeTools.decode(list: given[0], folder: given[1], side: given.count > 2 ? Double(given[2]).map { CGFloat($0) } : nil) }
        if args.contains("--probe-heal") { return ProbeTools.heal(list: given[0], folder: given[1]) }
        return ProbeTools.export(list: given[0], folder: given[1])
    }
    if args.contains("--report-keygen") { return ReportSeal.generate() }''')
PY
source "$HOME/.oni-toolchain/env.sh"
(cd "$OUT" && swift build -c release --arch arm64 --scratch-path .build-arm64)
rm -rf "$OUT/Open Negative Initiative.app"
cp -cR "$SWIFT/build/Open Negative Initiative.app" "$OUT/"
cp "$OUT/.build-arm64/arm64-apple-macosx/release/OpenNegative" "$OUT/Open Negative Initiative.app/Contents/MacOS/OpenNegative"
codesign -f -s - "$OUT/Open Negative Initiative.app/Contents/MacOS/OpenNegative"
echo "probe: $OUT/Open Negative Initiative.app/Contents/MacOS/OpenNegative"
