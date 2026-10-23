# Releasing

Every release is built without the ROM and must pass `sj-tools scan-release` (no graphics, sound or code bytes of the ROM in the binary). The scan needs the ROM (`vendor/` or `$SJ_ROM`), so it runs on a developer machine, not in CI.

## Tagging

Set `version` in `Cargo.toml`, run `tools/package/linux.sh` and `tools/package/web.sh` (scans included) on the commit, then push the tag `vVERSION`. The tag runs two workflows:

- `.github/workflows/release.yml` checks that the tag matches the crate version, builds the Linux, Windows (`x86_64`) and macOS (`aarch64`, no `.app`, unsigned) archives with `SJ_SKIP_SCAN=1` and attaches them to a **draft** GitHub Release. Check it, then publish it on GitHub.
- `.github/workflows/pages.yml` deploys the web build (below).

## Linux

`tools/package/linux.sh [OUT_DIR]` builds the release binary, strips it, scans it and writes `solar-jetman-VERSION-linux-x86_64.tar.gz` with the binary, `LICENSE`, `README.md` and `THIRD-PARTY-NOTICES.txt` (licence files of the linked crates, from `cargo metadata`). Needs SDL3 installed (the binary links the system library). `SJ_SKIP_SCAN=1` is for dry runs only.

## Windows (untested)

1. Install Rust (the pinned toolchain), Visual Studio Build Tools (MSVC) and CMake. The `sdl3` crate builds SDL3 from source and links it statically (`build-from-source-static`).
2. `cargo build --release --bin solar-jetman`
3. `cargo run --release --bin sj-tools -- scan-release target\release\solar-jetman.exe` with the ROM in `vendor\`.
4. Zip `solar-jetman.exe`, `LICENSE`, `README.md` and the notices (`cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc | python tools/package/notices.py > THIRD-PARTY-NOTICES.txt`).

## macOS (untested)

1. Install Rust and CMake (`brew install cmake`), Xcode command line tools.
2. Build as above (once per `aarch64-apple-darwin` and `x86_64-apple-darwin`, or `lipo` the two into one binary).
3. Scan the binary as above.
4. Put the binary in `Solar Jetman.app/Contents/MacOS/`, add an `Info.plist`, then sign and notarise (needs an Apple developer account); zip or make a dmg with `LICENSE` and the notices (`--filter-platform aarch64-apple-darwin`).

## Web (emscripten)

`tools/package/web.sh [OUT_DIR]` (default `target/package/web`) builds `src/app` for `wasm32-unknown-emscripten` with emsdk (`$EMSDK`, or `~/emsdk`), writes `index.html`, `index.js` and `index.wasm` to OUT_DIR, and runs `scan-release` on the `.wasm` (no hits). Serve the folder over http and open `index.html`. The link flags (`--use-port=sdl3`, ASYNCIFY, `-lidbfs.js`, the shell file) are set by the script through `CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUSTFLAGS`; the web build never embeds the ROM.

GitHub Pages: pushing a `v*` tag runs `.github/workflows/pages.yml`, which builds the page with `web.sh` and deploys it. CI has no ROM, so it builds with `SJ_SKIP_SCAN=1`: run `tools/package/web.sh` locally (scan included) on the commit before tagging it.

How it works:
- The page is `web/shell.html`. Before the program starts it mounts IDBFS at `/solar-jetman` (the player's folder, a fixed path in `Folder::locate`) and loads it from IndexedDB. `sjSync()` writes it back, batched; `src/app` calls it (`web.rs`) after settings, saves, the import and screenshots.
- The main loop keeps its blocking shape; `web::pause` calls `emscripten_sleep` so the browser gets the thread.
- ROM import: a "Choose ROM file" button and drag and drop on the page. The page writes the file to `/solar-jetman/import.nes`; the ROM-needed screen sees it, imports it and deletes it. SDL's own dialog and drop events are not used on the web.

Not yet tested: Firefox, Safari, a real file-picker click, touch, audible sound (browsers need a click or key before audio starts), gamepads, and slow machines (the loop yields with `emscripten_sleep`, not `requestAnimationFrame`; a callback main loop would be smoother).
