//! The Scaled engine's tick rate chosen at run time from the display (`docs/scaled.md`):
//! the frame intervals are measured, a vsync-locked display gets its own refresh rate (or a
//! whole fraction of it at 200 Hz or under) with a phase lock that gives every frame a whole
//! number of refreshes, and an unlocked one the smallest listed rate that keeps up. Changes are
//! debounced, and a game that falls behind steps down.
//!
//! Platform-free: the program passes each presented frame's time and how long the frame's work
//! took. Times are nanoseconds.

use std::collections::VecDeque;

/// The rates Auto chooses from, and the fixed rates the port menu offers.
pub const LIST: [u32; 8] = [60, 75, 90, 100, 120, 144, 165, 200];
/// Display rates above the cap that run at a whole fraction of themselves.
const HIGH: [u32; 2] = [240, 360];
const MIN: u32 = 60;
const MAX: u32 = 200;

const SECOND: u64 = 1_000_000_000;
const MS: u64 = 1_000_000;
/// Samples longer than this (a sleep, a hidden tab) are not measured.
const LONGEST_SAMPLE: u64 = 250 * MS;
/// No frame advances the game by more than this.
const LONGEST_STEP: u64 = 100 * MS;
const WINDOW: u64 = SECOND;
const LONG_WINDOW: u64 = 4 * SECOND;
const ESTIMATE_EVERY: u64 = 100 * MS;
const FIRST_ESTIMATE: u64 = SECOND / 2;
/// The phase lock gives up after this many frames off the grid in a row.
const SNAP_FAILURES: u32 = 6;

/// Auto, or a rate the player chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Auto,
    Fixed(u32),
}

/// What the controller decided for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Ticks to run before drawing.
    pub ticks: u32,
    /// Draw between the last two ticks at this blend (0 the earlier, 1 the later); `None`
    /// draws the last tick as it is.
    pub blend: Option<f32>,
}

/// A rate waiting to be taken: the value and since when it has been asked for.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    rate: u32,
    since: u64,
}

#[derive(Debug, Clone)]
pub struct Controller {
    choice: Choice,
    rate: u32,
    /// (time, interval, work) of the frames of the last second, and the intervals of the last
    /// four.
    window: VecDeque<(u64, u64, u64)>,
    long: VecDeque<(u64, u64)>,
    /// The refresh grid: its last point and its period, while locked.
    grid: u64,
    period: Option<u64>,
    /// The display's nominal rate while locked.
    nominal: Option<u32>,
    /// The phase errors of the snapped frames, in thousandths of the period.
    residues: VecDeque<(u64, i64)>,
    failures: u32,
    /// Time accumulated toward the next tick: in 1/nominal-tick units while locked, in
    /// nanosecond-ticks (ns × rate) otherwise.
    owed: u64,
    candidate: Option<Candidate>,
    last: Option<u64>,
    start: Option<u64>,
    last_estimate: u64,
    estimated: bool,
    last_step_down: Option<u64>,
    blocks: u32,
    raise_blocked_until: u64,
}

impl Controller {
    pub fn new(choice: Choice) -> Controller {
        let rate = match choice {
            Choice::Auto => MIN,
            Choice::Fixed(rate) => rate.clamp(MIN, MAX),
        };
        Controller {
            choice,
            rate,
            window: VecDeque::new(),
            long: VecDeque::new(),
            grid: 0,
            period: None,
            nominal: None,
            residues: VecDeque::new(),
            failures: 0,
            owed: 0,
            candidate: None,
            last: None,
            start: None,
            last_estimate: 0,
            estimated: false,
            last_step_down: None,
            blocks: 0,
            raise_blocked_until: 0,
        }
    }

    /// The tick rate now, in Hz.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn choose(&mut self, choice: Choice) {
        if choice != self.choice {
            self.choice = choice;
            if let Choice::Fixed(rate) = choice {
                self.set_rate(rate.clamp(MIN, MAX));
            }
            self.candidate = None;
        }
    }

    /// One presented frame at `now`, whose work (logic and drawing) took `work`; `vsync`
    /// whether the display waits for the refresh. Returns the ticks to run and how to draw.
    pub fn frame(&mut self, now: u64, work: u64, vsync: bool) -> Frame {
        let start = *self.start.get_or_insert(now);
        let Some(last) = self.last.replace(now) else {
            self.grid = now;
            return Frame {
                ticks: 0,
                blend: None,
            };
        };
        let interval = now.saturating_sub(last);
        if interval > 0 && interval <= LONGEST_SAMPLE {
            self.window.push_back((now, interval, work));
            self.long.push_back((now, interval));
        }
        while self.window.front().is_some_and(|&(t, ..)| now - t > WINDOW) {
            self.window.pop_front();
        }
        while self
            .long
            .front()
            .is_some_and(|&(t, _)| now - t > LONG_WINDOW)
        {
            self.long.pop_front();
        }
        while self
            .residues
            .front()
            .is_some_and(|&(t, _)| now - t > WINDOW)
        {
            self.residues.pop_front();
        }
        let refreshes = if vsync { self.snap(now) } else { None };
        if self.failures >= SNAP_FAILURES {
            // Relock from the newest samples alone.
            while self.window.len() > 6 {
                self.window.pop_front();
            }
            self.failures = 0;
            self.estimate(now, start, vsync);
        } else if now - self.last_estimate >= ESTIMATE_EVERY {
            self.estimate(now, start, vsync);
        }
        let ticks = match (refreshes, self.nominal) {
            (Some(n), Some(nominal)) => {
                self.owed += u64::from(n) * u64::from(self.rate);
                let ticks = self.owed / u64::from(nominal);
                self.owed %= u64::from(nominal);
                ticks
            }
            _ => {
                if self.nominal.is_some() {
                    // Lost the lock this frame: carry on in nanoseconds.
                    self.owed = 0;
                    self.nominal = None;
                }
                self.owed += interval.min(LONGEST_STEP) * u64::from(self.rate);
                let ticks = self.owed / SECOND;
                self.owed %= SECOND;
                ticks
            }
        };
        Frame {
            ticks: ticks as u32,
            blend: (!self.holds()).then(|| self.fraction()),
        }
    }

    /// Whether drawing holds the last tick: locked, with the rate a multiple or a whole
    /// fraction of the display's.
    fn holds(&self) -> bool {
        self.nominal
            .is_some_and(|h| self.rate.is_multiple_of(h) || h.is_multiple_of(self.rate))
    }

    /// How far the time owed is toward the next tick.
    fn fraction(&self) -> f32 {
        match self.nominal {
            Some(h) => self.owed as f32 / h as f32,
            None => self.owed as f32 / SECOND as f32,
        }
    }

    /// Places `now` on the refresh grid: the whole number of refreshes since the last frame,
    /// or `None` (and a relock) when it lies off the grid.
    fn snap(&mut self, now: u64) -> Option<u32> {
        let Some(period) = self.period else {
            self.grid = now;
            return None;
        };
        let gap = now as i64 - self.grid as i64;
        if gap <= 0 || gap as u64 > LONGEST_STEP {
            self.grid = now;
            return None;
        }
        let p = period as i64;
        let n = ((gap + p / 2) / p).max(1);
        let error = gap - n * p;
        self.residues.push_back((now, error * 1000 / p));
        if error.abs() * 100 > 35 * p {
            self.grid = now;
            self.failures += 1;
            return None;
        }
        // 5% of the error corrects the phase.
        self.grid = (self.grid as i64 + n * p + error / 20) as u64;
        self.failures = 0;
        Some(n as u32)
    }

    fn estimate(&mut self, now: u64, start: u64, vsync: bool) {
        self.last_estimate = now;
        if self.window.len() < 5 {
            return;
        }
        let mut intervals: Vec<u64> = self.window.iter().map(|&(_, dt, _)| dt).collect();
        intervals.sort_unstable();
        let at = |q: usize| intervals[(intervals.len() - 1) * q / 100];
        let (median, p10, p90) = (at(50), at(10), at(90));
        let tight = (p90 - p10) * 10 <= 6 * median;
        let residue_rms = {
            let n = self.residues.len().max(1) as i64;
            let sum: i64 = self.residues.iter().map(|&(_, r)| r * r).sum();
            ((sum / n) as f64).sqrt()
        };
        let hz = SECOND as f64 / median as f64;
        let locking = vsync && tight;
        self.period = locking.then_some(median);
        let locked = locking && residue_rms < 200.0;
        let candidate = if locked {
            let nominal = LIST
                .iter()
                .chain(&HIGH)
                .copied()
                .find(|&l| (hz - f64::from(l)).abs() / f64::from(l) < 0.015)
                .unwrap_or(hz.round() as u32)
                .max(1);
            if self.nominal != Some(nominal) {
                self.owed = 0;
            }
            self.nominal = Some(nominal);
            if nominal > MAX {
                nominal / nominal.div_ceil(MAX)
            } else if nominal < MIN {
                nominal * MIN.div_ceil(nominal)
            } else {
                nominal
            }
        } else {
            if self.nominal.take().is_some() {
                self.owed = 0;
            }
            let mut long: Vec<u64> = self.long.iter().map(|&(_, dt)| dt).collect();
            long.sort_unstable();
            let fastest = long[(long.len() - 1) / 10].max(1);
            let peak = SECOND as f64 / fastest as f64;
            LIST.iter()
                .copied()
                .find(|&l| f64::from(l) >= 0.97 * peak)
                .unwrap_or(MAX)
        };
        if self.choice != Choice::Auto {
            return;
        }
        let (work, spent) = self
            .window
            .iter()
            .fold((0, 0), |(w, s), &(_, dt, work)| (w + work, s + dt));
        let busy = work * 10 > spent * 8;
        if busy && self.rate > MIN && self.last_step_down.is_none_or(|t| now - t >= 2 * SECOND) {
            let lower = LIST.iter().rev().copied().find(|&l| l < self.rate);
            if let Some(lower) = lower {
                self.set_rate(lower);
                self.last_step_down = Some(now);
                self.raise_blocked_until = now + 20 * SECOND * 3u64.pow(self.blocks);
                self.blocks = (self.blocks + 1).min(3);
                self.candidate = None;
            }
            return;
        }
        if !self.estimated && now - start >= FIRST_ESTIMATE {
            self.estimated = true;
            self.set_rate(candidate);
            return;
        }
        let raise = candidate > self.rate;
        let ignored = candidate == self.rate
            || (!locked && candidate.abs_diff(self.rate) * 100 < 12 * self.rate)
            || (raise && now < self.raise_blocked_until);
        if ignored {
            self.candidate = None;
            return;
        }
        let since = match self.candidate {
            Some(c) if c.rate == candidate => c.since,
            _ => {
                self.candidate = Some(Candidate {
                    rate: candidate,
                    since: now,
                });
                now
            }
        };
        let wait = match (locked, raise) {
            (true, false) => SECOND,
            (true, true) => 2 * SECOND,
            (false, false) => 6 * SECOND,
            (false, true) => 3 * SECOND / 2,
        };
        if now - since >= wait {
            self.set_rate(candidate);
            self.candidate = None;
        }
    }

    /// Changes the rate; the time owed toward the next tick keeps its fraction of a tick.
    fn set_rate(&mut self, rate: u32) {
        if rate != self.rate && self.nominal.is_none() {
            self.owed = self.owed * u64::from(rate) / u64::from(self.rate);
        }
        self.rate = rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `seconds` of frames presented every `period` ns (vsync on) and returns the
    /// controller and the ticks of each frame after the first second.
    fn run(controller: &mut Controller, period: u64, seconds: u64, vsync: bool) -> Vec<u32> {
        let frames = seconds * SECOND / period;
        let mut ticks = Vec::new();
        for i in 0..frames {
            let frame = controller.frame(i * period, period / 10, vsync);
            if i * period > SECOND {
                ticks.push(frame.ticks);
            }
        }
        ticks
    }

    #[test]
    fn a_locked_display_runs_at_its_own_rate_with_one_tick_a_frame() {
        for hz in [60, 75, 120, 144, 165] {
            let mut controller = Controller::new(Choice::Auto);
            let ticks = run(&mut controller, SECOND / hz, 4, true);
            assert_eq!(controller.rate(), hz as u32, "{hz} Hz");
            assert!(ticks.iter().all(|&n| n == 1), "{hz} Hz: {ticks:?}");
        }
    }

    #[test]
    fn a_fast_display_runs_at_a_whole_fraction_and_holds() {
        let mut controller = Controller::new(Choice::Auto);
        let ticks = run(&mut controller, SECOND / 240, 4, true);
        assert_eq!(controller.rate(), 120);
        // One tick every second refresh, drawn as it is.
        let total = ticks.iter().sum::<u32>() as usize;
        assert!(total.abs_diff(ticks.len() / 2) <= 1, "{total}");
        assert!(ticks.iter().all(|&n| n <= 1));
        let next = 4 * SECOND / (SECOND / 240) * (SECOND / 240);
        assert_eq!(controller.frame(next, 0, true).blend, None);
    }

    #[test]
    fn a_slightly_slow_panel_still_gets_a_tick_every_frame() {
        // 59.94 Hz: no empty or doubled frame over a minute.
        let mut controller = Controller::new(Choice::Auto);
        let ticks = run(&mut controller, 16_683_350, 60, true);
        assert_eq!(controller.rate(), 60);
        assert!(ticks.iter().all(|&n| n == 1));
    }

    #[test]
    fn unlocked_picks_the_smallest_rate_that_keeps_up_and_interpolates() {
        let mut controller = Controller::new(Choice::Auto);
        run(&mut controller, SECOND / 300, 4, false);
        assert_eq!(controller.rate(), 200);
        assert!(controller.frame(5 * SECOND, 0, false).blend.is_some());
        let mut controller = Controller::new(Choice::Auto);
        run(&mut controller, SECOND / 100, 4, false);
        assert_eq!(controller.rate(), 100);
    }

    #[test]
    fn a_sleep_does_not_make_a_burst_of_ticks() {
        let mut controller = Controller::new(Choice::Auto);
        run(&mut controller, SECOND / 60, 2, true);
        let frame = controller.frame(40 * SECOND, 0, true);
        assert!(frame.ticks <= 6, "{}", frame.ticks);
    }

    #[test]
    fn a_fixed_rate_stays_and_interpolates_when_it_does_not_divide_the_display() {
        let mut controller = Controller::new(Choice::Fixed(100));
        run(&mut controller, SECOND / 144, 3, true);
        assert_eq!(controller.rate(), 100);
        assert!(controller.frame(4 * SECOND, 0, true).blend.is_some());
    }

    #[test]
    fn falling_behind_steps_down() {
        let mut controller = Controller::new(Choice::Auto);
        run(&mut controller, SECOND / 144, 3, true);
        assert_eq!(controller.rate(), 144);
        let period = SECOND / 144;
        for i in 0..(2 * SECOND / period) {
            controller.frame(3 * SECOND + i * period, period * 9 / 10, true);
        }
        assert!(controller.rate() < 144);
    }
}
