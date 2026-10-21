//! From channel levels to samples: the mixer, an average over each sample period, and the
//! console's output filters.

use crate::apu::{CPU_HZ, mix};
use std::f64::consts::PI;

/// The channels' levels in one CPU cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Levels {
    pub pulse1: u8,
    pub pulse2: u8,
    pub triangle: u8,
    pub noise: u8,
    pub dmc: u8,
}

/// A first-order filter, high-pass or low-pass, at the sample rate.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Filter {
    high_pass: bool,
    alpha: f32,
    previous_in: f32,
    previous_out: f32,
}

impl Filter {
    fn new(high_pass: bool, cutoff: f64, sample_rate: u32) -> Filter {
        let rc = 1.0 / (2.0 * PI * cutoff);
        let dt = 1.0 / f64::from(sample_rate);
        let alpha = if high_pass {
            rc / (rc + dt)
        } else {
            dt / (rc + dt)
        };
        Filter {
            high_pass,
            alpha: alpha as f32,
            previous_in: 0.0,
            previous_out: 0.0,
        }
    }

    fn apply(&mut self, input: f32) -> f32 {
        let out = if self.high_pass {
            self.alpha * (self.previous_out + input - self.previous_in)
        } else {
            self.previous_out + self.alpha * (input - self.previous_out)
        };
        self.previous_in = input;
        self.previous_out = out;
        out
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Output {
    sample_rate: u32,
    cycles_per_sample: f64,
    /// Cycles left until the current sample is complete.
    until_sample: f64,
    sum: f64,
    cycles: u32,
    levels: Levels,
    level: f32,
    filters: [Filter; 3],
}

impl Output {
    pub(crate) fn new(sample_rate: u32) -> Output {
        let sample_rate = sample_rate.max(1);
        let cycles_per_sample = CPU_HZ / f64::from(sample_rate);
        Output {
            sample_rate,
            cycles_per_sample,
            until_sample: cycles_per_sample,
            sum: 0.0,
            cycles: 0,
            levels: Levels::default(),
            level: 0.0,
            filters: [
                Filter::new(true, 90.0, sample_rate),
                Filter::new(true, 440.0, sample_rate),
                Filter::new(false, 14_000.0, sample_rate),
            ],
        }
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn add_cycle(&mut self, levels: Levels, out: &mut Vec<f32>) {
        if levels != self.levels {
            self.levels = levels;
            self.level = mix(
                levels.pulse1,
                levels.pulse2,
                levels.triangle,
                levels.noise,
                levels.dmc,
            );
        }
        self.sum += f64::from(self.level);
        self.cycles += 1;
        self.until_sample -= 1.0;
        if self.until_sample <= 0.0 {
            self.until_sample += self.cycles_per_sample;
            let mut sample = (self.sum / f64::from(self.cycles)) as f32;
            self.sum = 0.0;
            self.cycles = 0;
            for filter in &mut self.filters {
                sample = filter.apply(sample);
            }
            out.push(sample);
        }
    }
}
