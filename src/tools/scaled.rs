//! The Scaled engine checked against the Original by tolerance (`docs/scaled.md`,
//! "Checking it").
//!
//! The Original plays a replay step by step with the remake's rules, which Scaled follows. Scaled
//! plays the same input at a tick rate, each step's buttons held for 1/30 s of game time, and is
//! read at the Original's step times: the state after step k is compared with Scaled's at game
//! time k steps, between the two ticks around it. A replay's test setups (warps, pokes, objects)
//! cut it into stretches, and each stretch starts Scaled from the Original's state.

use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::{Gameplay, ValidationInput};
use crate::game::level::World;
use crate::game::scaled::Scaled;
use crate::game::scaled::clock::STEP;
use crate::tools::replay::{Capture, Replay, apply_setups, start_game};

/// The rates every check runs at (`docs/scaled.md`).
pub const TEST_RATES: [u32; 9] = [60, 75, 90, 100, 120, 137, 144, 165, 200];

/// A rate for [`run`] that changes every [`SWITCH_STEPS`] steps through [`TEST_RATES`], as the
/// rate controller may change it in play: the state is rate-free, so a change must not show.
pub const SWITCHING: u32 = 0;
pub const SWITCH_STEPS: usize = 45;

/// What a run may differ by from the Original and still play like it, from the results files'
/// error tables (`docs/scaled.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    /// The pod's position, in pixels (`docs/scaled.md`: 1-2 px at the 90th percentile with events).
    pub pod: f64,
    /// The camera, in pixels (`docs/scaled.md`).
    pub camera: f64,
    /// The facing, in units of the 16 facings (`docs/scaled.md`: at most 1 after any hold).
    pub facing: u8,
}

pub const BUDGET: Budget = Budget {
    pod: 2.0,
    camera: 2.0,
    facing: 1,
};

/// Whether a rate's runs together play like the Original: the 90th percentile of the pod's and
/// the camera's errors within the budget, and the facing's 99th. The tail is not judged: an
/// impact noticed in a tick that the Original's step passes over (`docs/scaled.md`) sets a window
/// apart from the Original for the rest of it, by design.
pub fn passes(pod: &Errors, camera: &Errors, facing: &Errors, budget: Budget) -> bool {
    pod.percentile(90) <= budget.pod
        && camera.percentile(90) <= budget.camera
        && facing.percentile(99) <= f64::from(budget.facing)
}

/// What the comparison reads of either engine at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sample {
    pub pod: (f64, f64),
    /// The pod's speed, px per step, right and down positive.
    pub speed: (f64, f64),
    pub camera: (f64, f64),
    pub facing: u8,
}

fn original_sample(game: &Gameplay) -> Sample {
    let body = &game.pod.body;
    let at = |whole: u16, fraction: u8| f64::from(whole) + f64::from(fraction) / 256.0;
    let camera = game.camera.position;
    let signed = |speed: u16, negative: bool| {
        let v = f64::from(speed) / 256.0;
        if negative { -v } else { v }
    };
    Sample {
        pod: (at(body.x, body.x_fraction), at(body.y, body.y_fraction)),
        speed: (
            signed(body.h_speed, body.h_left),
            signed(body.v_speed, !body.v_down),
        ),
        camera: (f64::from(camera.x), f64::from(camera.y)),
        facing: game.pod.facing,
    }
}

fn scaled_sample(scaled: &Scaled) -> Sample {
    let at = |q16: u32| f64::from(q16) / 65536.0;
    let body = &scaled.pod.body;
    let signed = |magnitude: u32, negative: bool| {
        let v = f64::from(magnitude) / f64::from(1u32 << 24);
        if negative { -v } else { v }
    };
    Sample {
        pod: (at(scaled.pod_q16().0), at(scaled.pod_q16().1)),
        speed: (
            signed(body.h_mag, body.h_left),
            signed(body.v_mag, !body.v_down),
        ),
        camera: (
            f64::from(scaled.camera.position.x),
            f64::from(scaled.camera.position.y),
        ),
        facing: scaled.pod.facing,
    }
}

/// The sample `t` of the way from `a` to `b`, the facing as `b` has it.
fn between(a: Sample, b: Sample, t: f64) -> Sample {
    let mix = |a: f64, b: f64| a + (b - a) * t;
    Sample {
        pod: (mix(a.pod.0, b.pod.0), mix(a.pod.1, b.pod.1)),
        speed: b.speed,
        camera: (mix(a.camera.0, b.camera.0), mix(a.camera.1, b.camera.1)),
        facing: b.facing,
    }
}

/// Part of a replay the Original played with no test setup: Scaled starts from the Original's
/// state before its first step.
#[derive(Debug, Clone)]
pub struct Stretch {
    /// The replay's step (1-based) the stretch starts with.
    pub first_step: usize,
    /// The replay keeps the tank full (its `keepfuel` setup).
    pub keep_fuel: bool,
    pub start: Scaled,
    /// Each step's buttons, and the Original's state after it.
    pub buttons: Vec<Buttons>,
    pub samples: Vec<Sample>,
}

impl Stretch {
    fn new(first_step: usize, game: &Gameplay, replay: &Replay) -> Stretch {
        Stretch {
            first_step,
            keep_fuel: replay.keep_fuel.is_some_and(|from| first_step >= from),
            start: Scaled::from_original(game),
            buttons: Vec::new(),
            samples: Vec::new(),
        }
    }
}

/// The Original's play of a replay, as the comparison needs it.
#[derive(Debug, Clone)]
pub struct Reference {
    pub stretches: Vec<Stretch>,
    /// Why the Original's play stopped before the replay's end, if it did.
    pub stopped: Option<String>,
}

/// Whether the replay sets something up before `step`; the fuel kept full from then on is
/// Scaled's infinite-fuel cheat, which fills the tank as often.
fn has_setup(replay: &Replay, step: usize) -> bool {
    replay.keep_fuel == Some(step)
        || replay.fuel_pokes.contains_key(&step)
        || replay.warps.contains_key(&step)
        || replay.objs.contains_key(&step)
        || replay.spins.contains_key(&step)
        || replay.items.contains_key(&step)
}

/// Plays `replay` in the Original with the remake's rules, until its end, a scene change or a
/// pause (which Scaled does not have yet).
///
/// With `window`, a stretch is also cut every `window` steps, so that Scaled restarts from the
/// Original's state as the lab's measurements did and an early difference does not carry on
/// through a chaotic stretch.
pub fn reference(
    world: &World,
    replay: &Replay,
    capture: &Capture,
    validation: Option<&ValidationInput>,
    window: Option<usize>,
) -> Result<Reference, String> {
    if replay.pod_sync {
        return Err("the replay takes the pod from the original every step".into());
    }
    let mut game = start_game(world, replay, capture, validation);
    game.set_original_rules(false);
    let count = if replay.demo.is_some() {
        capture.samples.last().map_or(0, |s| s.step)
    } else {
        replay.steps.len()
    };
    let mut stretches = Vec::new();
    let mut stretch = Stretch::new(1, &game, replay);
    let mut stopped = None;
    for index in 0..count {
        let step = index + 1;
        if step > 1 {
            let cut = has_setup(replay, step) || window.is_some_and(|w| stretch.buttons.len() >= w);
            apply_setups(&mut game, replay, step);
            if cut {
                let next = Stretch::new(step, &game, replay);
                stretches.push(std::mem::replace(&mut stretch, next));
            }
        }
        let buttons = if replay.demo.is_some() {
            // The autopilot's buttons are what Scaled is given.
            Buttons(0)
        } else {
            replay.steps.get(index).copied().unwrap_or_default()
        };
        game.step(world, buttons);
        game.take_sound();
        let given = if replay.demo.is_some() {
            game.snapshot()
                .iter()
                .find(|&&(name, _)| name == "ih")
                .map_or(Buttons(0), |&(_, held)| Buttons(held as u8))
        } else {
            buttons
        };
        stretch.buttons.push(given);
        stretch.samples.push(original_sample(&game));
        let why = if game.transition().is_some() {
            Some(format!("a scene change after step {step}"))
        } else if game.paused() {
            Some(format!("a pause at step {step}"))
        } else if game.demo_over() {
            Some(format!("the demo's end at step {step}"))
        } else {
            None
        };
        if why.is_some() {
            stopped = why;
            break;
        }
    }
    stretches.push(stretch);
    stretches.retain(|s| !s.buttons.is_empty());
    Ok(Reference { stretches, stopped })
}

/// The same replay as Scaled plays it a whole step a tick (30 Hz), where every rule is the
/// Original's but for the decisions that change them: compared with it, a rate shows what its
/// ticks add.
pub fn at_step_rate(world: &World, reference: &Reference) -> Reference {
    let mut out = reference.clone();
    for stretch in &mut out.stretches {
        let mut scaled = stretch.start.clone();
        if stretch.keep_fuel {
            scaled.set_cheats(crate::game::gameplay::Cheats {
                infinite_fuel: true,
                ..Default::default()
            });
        }
        let mut samples = Vec::new();
        for &buttons in &stretch.buttons {
            let time = scaled.clock().time();
            scaled.tick(
                world,
                buttons,
                buttons,
                crate::game::scaled::clock::STEP_RATE,
            );
            if scaled.clock().time() == time {
                break;
            }
            samples.push(scaled_sample(&scaled));
        }
        stretch.buttons.truncate(samples.len());
        stretch.samples = samples;
    }
    out.stretches.retain(|s| !s.buttons.is_empty());
    out
}

/// Running sums of an error.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Errors {
    pub count: usize,
    pub squares: f64,
    pub max: f64,
    /// Every error, for the percentiles.
    pub all: Vec<f64>,
}

impl Errors {
    fn add(&mut self, error: f64) {
        self.count += 1;
        self.squares += error * error;
        self.max = self.max.max(error);
        self.all.push(error);
    }

    /// The `p`th percentile (0-100).
    pub fn percentile(&self, p: usize) -> f64 {
        let mut all = self.all.clone();
        all.sort_by(f64::total_cmp);
        all.get((all.len().max(1) - 1) * p / 100)
            .copied()
            .unwrap_or(0.0)
    }

    /// Both sums together.
    pub fn merge(&mut self, other: &Errors) {
        self.count += other.count;
        self.squares += other.squares;
        self.max = self.max.max(other.max);
        self.all.extend_from_slice(&other.all);
    }

    pub fn rms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.squares / self.count as f64).sqrt()
        }
    }
}

/// One replay at one rate against the Original.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub rate: u32,
    pub steps: usize,
    /// Distances in pixels, and the facing's difference in facings.
    pub pod: Errors,
    pub camera: Errors,
    pub facing: Errors,
    /// Steps compared inside the budget on every measure.
    pub in_budget: usize,
    /// The replay's first step (1-based) outside the budget.
    pub first_out: Option<usize>,
    /// Steps not compared because Scaled stopped for a scene change or a pause.
    pub stopped_early: usize,
}

impl Run {
    /// Seconds of play (at 30 steps a second) before the first step outside the budget.
    pub fn seconds_in_budget(&self) -> f64 {
        self.first_out.map_or(self.steps, |step| step - 1) as f64 / 30.0
    }
}

/// A difference of 16-bit positions, the shorter way round.
fn wrapped(d: f64) -> f64 {
    (d + 32768.0).rem_euclid(65536.0) - 32768.0
}

/// The facings' difference, the shorter way round the 16 facings.
fn facing_difference(a: u8, b: u8) -> u8 {
    let d = a.wrapping_sub(b) & 15;
    d.min(16 - d)
}

/// Plays every stretch of `reference` in Scaled at `rate` and compares it with the Original.
/// How the replay's buttons reach Scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mapping {
    /// As recorded: Scaled thrusts in the step B is held, a step before the Original
    /// (`docs/scaled.md`).
    #[default]
    AsPlayed,
    /// B one step late, as the Original's thrust acts: the difference left is the rules' own
    /// (`docs/scaled.md`, mapping M).
    ThrustLag,
}

pub fn run(
    world: &World,
    reference: &Reference,
    rate: u32,
    budget: Budget,
    mapping: Mapping,
) -> Run {
    // `SJ_TRACE=FROM-TO` prints both engines' pods at those steps (a debugging aid).
    let trace = std::env::var("SJ_TRACE").ok().and_then(|range| {
        let (from, to) = range.split_once('-')?;
        Some((from.parse::<usize>().ok()?, to.parse::<usize>().ok()?))
    });
    let mut out = Run {
        rate,
        ..Run::default()
    };
    for stretch in &reference.stretches {
        let mut scaled = stretch.start.clone();
        if stretch.keep_fuel {
            scaled.set_cheats(crate::game::gameplay::Cheats {
                infinite_fuel: true,
                ..Default::default()
            });
        }
        let (mut before, mut before_time) = (scaled_sample(&scaled), scaled.clock().time());
        let steps = stretch.buttons.len();
        let mut next = 1;
        while next <= steps {
            // A tick reads the buttons of the step its start falls in.
            let index = (scaled.clock().time() / STEP) as usize;
            let mut buttons = stretch.buttons[index.min(steps - 1)];
            if mapping == Mapping::ThrustLag {
                let earlier = index
                    .checked_sub(1)
                    .map(|i| stretch.buttons[i.min(steps - 1)]);
                let b = earlier.is_some_and(|e| e.has(Buttons::B));
                buttons = Buttons(buttons.0 & !Buttons::B | if b { Buttons::B } else { 0 });
            }
            let rate = if rate == SWITCHING {
                let at = stretch.first_step + next;
                TEST_RATES[(at / SWITCH_STEPS + at / 7) % TEST_RATES.len()]
            } else {
                rate
            };
            scaled.tick(world, buttons, buttons, rate);
            let (now, time) = (scaled_sample(&scaled), scaled.clock().time());
            if time == before_time {
                // Scaled stopped for a scene change or a pause the Original did not make.
                out.stopped_early += steps + 1 - next;
                break;
            }
            while next <= steps && next as u64 * STEP <= time {
                let at = next as u64 * STEP;
                let ours = between(
                    before,
                    now,
                    (at - before_time) as f64 / (time - before_time) as f64,
                );
                let theirs = stretch.samples[next - 1];
                if trace.is_some_and(|(from, to)| {
                    (from..=to).contains(&(stretch.first_step + next - 1))
                }) {
                    eprintln!(
                        "{rate} Hz step {}: pod ({:.2}, {:.2}) v ({:.3}, {:.3}) / original ({:.2}, {:.2}) v ({:.3}, {:.3}), facing {} / {}",
                        stretch.first_step + next - 1,
                        ours.pod.0,
                        ours.pod.1,
                        ours.speed.0,
                        ours.speed.1,
                        theirs.pod.0,
                        theirs.pod.1,
                        theirs.speed.0,
                        theirs.speed.1,
                        ours.facing,
                        theirs.facing,
                    );
                }
                let distance =
                    |a: (f64, f64), b: (f64, f64)| wrapped(a.0 - b.0).hypot(wrapped(a.1 - b.1));
                let pod = distance(ours.pod, theirs.pod);
                let camera = distance(ours.camera, theirs.camera);
                let facing = facing_difference(ours.facing, theirs.facing);
                out.pod.add(pod);
                out.camera.add(camera);
                out.facing.add(f64::from(facing));
                out.steps += 1;
                if pod <= budget.pod && camera <= budget.camera && facing <= budget.facing {
                    out.in_budget += 1;
                } else if out.first_out.is_none() {
                    out.first_out = Some(stretch.first_step + next - 1);
                }
                next += 1;
            }
            (before, before_time) = (now, time);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::gameplay::Seed;

    /// Scaled at a whole step a tick, from the start of level 0 with no input.
    fn falling(world: &World, steps: usize) -> Reference {
        let game = Gameplay::new(world, 0, Seed::default());
        let mut stretch = Stretch::new(1, &game, &Replay::default());
        stretch.buttons = vec![Buttons(0); steps];
        stretch.samples = vec![Sample::default(); steps];
        let reference = Reference {
            stretches: vec![stretch],
            stopped: None,
        };
        at_step_rate(world, &reference)
    }

    #[test]
    fn every_step_is_read_once_at_every_rate() {
        let Some(world) = crate::tools::test_world() else {
            return;
        };
        let reference = falling(world, 95);
        for rate in TEST_RATES {
            let run = run(world, &reference, rate, BUDGET, Mapping::AsPlayed);
            assert_eq!(run.steps, 95, "{rate} Hz");
        }
        // At the step rate it is the reference itself.
        let run = run(world, &reference, 30, BUDGET, Mapping::AsPlayed);
        assert_eq!(run.in_budget, 95);
        assert_eq!(
            (run.pod.max, run.camera.max, run.facing.max),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(run.seconds_in_budget(), 95.0 / 30.0);
    }

    #[test]
    fn the_first_step_outside_the_budget_is_reported() {
        let Some(world) = crate::tools::test_world() else {
            return;
        };
        let mut reference = falling(world, 60);
        let stretch = &mut reference.stretches[0];
        stretch.first_step = 11;
        for sample in &mut stretch.samples[20..] {
            sample.pod.0 += 3.0;
        }
        stretch.samples[30].facing = stretch.samples[30].facing.wrapping_add(15);
        let run = run(world, &reference, 30, BUDGET, Mapping::AsPlayed);
        assert_eq!(run.first_out, Some(31));
        assert_eq!(run.in_budget, 20);
        assert_eq!(run.pod.max, 3.0);
        // 15 facings one way is 1 the other.
        assert_eq!(run.facing.max, 1.0);
    }
}
