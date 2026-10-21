# Scaled-engine lab

The simulations behind `docs/scaled.md`: models of the Original's per-step rules, validated against
the real engine (`src/game`), and the Scaled candidates measured against them at 30-200 Hz. It is
throwaway analysis code, its own workspace outside the main crate and its lints. Run from this directory with
`cargo run --release --bin <name>`; it needs the ROM (`vendor/rom/` or `$SJ_ROM`) for the level data and the
replays in `tools/mesen/replays`. `rate/` and `input/` are Python. `out/` (gitignored) holds raw outputs.
