#!/usr/bin/env bash
# Installs SDL3 on an Ubuntu CI runner: the distribution package where there is one (Ubuntu 25.04
# and later), otherwise built from source into /usr/local.
set -euo pipefail
sudo=$([ "$(id -u)" = 0 ] || echo sudo)

$sudo apt-get update
if $sudo apt-get install -y libsdl3-dev pkg-config; then
  exit 0
fi
echo "libsdl3-dev is not packaged on this runner; building SDL3 from source"
# Build dependencies from https://wiki.libsdl.org/SDL3/README-linux
$sudo apt-get install -y build-essential git pkg-config cmake ninja-build \
  libasound2-dev libpulse-dev libpipewire-0.3-dev \
  libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev libxi-dev libxss-dev \
  libxtst-dev libxkbcommon-dev libwayland-dev libdecor-0-dev \
  libdrm-dev libgbm-dev libgl1-mesa-dev libgles2-mesa-dev libegl1-mesa-dev \
  libdbus-1-dev libibus-1.0-dev libudev-dev
src=$(mktemp -d)
git clone --depth 1 --branch release-3.4.0 https://github.com/libsdl-org/SDL "$src"
cmake -S "$src" -B "$src/build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DSDL_TESTS=OFF -DSDL_EXAMPLES=OFF
cmake --build "$src/build"
$sudo cmake --install "$src/build"
$sudo ldconfig
