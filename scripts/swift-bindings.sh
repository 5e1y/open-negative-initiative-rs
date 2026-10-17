#!/usr/bin/env bash
# Builds oni-link-ffi and writes its Swift bindings (.swift, header, modulemap) into OUT_DIR.
# Usage: scripts/swift-bindings.sh OUT_DIR [LIBRARY]; LIBRARY defaults to this host's release build.
set -euo pipefail

out=${1:?usage: swift-bindings.sh OUT_DIR [LIBRARY]}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

library=${2:-}
if [[ -z "$library" ]]; then
    cargo build -p oni-link-ffi --release
    case "$(uname -s)" in
        Darwin) library=target/release/liboni_link_ffi.dylib ;;
        *) library=target/release/liboni_link_ffi.so ;;
    esac
fi

mkdir -p "$out"
cargo run -p oni-link-ffi --features cli --bin uniffi-bindgen --release -- \
    generate --library "$library" --language swift --out-dir "$out"
ls -1 "$out"
