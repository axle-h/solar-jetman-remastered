#!/usr/bin/env bash
# Runs the original in MesenCE's test runner, headless: tools/mesen/run.sh SCRIPT.lua [ROM]
# No display is reachable, so Mesen cannot open a window; its config is private to vendor/.
# See docs/reference-environments.md.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mesen="$repo/vendor/Mesen"
home="$repo/vendor/mesen-home"

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 SCRIPT.lua [ROM]" >&2
  exit 2
fi
script="$(realpath "$1")"

if [[ $# -eq 2 ]]; then
  rom="$(realpath "$2")"
else
  rom="$(find "$repo/vendor/rom" -maxdepth 1 -iname '*.nes' 2>/dev/null | head -n 1 || true)"
  if [[ -z "$rom" ]]; then
    zip="$(find "$repo/vendor" -maxdepth 1 -iname '*.zip' | head -n 1)"
    [[ -n "$zip" ]] || { echo "no ROM: put the .nes or its .zip in vendor/" >&2; exit 1; }
    mkdir -p "$repo/vendor/rom"
    unzip -q -o "$zip" '*.nes' -d "$repo/vendor/rom"
    rom="$(find "$repo/vendor/rom" -maxdepth 1 -iname '*.nes' | head -n 1)"
  fi
fi

[[ -x "$mesen" ]] || { echo "MesenCE not found at $mesen" >&2; exit 1; }

# A minimal settings file skips the first-run wizard (which would open a window)
# and lets Lua scripts use io and os.
if [[ ! -f "$home/MesenCE/settings.json" ]]; then
  mkdir -p "$home/MesenCE"
  printf '{"Version":"2.2.1","ConfigUpgrade":5,"Debug":{"ScriptWindow":{"AllowIoOsAccess":true}}}' \
    > "$home/MesenCE/settings.json"
fi

exec env -u DISPLAY -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$home" \
  "$mesen" --testrunner "$rom" "$script"
