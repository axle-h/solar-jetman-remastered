# Performance against emulation

How fast the remake runs flat out, headless, next to the original in Mesen. Measured on 2026-10-22 on an AMD Ryzen 9 7900X (one core), release build.

## Result

Both ran from power-on with no input: the licence screen, the title and the attract demos.

| Run | Frames/s | × real time (60.1 Hz) | Per frame |
|---|---|---|---|
| Mesen: full emulation (CPU, PPU, APU) and a Lua frame counter | ~520 | ~8.6× | ~1.9 ms |
| Remake, full: logic, drawing the 256×240 frame, sound synthesised at 48 kHz | ~2,900 | ~48× | ~0.35 ms |
| Remake, logic and drawing | ~6,000–7,000 | ~100–117× | ~0.15 ms |
| Remake, logic only | ~220,000–380,000 | thousands × | ~3–5 µs |

- **Full against full, the remake is about 5.5× faster.** Mesen emulates the 6502 and the PPU cycle by cycle; the game's own rules take a few microseconds a frame.
- **Sound synthesis is the largest cost** (~0.2 ms a frame), more than drawing (~0.14 ms) and logic together. It is the place to look for a faster fast-forward or replay check.
- **Caveats:** the remake's frame stays as palette indices, while Mesen also converts its picture to RGB. The Lua callback adds a little to Mesen. Mesen starts up in about a second, which is left out of its figure. Logic-only speed varies with which scenes the run covers.

## Reproducing

**Mesen.** `tools/mesen/run.sh bench.lua` with `SJ_FRAMES=30600`. The first 600 frames, which cover start-up, are left out of the timing. Stay well under the test runner's 100-second limit (`reference-environments.md`): a 60,000-frame run did not finish within it.

```lua
local target = tonumber(os.getenv("SJ_FRAMES"))
local frames, t0 = 0, nil
emu.addEventCallback(function()
  frames = frames + 1
  if frames == 600 then t0 = os.clock() end
  if frames == target then
    local dt = os.clock() - t0
    print(string.format("mesen: %d frames in %.3fs = %.0f fps", target - 600, dt, (target - 600) / dt))
    emu.stop(0)
  end
end, emu.eventType.endFrame)
```

**Remake.** A separate crate depending on `solar-jetman` by path, run as `sj-bench FRAMES logic|draw|full`. Each frame is one `Game::pass` with no buttons held. `draw` adds a `Game::draw` per pass. `full` also hands the sound requests to a `Player`, as the app does, and runs `Player::run_frame`.

```rust
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::playfield::Frame;
use solar_jetman::game::scene::{Game, SoundRequest, Start};
use solar_jetman::game::sound::player::Player;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let passes: u32 = args[1].parse().unwrap();
    let mode = args[2].as_str();
    let repo = std::path::Path::new("/path/to/solar-jetman");
    let (_, rom) = solar_jetman::tools::load_rom(repo, None).unwrap();
    let assets = solar_jetman::tools::assets(&rom).unwrap();
    let pack = solar_jetman::rom::import(&rom).unwrap();
    let periods = solar_jetman::rom::note_periods(&pack.files).unwrap();
    let sound = solar_jetman::rom::sound_data(&pack.files, periods).unwrap();
    let mut player = Player::new(sound, 48_000);
    let mut game = Game::new(&assets, Start::Cold).unwrap();
    let mut frame = Frame::default();
    let t = Instant::now();
    for _ in 0..passes {
        game.pass(&assets, Buttons(0)).unwrap();
        let requests = game.take_sound();
        if mode != "logic" {
            game.draw(&mut frame, &assets, 1.0);
        }
        if mode == "full" {
            player.sound_mut().set_frame(game.sound_counter() as u8);
            for request in requests {
                match request {
                    SoundRequest::SongIfNew(s) => player.sound_mut().play_song_if_new(s),
                    SoundRequest::SongLooped(s) => player.sound_mut().play_song_looped(s),
                    SoundRequest::Request(r) => player.sound_mut().apply(r),
                    SoundRequest::Pause => player.pause(),
                    SoundRequest::Resume => player.resume(),
                }
            }
            std::hint::black_box(player.run_frame());
        }
    }
    let secs = t.elapsed().as_secs_f64();
    std::hint::black_box(&frame);
    println!("{mode}: {passes} frames in {secs:.3}s = {:.0} fps", passes as f64 / secs);
}
```
