#!/usr/bin/env bash
# Builds the browser version (wasm32-unknown-emscripten, needs emsdk: set $EMSDK or have ~/emsdk),
# runs the release scan on the .wasm and collects the page in OUT_DIR.
# Usage: tools/package/web.sh [OUT_DIR]   (default: target/package/web)
# Serve OUT_DIR over http (e.g. `python3 -m http.server -d OUT_DIR`) and open index.html.
set -euo pipefail
cd "$(dirname "$0")/../.."

emsdk=${EMSDK:-$HOME/emsdk}
if ! command -v emcc >/dev/null; then
  # shellcheck disable=SC1091
  EMSDK_QUIET=1 source "$emsdk/emsdk_env.sh" >/dev/null
fi
out=${1:-target/package/web}
mkdir -p "$out"
out=$(cd "$out" && pwd)
rm -f "$out"/index.*

target=wasm32-unknown-emscripten
shell="$PWD/web/shell.html"
export SJ_NO_EMBED=1
# ASYNCIFY lets the blocking main loop yield to the page; FS is used by the page's shell.
export CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUSTFLAGS="-C link-arg=--use-port=sdl3 -C link-arg=-sASYNCIFY -C link-arg=-lidbfs.js \
 -C link-arg=-sEXPORTED_RUNTIME_METHODS=FS,IDBFS -C link-arg=-sFORCE_FILESYSTEM=1 \
 -C link-arg=-sALLOW_MEMORY_GROWTH=1 -C link-arg=-sINITIAL_MEMORY=128MB \
 -C link-arg=--shell-file=$shell -C link-arg=-o$out/index.html"
# The page is written straight to OUT_DIR by the link step; touch main.rs so cargo links again.
touch src/app/main.rs
cargo build --release --bin solar-jetman --target $target
for f in index.html index.js index.wasm; do
  [ -f "$out/$f" ] || { echo "the link step did not write $out/$f" >&2; exit 1; }
done
cp LICENSE "$out/"

# No ripped bytes in the page (needs the ROM in vendor/ or $SJ_ROM); set SJ_SKIP_SCAN=1 only for a dry run.
if [ "${SJ_SKIP_SCAN:-}" = 1 ]; then
  echo "WARNING: release scan skipped; do not distribute this build" >&2
else
  cargo run --release --bin sj-tools -- scan-release "$out/index.wasm"
fi
echo "$out/index.html"
