# CLAUDE.md

A clean-room Rust re-implementation of Rare's *Solar Jetman: Hunt for the Golden Warpship* (NES, USA, 1990), on SDL3. The player brings their own ROM. The repository is local for now and will be public.

Read first: `docs/CLEAN_ROOM.md` (the rules), `docs/spec/` (what to implement from) and `docs/reference-environments.md` (running the original).

## Hard rules

- **You are an implementer (clean)** unless the prompt that started you assigns you the analyst role:
  - **Never** read, list, search or open `~/projects/solar-jetman-dirty/`: no Ghidra output, disassembly, traces or analyst notes, even to check something.
  - Implement only from `docs/spec/`. If a spec is missing or unclear, ask for an analyst to extend it; don't work it out from the ROM's code.
  - Looking at the ROM's *data* through our own tools, and running the original in Mesen as a black box, are fine.
- **Analysts** follow `~/projects/solar-jetman-dirty/README.md`, and write to this repository only in `docs/spec/`.
- **No graphics or sound bytes in git.** `vendor/` (gitignored) holds the ROM, Mesen and its private config. Tests use synthetic data; ROM tests read `vendor/` (or `$SJ_ROM`) and skip without it.
- **The ROM is data, never code** in the remake: no 6502 emulation.
- **Git is read-only** unless Alex explicitly asks: no commit, reset, rebase, push. Don't push to GitHub.
- **Mesen never opens a window.** Run it only through `tools/mesen/run.sh` (no display, private config). Never launch `vendor/Mesen` directly. Kill only processes you started: `ps -eo pid,args | grep -i '[m]esen --'`.
- **Check with Alex** before changing scope or an established design decision.

## Conventions

- **One crate**, `solar-jetman`, edition 2024: a library (`src/lib.rs`) and two binaries, `solar-jetman` (`src/app/`, `cargo run`) and `sj-tools` (`src/bin/sj-tools/`). On Android, `src/app/` is built as a library instead (`docs/android.md`), so its modules name each other with `super::`, not `crate::`. Dependencies in `Cargo.toml` with exact versions (`=x.y.z`); keep them few.
- **Platform-free core:** `src/game`, `src/rom` and `src/apu` do no file, window or audio I/O (the clean-room check enforces it), and `#![forbid(unsafe_code)]`. The library and the app must build for `wasm32-unknown-emscripten`.
- **Our own names:** no decompiler identifiers (`FUN_`, `DAT_`, `LAB_`, `uVar`, `param_`, …) or 6502 assembly anywhere. `cargo run --bin sj-tools -- cleanroom` checks.
- **Fidelity:** faithful to the original, minus hardware limitations (no flicker, sprite limits, slowdown or seams) and minus bugs and glitches. Additions go behind port-menu switches.
- **Before finishing a change:** `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and the clean-room check.
