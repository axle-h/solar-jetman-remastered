#!/usr/bin/env bash
# Builds the release binary, runs the release scan on it and makes a Linux tarball:
# binary, LICENSE, README and the licence texts of the dependencies linked into it.
# Usage: tools/package/linux.sh [OUT_DIR]   (default: target/package)
set -euo pipefail
cd "$(dirname "$0")/../.."

version=$(cargo metadata --no-deps --format-version 1 |
  python3 -I -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="solar-jetman"))')
name="solar-jetman-$version-linux-x86_64"
out=${1:-target/package}
stage="$out/$name"

cargo build --release --bin solar-jetman
bin=target/release/solar-jetman

rm -rf "$stage"
mkdir -p "$stage"
cp "$bin" LICENSE README.md "$stage/"
strip "$stage/solar-jetman" || true

# No ripped bytes in the binary (needs the ROM in vendor/ or $SJ_ROM); set SJ_SKIP_SCAN=1 only for a dry run.
if [ "${SJ_SKIP_SCAN:-}" = 1 ]; then
  echo "WARNING: release scan skipped; do not distribute this tarball" >&2
else
  cargo run --release --bin sj-tools -- scan-release "$stage/solar-jetman"
fi

cargo metadata --format-version 1 --filter-platform x86_64-unknown-linux-gnu |
  python3 -I tools/package/notices.py > "$stage/THIRD-PARTY-NOTICES.txt"

tar -C "$out" -czf "$out/$name.tar.gz" "$name"
echo "$out/$name.tar.gz"
