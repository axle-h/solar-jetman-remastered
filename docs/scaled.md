# The Scaled engine

The gameplay engine behind PHYSICS: REMASTERED (the default). Its physics tick runs at a rate R from 60 to 200 Hz, chosen at run time to match the display. It plays like the Original's 30 Hz logic step in real time, but acts on input within a tick. The Original engine (`src/game/gameplay`) is unchanged, for Mesen validation, replays and the original feel; `src/game/scaled` is a separate copy of its step logic, and the duplication is accepted. The differences of play are listed in `docs/deviations.md`, "The Scaled engine".

Each rule was derived by simulating the Original against candidates at 30-200 Hz (a lab since removed; it is in the git history under `tools/scaled-lab`), and every one reduces bit for bit to the Original's when a tick is a whole step (R = 30), except where a decision below changes it on purpose.

## Ground rules

- **Integer maths, no floats.** The tick fraction f is Q16 of a step (65536 = 1/30 s). Positions are Q16.16, speeds Q8.24 magnitudes with a direction, accumulators Q.24, i64 intermediates.
- **One clock** (`clock.rs`). `game_time: u64` in Q32 steps, advanced per tick by `ceil(2^32 * 30 / R)`: rounding up keeps deadlines from coming early. Steps are `gt >> 32`, frames `gt >> 31`. Each per-step or per-frame cadence (counter-bit tests, the random generator, timers, lifetimes, sound cadences, scans) fires once per newly crossed count, never on the raw low bits of a tick. Durations are deadlines on this clock. Events come 0 to 1 tick late and are never lost.
- **Save states are rate-free** (`SCALED_FORMAT_VERSION`). They hold game time, values in their Q formats and deadlines as time remaining; tick lengths, the wall-clock remainder and previous-tick positions are rebuilt on load. Pending press latches are saved. A state saved at one rate loads at any other.
- **Scans and spawners** run once per crossed step, never per tick (per tick multiplies spawns).

## Subsystems

| Area | Rule | Code |
|---|---|---|
| Input | Read every tick; presses and releases latched between ticks, so a tap always acts once. A new Left/Right rotates on its tick, then an accumulator seeded `f - 32768` rotates once per step of game time. Fire works the same way, autofire every N = 6/4/2 steps. Thrust acts on the first tick that sees B | `controls.rs` |
| Motion | All of a tick's accelerations, each times f, before the move, in the order thrust, momentum killer, gravity, caps; then `x += f*v_new + (v_new - v_old)*(1-f)/2`. Reflections through zero are signed and exact. The vertical thrust cap is lowered by the part of the step's gravity still to come; the reversal kick stays full size, once per crossing. Horizontal thrust carry-in is 0, vertical carry-in is the gravity byte scaled by f. Fuel drains `cost * f` per tick into an accumulator | `motion.rs`, `pod.rs` |
| Terrain contact | Tested every tick, the floor band from the tick's displacement. A response (reflection, halving) happens only when the pod is closing on the surface, or once per step of game time otherwise. Sound, sparks and damage have a one-step cooldown per contact kind (flat floor, floor slope, wall, flat ceiling, ceiling slope). Halving floors to the 8.8 grid. Hit pause 48 steps | `pod.rs` |
| Pod against objects | Every tick, each object's hurting touch (damage, sound, sparks, impulse, direction flip) behind a cooldown of exactly one step with the overflow kept: sustained contact hits 30 times a second at any R | `collide.rs` |
| Camera | Each tick, the pod's whole-pixel move in two half-moves through the dead-zone bands, each clamped to ±`ceil(240/R)`. The Original's -9 asymmetry is dropped. No sub-pixel camera state | `camera.rs` |
| Stars | Every tick, the tick's camera move at the layer ratios 1, 3/4 and 1/2; spawns and deaths on the frame clock, all eight stars; flicker from the frame clock at draw time | `stars.rs` |
| Tow rope | Per-step amounts times f. The angle advances by a trapezoid of the old and new swing speed, with a damping compensation; W loses 1/256 per step as a rate. Every force reads the angle as it was at the start of the step. W's carry snap is kept as an integer crossing with a half-share compensation. The spin counter counts on the step's starting angle. No torque without a load | `tow.rs`, `economy.rs` |
| Plain movers (shots, sparks, bullets) | `pos0 + v * (gt - t_spawn)` | `minions.rs`, `behave.rs` |
| Homing enemies (mine, orb, sphere, satellite, companion, drone, creature, machine) | Not scaled: per-tick versions lose the Original's oscillation. Their step runs at the step's start, against the pod extrapolated a step ahead, and they are drawn on their way to the step's end | `mod.rs`, `behave.rs`, `enemies.rs` |
| Table paths (formations, particles) | Integer phase on the step clock, interpolated by the step's fraction | `enemies.rs` |
| Random generator | The same generator, stepped per frame event with that tick's latched buttons; decisions read it once per step in slot order | `behave.rs` |
| Jetman | Accelerations times f; braking after a reflection until the next step line; no `y += 1` floor stick | `jetman.rs` |
| Final level | The ship's accelerations times f; carry, push and sway as 30 Hz events; the screen limits a step behind the camera, which follows the route on the frame clock | `final_level.rs` |
| Gate and warp | Bursts and fades as 30 Hz events from the gate's own start; the pod drawn on its way while the gate moves it | `gate.rs`, `warp.rs` |
| Shield, gauges, flame, flicker, hit pause, invulnerability | On the clock; shield growth `w += f` | `pod.rs` |

## Pod rest

On with PHYSICS: REMASTERED, with no switch of its own; the Original never has it, and `src/game` leaves it off unless asked (`Game::set_pod_rest`), so `scaled-compare` measures the engine without it. An upright pod (facing 0) landing on a flat floor slower than 2.00 px per step (`REST_BELOW`, Jetman's soft landing) comes to rest instead of hopping. The landing is an impact like a bounce. At rest there is no gravity, the floor is held as for Jetman standing, and the horizontal speed brakes by 0.20 a step. Thrust, turning away from upright, a push, the rope pulling or the floor going ends it.

## Rate control (`rate.rs`)

- **Measurement:** present intervals over a 1 s window (median, p10, p90). Samples over 0.25 s are ignored and dt is capped at 0.1 s, so a sleep never causes a burst.
- **Locked** (vsync on, steady intervals): the nearest of 60, 75, 90, 100, 120, 144, 165, 200 within 1.5%; above 200 Hz a divisor of the refresh rate under 200 (240 gives 120, 360 gives 180). A phase lock gives each frame a whole number of ticks, at up to 0.15% off wall-clock speed on 59.94 and 143.86 Hz panels.
- **Unlocked** (vsync off, VRR): the smallest list entry at or above 0.97 × the p90 frame rate.
- **Debounce:** locked, down after 1 s and up after 2 s; unlocked, down after 6 s and up after 1.5 s with a 12% dead band. Falling behind (busy over 80%) steps down one entry and blocks raising for 20 s × 3ⁿ.
- **Drawing:** the last tick is held when R divides the display rate or is a multiple of it, otherwise interpolated. Homing enemies are always interpolated.
- The app always asks for Auto; fixed rates are for the tools.

## Checking it (`sj-tools scaled-compare`)

The replays' stretches between test setups, cut into 1 s windows that start from the Original's state, at the test rates (`TEST_RATES`). Each step's buttons are held for 1/30 s of game time. A rate passes when the pod's and the camera's errors are within 2 px at the 90th percentile and the facing within 1 at the 99th; the tail is not judged, since an impact noticed in a tick that the Original's step passes over sends a window its own way. `--against-30` compares with Scaled at a whole step a tick, `--thrust-lag` gives B a step late, `--switch` changes the rate during each run, `--window 0` runs whole stretches. The original's attract demos (`--demos`) are replayed in the Original; the remake's own demos fly whichever engine is chosen.
