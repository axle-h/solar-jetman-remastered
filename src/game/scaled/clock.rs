//! The Scaled engine's one clock (`docs/scaled.md`).
//!
//! Game time is a `u64` in Q32 logic steps of the original (one step is 1/30 s). A tick at rate R
//! advances it by `ceil(2^32 * 30 / R)`: rounding up means a deadline is never reached early (with
//! rounding to nearest, 75 Hz fires a 48-step deadline a tick late). Steps are `time >> 32` and
//! frames of the NES's clock `time >> 31`. A per-step or per-frame event fires once in the tick that
//! crosses it, so events come 0 to 1 tick late and are never lost.

use std::ops::Range;

/// The slowest and fastest tick rates the program runs, in Hz.
pub const MIN_RATE: u32 = 60;
pub const MAX_RATE: u32 = 200;
/// The Original's step rate: a tick of a whole step, where every rule is the Original's (the
/// checks run it; the program does not).
pub const STEP_RATE: u32 = 30;

/// One logic step of the original in game time.
pub const STEP: u64 = 1 << 32;

/// The game time one tick at `rate` Hz adds, rounded up.
pub fn tick_length(rate: u32) -> u64 {
    (30 * STEP).div_ceil(u64::from(rate.clamp(STEP_RATE, MAX_RATE)))
}

/// Game time since the level started, in Q32 steps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Clock {
    time: u64,
}

impl Clock {
    pub fn time(self) -> u64 {
        self.time
    }

    /// Advances the clock by one tick at `rate` Hz.
    pub fn advance(&mut self, rate: u32) -> Tick {
        let start = self.time;
        self.time += tick_length(rate);
        Tick {
            start,
            end: self.time,
        }
    }

    /// Whole steps since the level started.
    pub fn steps(self) -> u64 {
        self.time >> 32
    }

    /// Whole frames of the NES's clock since the level started.
    pub fn frames(self) -> u64 {
        self.time >> 31
    }
}

/// One tick: game time from `start` to `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    pub start: u64,
    pub end: u64,
}

impl Tick {
    /// The tick's length in Q16 steps, f in the results files. It is taken from the clock's Q16
    /// readings, so the fractions of successive ticks add up exactly.
    pub fn fraction(self) -> u32 {
        ((self.end >> 16) - (self.start >> 16)) as u32
    }

    /// The step events this tick crosses: step k fires in the tick where `start < k * STEP <=
    /// end`.
    pub fn steps(self) -> Range<u64> {
        (self.start >> 32) + 1..(self.end >> 32) + 1
    }

    /// The frames of the NES's clock that start in this tick: frame k starts at game time
    /// `k * STEP / 2`, from frame 0 at the level start, so the logic of step n (its A and B
    /// passes, frames 2n - 2 and 2n - 1) runs in the step's first half.
    pub fn frames(self) -> Range<u64> {
        let half = STEP / 2;
        self.start.div_ceil(half)..self.end.div_ceil(half)
    }

    /// Whether the tick reaches `deadline`: it is the first tick with `end >= deadline`.
    pub fn reaches(self, deadline: Deadline) -> bool {
        self.start < deadline.0 && deadline.0 <= self.end
    }
}

/// A moment of game time that a timer waits for. Game time is saved, so a deadline is
/// rate-free as it stands: a state saved at one rate expires it at the same game time at another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deadline(pub u64);

impl Deadline {
    /// The deadline `steps` whole steps after `now`.
    pub fn after(now: u64, steps: u32) -> Deadline {
        Deadline(now + u64::from(steps) * STEP)
    }

    /// Whether game time `now` has reached it.
    pub fn passed(self, now: u64) -> bool {
        now >= self.0
    }

    /// The game time left until it, 0 once it has passed.
    pub fn remaining(self, now: u64) -> u64 {
        self.0.saturating_sub(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_RATES: [u32; 10] = [60, 75, 90, 100, 120, 137, 144, 165, 200, 61];

    #[test]
    fn a_tick_is_never_short() {
        for rate in STEP_RATE..=MAX_RATE {
            let length = tick_length(rate);
            assert!(u128::from(length) * u128::from(rate) >= 30 * u128::from(STEP));
            assert!(u128::from(length - 1) * u128::from(rate) < 30 * u128::from(STEP));
        }
        assert_eq!(tick_length(60), STEP / 2);
        assert_eq!(tick_length(120), STEP / 4);
        // Out of range rates are held at the ends.
        assert_eq!(tick_length(30), STEP);
        assert_eq!(tick_length(10), STEP);
        assert_eq!(tick_length(1000), tick_length(MAX_RATE));
    }

    #[test]
    fn one_second_of_ticks_crosses_thirty_steps_and_sixty_frames() {
        for rate in TEST_RATES {
            let mut clock = Clock::default();
            let (mut steps, mut frames, mut fraction) = (0, 0, 0u64);
            for _ in 0..rate {
                let tick = clock.advance(rate);
                steps += tick.steps().count();
                frames += tick.frames().count();
                fraction += u64::from(tick.fraction());
            }
            // The ticks are rounded up: the next frame may start just inside the second.
            assert_eq!(steps, 30, "{rate} Hz");
            assert!((60..=61).contains(&frames), "{rate} Hz");
            assert_eq!(frames as u64, clock.time().div_ceil(STEP / 2));
            assert_eq!(clock.steps(), 30);
            // The Q16 fractions add up to the clock's own Q16 reading.
            assert_eq!(fraction, clock.time() >> 16);
        }
    }

    #[test]
    fn each_event_fires_once_and_in_order() {
        for rate in TEST_RATES {
            let mut clock = Clock::default();
            let mut next_step = 1;
            let mut next_frame = 0;
            for _ in 0..10 * rate {
                let tick = clock.advance(rate);
                for k in tick.steps() {
                    assert_eq!(k, next_step);
                    next_step += 1;
                }
                for k in tick.frames() {
                    assert_eq!(k, next_frame);
                    next_frame += 1;
                }
                // At most one step per tick at 60 Hz and up.
                assert!(tick.steps().count() <= 1);
            }
        }
    }

    #[test]
    fn a_deadline_is_reached_by_exactly_one_tick_never_early() {
        // misc.md 4.1: with rounding up a whole number of ticks always reaches it, so 48 steps
        // at 75 Hz (120 ticks) expire on time.
        for rate in TEST_RATES {
            for steps in [8, 11, 48, 96, 128] {
                let deadline = Deadline::after(0, steps);
                let mut clock = Clock::default();
                let mut reached = Vec::new();
                for n in 1..=(steps * rate / 30 + 2) {
                    let tick = clock.advance(rate);
                    if tick.reaches(deadline) {
                        reached.push(n);
                        assert!(deadline.passed(tick.end));
                        assert!(!deadline.passed(tick.start));
                    }
                }
                assert_eq!(reached.len(), 1);
                // Never early: the tick reaching it ends at or after the ideal time, and less
                // than a tick later.
                let n = reached[0];
                assert!(n * 30 >= steps * rate && (n - 1) * 30 < steps * rate);
                if (steps * rate) % 30 == 0 {
                    assert_eq!(reached[0], steps * rate / 30, "{steps} steps at {rate} Hz");
                }
            }
        }
    }

    #[test]
    fn a_deadline_keeps_its_game_time_whatever_the_rate() {
        let deadline = Deadline::after(3 * STEP + 12345, 48);
        assert_eq!(deadline.remaining(3 * STEP + 12345), 48 * STEP);
        assert_eq!(deadline.remaining(deadline.0 + 1), 0);
    }
}
