# Reference environments

How to run the original, as a black box for implementers and as a debugger for analysts.

## MesenCE, headless

`vendor/Mesen` is MesenCE (the community edition of Mesen 2), a single Linux binary. It is gitignored.

```
tools/mesen/run.sh SCRIPT.lua [ROM]
```

- **What it runs:** the game in Mesen's test runner at full speed, driven by the Lua script. The ROM defaults to the `.nes` in `vendor/`, unzipped once to `vendor/rom/`.
- **No window, ever:** the script clears `DISPLAY` and `WAYLAND_DISPLAY`. It also gives Mesen a private config folder, `vendor/mesen-home/`, whose generated `settings.json` skips the first-run wizard. Never launch `vendor/Mesen` directly: the GUI opens on Alex's desktop and ignores SIGTERM.
- **Lua file access:** Lua's `io` and `os` libraries are enabled in the private config only (`Debug.ScriptWindow.AllowIoOsAccess`). Use absolute paths.
- **Exit:** `emu.stop(code)` ends the run with that exit code. `print` goes to stdout.
- **Useful calls:**
  - `emu.addEventCallback(fn, emu.eventType.endFrame)`;
  - `emu.read(addr, emu.memType.nesInternalRam)`;
  - `emu.getScreenBuffer()` (256×240);
  - `emu.takeScreenshot()` (PNG bytes);
  - `emu.setInput(...)`;
  - `emu.addMemoryCallback(...)`.

  The full API is in Mesen's documentation.
- **Scene captures:** `tools/mesen/title-capture.lua` (screenshots, palette, sprite memory and pattern memory of chosen frames, with scripted input) and `tools/mesen/ppu-writes.lua` (every run of video-memory writes: which loads a scene makes) serve `sj-tools scene --compare`. Parameters are in each script's header. `tools/mesen/gameover-capture.lua` is the same capture with a forced game over (frame `SJ_FORCE_AT`); `tools/mesen/scene-capture.lua` (forced RAM writes `SJ_WRITES`, extra RAM bytes `SJ_RAM`) also logs the sound requests to `sound.txt`, which `sj-tools scene --compare` compares frame by frame with the remake's (the original's loading frames are added with `--gap FRAME:N`). Recipes: a forced scene change is a write of its code to RAM $3F in gameplay (`game-flow.md` 7.4): code 2 the surface menu (with $41 = 0), 5 a new level (with $DB), 6 the badge (with $67 gems and $DB), 7 the interlude, 1 a message ($66 = a slot, $03B2 + slot = the item kind, and $DB set by the game), 4 with $3E = 0 a game over (`SJ_WRITES=1301:3B=00,2900:3E=00,2900:3F=04` with Start at 1300 and 1500: the last-password screen is visible from frame 3144, `sj-tools scene --lastpassword 0 --offset 3143 --gap 3201:14 --input 3200:a`). `tools/mesen/replay-capture.lua` also records the attract demos (`tools/mesen/demos/`, `sj-tools replay-compare --demos`).
- **Logging noise:** Mesen logs every read of uninitialised memory at start-up. Filter it with `grep -v Uninitialized`.
- **Time limit:** the test runner stops by itself after about 100 seconds of wall-clock time. Split longer work into several runs, or start them from a save state.
- **Lua errors are silent:** a Lua error, for example a relative file path, makes the run hang until that limit with no output. Wrap risky code in `pcall` and print the error.
- **Code/data log:** Mesen's log of which bytes ran as code or were read as data carries over from run to run in the private config.
- **Several runs at once** are fine: they can share the private config.

## Ghidra (analysts only)

Ghidra is in `$GHIDRA_HOME` (default `~/ghidra`). Its project, and everything produced with it, lives in `~/projects/solar-jetman-dirty/`, never in this repository (`docs/CLEAN_ROOM.md`).
