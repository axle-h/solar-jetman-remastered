//! Solar Jetman remastered. The library holds the game and everything around it; the binaries
//! `solar-jetman` (the SDL3 program, `src/app/`) and `sj-tools` (`src/bin/sj-tools/`) use it.
//!
//! `game`, `rom` and `apu` are platform-free: no file, window or audio I/O, no unsafe
//! code, and they build for `wasm32-unknown-emscripten`. The clean-room check enforces it.

pub mod apu;
pub mod game;
pub mod rom;
pub mod tools;
