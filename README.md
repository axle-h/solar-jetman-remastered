# Solar Jetman remastered

A clean-room re-implementation of Rare's *Solar Jetman: Hunt for the Golden Warpship* (NES, 1990) in Rust, for Linux, Windows and macOS. It plays as the original did, without the NES's flicker, slowdown and bugs, and with smooth, low-latency motion on modern displays.

**This project contains none of the original game's code, graphics or sound. You need your own copy of the USA ROM**, `Solar Jetman - Hunt for the Golden Warpship (USA).nes` (or a `.zip` holding it). Other releases and hacks are not accepted.

## Download and play

1. Download the archive for your system from the [releases page](../../releases) and unpack it.
   - **Linux:** the game uses the system's SDL3. Install it first: `sudo dnf install SDL3` (Fedora), `sudo apt install libsdl3-0` (Debian, Ubuntu) or your distribution's equivalent.
   - **Windows and macOS:** SDL3 is built in; nothing else to install.
2. Run `solar-jetman` (`solar-jetman.exe` on Windows).
3. At the first start the game asks for your ROM. Drop the `.nes` or `.zip` onto the window, or press Enter (A on a gamepad) to pick it in a file dialog. You can also start it with `solar-jetman --import PATH`.

The game checks the ROM, converts its graphics and sound into your player folder (keeping a copy of the ROM there, so a newer version can convert it again), and never asks again.

## Run from source

1. Install Rust with [rustup](https://rustup.rs). The pinned toolchain in `rust-toolchain.toml` installs itself on the first build.
2. Install SDL3:
   - **Linux:** the development package, e.g. `sudo dnf install SDL3-devel` or `sudo apt install libsdl3-dev pkg-config`.
   - **Windows:** Visual Studio Build Tools (MSVC) and CMake; SDL3 is built from source and linked statically.
   - **macOS:** the Xcode command line tools and CMake (`brew install cmake`); SDL3 is built from source.
3. Build and run:

   ```
   cargo run --release
   ```

   The release build asks for your ROM at first start, as above. For development, put the ROM in `vendor/rom/`: a debug build (`cargo run`) then embeds it and starts without the import step.

`docs/release.md` describes packaging, including the browser build.

## Controls

| NES | Keyboard | Gamepad |
|---|---|---|
| D-pad | Arrows | D-pad |
| A (fire) | X | A |
| B (thrust) | Z | B |
| Select | Backspace | Back |
| Start | Enter | Start |
| Port menu | F1 | Guide, or Select+Start |
| Quick save / load | F5 / F9 | (unbound) |
| Screenshot | F12 | (unbound) |
| Quit | Escape | (unbound) |

Every control can be rebound in the port menu (Controls, then Keyboard or Gamepad), which also resets them to these defaults. The port menu is driven by the same controls: A or Start selects, B goes back, Select moves down. Each change is saved at once.

## Port menu (F1)

- **Physics:** Remastered (the default) runs the physics at the display's rate, up to 200 Hz, so the pod answers within a frame rather than a 30 Hz step and moves smoothly, and lets an upright pod rest on flat ground instead of hopping. Original is the original's 30 Hz step exactly.
- **Video:** vsync, window scale, fullscreen, and Overscan, which crops the edges a television hides.
- **Sound:** volume and mute.
- **Controls:** remappable keyboard and gamepad.
- **Cheats:** infinite lives, infinite fuel (hits and crashes still push and spark but take no fuel), a full weapon gauge, and a level select.
- **Reset:** back to the title, as the console's reset button.
- **Save states:** three slots, with quick save and load.

## Fixes

The game plays as the original did; the full list of differences, with the reasons, is in [docs/deviations.md](docs/deviations.md).

**The NES's hardware limits, always removed:**

- No sprite flicker and no limit of 8 sprites a line; the tow beam always meets the item it holds.
- No slowdown or dropped frames, and no seams or colour glitches while scrolling.
- Scenes load at once, the status panel refreshes every frame, and the planet menu's box no longer recolours the picture beside it.
- Sprites are drawn where they belong at the screen's edges and are hidden properly behind the scenery.
- Smooth motion on any display: the picture runs at the display's rate, blending between the game's steps.
- Any button ends the licence screen, the ending picture and the credits.
- The title music loops, and the attract demos play their level's music and sound effects.
- The attract demos are flown by a rules-based pilot that plays from the game's state (it finds its way to an item, tows it home and fights on the way) instead of the original's fixed waypoint routes.

**Bugs fixed:** patrol routes, aimed shots, missiles that zig-zag or circle the pod, the type 22 volley, objects keeping a previous occupant's stale fields, the tow lift's random sign, shots wearing down indestructible obstacles, invalid hand-made passwords being accepted, the planet map showing rows twice, the smart bomb locking up the final level, explosions lost after a bomb, the star markers, the missing parallax stars, the approach globes' colours, the lost gem on planet 4's interlude, the terrain test's buffer overflow on level 11, and more.

## Your files

Settings (`settings.ini`), save states (`saves/`), screenshots (`screenshots/`), the imported data (`data/`) and the copy of your ROM (`rom.nes`) live in the player folder:

- Linux: `~/.local/share/solar-jetman-remastered/`
- Windows: `%APPDATA%\solar-jetman-remastered\data\`
- macOS: `~/Library/Application Support/solar-jetman-remastered/`

Set `SJ_FOLDER` to use another folder.

## Command line

```
solar-jetman [--import PATH] [--title] [--no-vsync]
```

- `--import PATH`: import the game's data from a `.nes` (or a `.zip` holding one), then start.
- `--title`: skip the licence screen.
- `--no-vsync`: start with vsync off.

`solar-jetman --help` lists the development options too.

## For contributors

The remake is written from specifications of the original's behaviour, never from its code: [docs/CLEAN_ROOM.md](docs/CLEAN_ROOM.md) is the policy and [docs/spec/](docs/spec/README.md) the specifications. Before sending a change, run `cargo fmt`, `cargo clippy --all-targets`, `cargo test` (tests that need the ROM skip without it) and `cargo run --bin sj-tools -- cleanroom`.

```
src/game/           the game: deterministic, no I/O, runs headless
src/rom/            ROM checks and the import of graphics, sound and level data
src/apu/            2A03-style sound synthesiser
src/app/            the program (solar-jetman): SDL3 window, input, audio, port menu
src/tools/          clean-room check, validation (bin: src/bin/sj-tools/)
tests/              integration tests
web/                the page of the browser build
docs/               clean-room policy, reference environments, specifications
tools/              headless Mesen runner, packaging scripts
vendor/             your ROM, Mesen and its private config        (gitignored)
```

## Licence

The code and documentation in this repository are under the [MIT licence](LICENSE). *Solar Jetman* and its graphics, sound and code are © 1990 Rare and are not part of this project.
