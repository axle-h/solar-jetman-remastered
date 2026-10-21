# Clean-room policy

This project rebuilds Rare's *Solar Jetman: Hunt for the Golden Warpship* (NES, 1990) in Rust. It does so by **understanding** the original program's data and behaviour, then writing original code. We never copy Rare's code into the project, and the release build contains none of its graphics or sound. The repository will be public.

## Two teams

The original game is mostly code: physics, enemy behaviour, the sound driver and the level logic are all 6502 routines. Specifications of behaviour make it easy for code structure to leak, so the work is split between two teams that never share material except through `docs/spec/`.

| | Analysts (dirty) | Implementers (clean) |
|---|---|---|
| Who | Subagents given the analyst role in their prompt, and only those | The main session and every other subagent |
| Reads | The ROM, Ghidra, disassembly, emulator traces, `docs/` | `docs/`, the ROM through our own tools, the running original (black box) |
| Never reads | `src/`, `tests/` (to avoid writing specs shaped by our code) | `~/projects/solar-jetman-dirty/`, anything an analyst produced outside `docs/spec/` |
| Writes | `~/projects/solar-jetman-dirty/` (private), `docs/spec/` (public specs) | Everything else in this repository |

- **The analysts' workspace is outside the repository**, at `~/projects/solar-jetman-dirty/`, so that no search in this repository can reach it by accident. It holds the Ghidra project, disassembly, traces, the analysts' own notes and their throwaway scripts. None of it leaves that folder.
- **Analyst reports are spec-shaped.** What an analyst returns to the session that started it follows the spec rules below: no assembly, no labels, no decompiler identifiers, no code.
- **Spec review.** A fresh subagent that has read only `docs/` reviews each new or changed spec before implementers use it, and rejects anything that reads as a transliteration.

## Where knowledge may come from

1. **Observing the data:** hex dumps, statistics, trial decoding of the ROM.
2. **Public documentation** of the hardware and file formats: the NESdev wiki (CPU, PPU, APU, mapper 7), the iNES/NES 2.0 header, PNG, and similar.
3. **Black-box behaviour** of the original running in Mesen: what it displays, what it plays, and how its RAM changes in response to input.
4. **Static and dynamic analysis** (Ghidra, emulator traces), by analysts only, to learn *what* the data contains and *how* the game behaves.

## Rules

1. **Specification first.** Before any behaviour or format is implemented, it is written up in `docs/spec/` in our own words: field tables, offsets, bit layouts, state machines, formulas, algorithm descriptions in prose or short neutral pseudocode, and worked examples. See `docs/spec/README.md`.
   - **Allowed:** ROM offsets and CPU addresses as provenance and for locating data; RAM addresses and their meaning; constants, tables and enum values.
   - **Not allowed:** 6502 assembly, decompiler output, labels or identifiers from the analysis, or pseudocode that mirrors a routine's control flow line by line.
2. **Implement from the specification.** Rust is written from `docs/spec/` with our own names, types and structure. If a spec is wrong or incomplete, an analyst fixes the spec first, then the code changes.
3. **The ROM is data, never code.** The remake never executes, emulates or links the ROM's program. The original runs only in Mesen, as a source of ground truth.
4. **Validate against behaviour, not code.** Compare our game's state and screens with the original's, playing the same input (`PLAN.md`, "Validation").
5. **What may be committed, and what is ripped at first start** (decided by Alex, 2026-10-16):

   | Ripped from the player's ROM at first start; never in git or a release build | May be committed and compiled into the release build |
   |---|---|
   | Graphics: tile and sprite pixel data, palettes, and full-screen pictures such as the title screen and logos | Mechanics: physics, speeds, timers, damage, scoring, prices, the random-number algorithm, the password encoding |
   | Sound: music and sound-effect data | Level design: planet maps, terrain and collision data, object and enemy placement |
   |  | Sprite layouts and animation sequences (which tiles, in what order and timing; not the pixels) |
   |  | In-game text |

   Constants are facts, not expression. Level design, sprite layouts and the other data tables of the right-hand column are still not committed: to keep the public repository well clear of the original, the import extracts them from the player's ROM along with the graphics and sound, into our own formats (`src/rom/extract.rs`).

   Tables that only say *where* graphics or sound are, or *which* to use (offsets, lengths, pointers, palette preset indexes, per-level choices), are commit: they hold no pixels, colours or sound. Tables of pixel rows (e.g. gauge pieces) or colours are graphics, and ripped.
6. **No ROM bytes outside `vendor/`.** Unit tests use synthetic data we make ourselves. Tests that need the ROM read `vendor/` (or `$SJ_ROM`) and skip when it is absent.
7. **The release build is checked.** `sj-tools scan-release` looks for any run of the ROM's graphics or sound bytes in a release binary, and fails if it finds one. Debug builds may embed the ROM for convenience.

## Review checklist (every change touching game behaviour or a decoder)

- [ ] The spec in `docs/spec/` was written or updated first, and reviewed.
- [ ] `cargo run --bin sj-tools -- cleanroom` passes: no decompiler identifiers or 6502 assembly in the repository.
- [ ] Control flow and naming are our own.
- [ ] No graphics or sound bytes in the repository: tests, fixtures, snapshots or docs.
