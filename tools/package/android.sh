#!/usr/bin/env bash
# Builds the Android APK in Docker (android/Dockerfile), runs the release scan on the game's
# library in it and signs it.
# Usage: tools/package/android.sh [OUT_DIR]   (default: target/package)
#   ABI=x86_64      for the Android emulator rather than an arm64 handheld
#   DEPLOY=1        then `adb install` it on the device adb sees
# Signed with android/release.keystore, made on the first build and never committed: keep it,
# since Android installs an update only over an app signed with the same key.
# ANDROID_KEYSTORE and ANDROID_KEYSTORE_PASSWORD pick another.
set -euo pipefail
cd "$(dirname "$0")/../.."

ABI=${ABI:-arm64-v8a}
case "$ABI" in
  arm64-v8a) target=aarch64-linux-android suffix=android-arm64 ;;
  x86_64) target=x86_64-linux-android suffix=android-x86_64 ;;
  *) echo "ABI is arm64-v8a or x86_64, not $ABI" >&2; exit 1 ;;
esac
version=$(cargo metadata --no-deps --format-version 1 |
  python3 -I -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="solar-jetman"))')
# versionCode: the version as MMmmpp
IFS=. read -r major minor patch <<< "$version"
version_code=$((major * 10000 + minor * 100 + patch))
# SDL's release, from the sdl3-sys the game links ("0.7.2+SDL-3.4.18")
sdl_version=$(sed -n '/^name = "sdl3-sys"/{n;s/^version = ".*+SDL-\(.*\)"/\1/p}' Cargo.lock)
rust_version=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
[ -n "$sdl_version" ] && [ -n "$rust_version" ] || { echo "cannot read the SDL or Rust version" >&2; exit 1; }

out=${1:-target/package}
mkdir -p "$out"
out=$(cd "$out" && pwd)
apk="$out/solar-jetman-$version-$suffix.apk"
image="solar-jetman-android-$ABI"
keystore=${ANDROID_KEYSTORE:-android/release.keystore}
export KEYSTORE_PASSWORD=${ANDROID_KEYSTORE_PASSWORD:-solar-jetman}
keys=$(cd "$(dirname "$keystore")" && pwd)
key_file=$(basename "$keystore")
build_args=(--build-arg "ABI=$ABI" --build-arg "TARGET=$target"
  --build-arg "SDL_VERSION=$sdl_version" --build-arg "RUST_VERSION=$rust_version")
# The toolchain runs as the host user, so that what it writes is the host user's.
as_host() {
  docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e KEYSTORE_PASSWORD \
    -v "$keys:/keys" -v "$out:/out" "$@"
}

if [ ! -f "$keystore" ]; then
  docker build . -f android/Dockerfile --target toolchain -t "$image-toolchain" "${build_args[@]}"
  echo "making $keystore"
  as_host "$image-toolchain" keytool -genkeypair -keystore "/keys/$key_file" \
    -alias solar-jetman -keyalg RSA -keysize 4096 -validity 36500 -dname "CN=solar-jetman" \
    -storepass:env KEYSTORE_PASSWORD -keypass:env KEYSTORE_PASSWORD
fi

docker build . -f android/Dockerfile -t "$image" "${build_args[@]}" \
  --build-arg "VERSION_CODE=$version_code" --build-arg "VERSION_NAME=$version"

rm -f "$apk" "$apk.idsig"
unsigned="$out/solar-jetman-unsigned.apk"
library="$out/libsolar_jetman_android-$ABI.so"
trap 'rm -f "$unsigned" "$library"' EXIT
as_host "$image" cp /app/android/app/build/outputs/apk/release/app-release-unsigned.apk \
  "/out/$(basename "$unsigned")"

# No ripped bytes in the game's library (needs the ROM in vendor/ or $SJ_ROM), checked before
# anything is signed; set SJ_SKIP_SCAN=1 only for a dry run.
if [ "${SJ_SKIP_SCAN:-}" = 1 ]; then
  echo "WARNING: release scan skipped; do not distribute this APK" >&2
else
  unzip -p "$unsigned" "lib/$ABI/libsolar_jetman_android.so" > "$library"
  cargo run --release --bin sj-tools -- scan-release "$library"
fi

as_host "$image" apksigner sign --ks "/keys/$key_file" --ks-pass env:KEYSTORE_PASSWORD \
  --out "/out/$(basename "$apk")" "/out/$(basename "$unsigned")"
rm -f "$apk.idsig"

as_host "$image" apksigner verify "/out/$(basename "$apk")"
echo "$apk ($version, versionCode $version_code, $ABI)"
if [ -n "${DEPLOY:-}" ]; then
  adb install -r "$apk"
fi
