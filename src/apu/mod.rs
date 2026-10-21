//! A 2A03-style sound synthesiser, written from the NESdev wiki's public documentation of the
//! NES APU ("APU", "APU Pulse", "APU Triangle", "APU Noise", "APU Envelope", "APU Sweep",
//! "APU Length Counter", "APU Frame Counter", "APU Mixer").
//!
//! The caller writes registers ($4000-$4017) and runs the chip for a number of CPU cycles; the
//! chip appends mono `f32` samples at the sample rate chosen when it was made. Writes take effect
//! at the cycle they are made, so a caller with timestamps runs up to each write's cycle first
//! ([`Apu::run_frame`] does that for one video frame).
//!
//! Channels: the two pulses (envelope, sweep, length counter), the triangle (linear and length
//! counters) and the noise channel (envelope, length counter, both feedback modes). Of the DMC
//! only the output level ($4011) is modelled: Solar Jetman never plays samples, but the level it
//! leaves there lowers the triangle and noise through the non-linear mixer
//! (`docs/spec/sound.md`, section 1).
//!
//! Output is the standard non-linear mix, averaged over each sample period, then the console's
//! two high-pass filters (90 Hz, 440 Hz) and its low-pass filter (14 kHz), all first-order.
#![forbid(unsafe_code)]

mod channels;
mod output;

use channels::{Noise, Pulse, Triangle};
use output::Output;

/// The NTSC CPU clock in Hz (21.477272 MHz master clock ÷ 12).
pub const CPU_HZ: f64 = 236.25e6 / 11.0 / 12.0;
/// The NTSC video frame rate in Hz.
pub const FRAME_HZ: f64 = 60.0988;
/// CPU cycles in one video frame, on average.
pub const CYCLES_PER_FRAME: f64 = CPU_HZ / FRAME_HZ;

/// One register write with its time, in CPU cycles from the start of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedWrite {
    pub cycle: u32,
    pub register: u16,
    pub value: u8,
}

/// Frame counter sequence lengths and step times in CPU cycles ("APU Frame Counter", NTSC).
const FOUR_STEP: [u32; 4] = [7457, 14913, 22371, 29829];
const FOUR_STEP_LENGTH: u32 = 29830;
const FIVE_STEP: [u32; 4] = [7457, 14913, 22371, 37281];
const FIVE_STEP_LENGTH: u32 = 37282;

/// The APU.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Apu {
    pulses: [Pulse; 2],
    triangle: Triangle,
    noise: Noise,
    /// DMC output level, 0-127 ($4011).
    dmc_level: u8,
    five_step: bool,
    /// CPU cycles since the frame counter's sequence started.
    sequence_cycle: u32,
    /// Odd or even CPU cycle: pulse and noise timers run on every other one (APU cycles).
    odd_cycle: bool,
    output: Output,
    /// Fractional CPU cycles carried between frames by [`Apu::run_frame`].
    frame_remainder: f64,
}

impl Apu {
    /// A chip at power-on, producing `sample_rate` samples a second.
    pub fn new(sample_rate: u32) -> Apu {
        Apu {
            pulses: [Pulse::new(true), Pulse::new(false)],
            triangle: Triangle::default(),
            noise: Noise::default(),
            dmc_level: 0,
            five_step: false,
            sequence_cycle: 0,
            odd_cycle: false,
            output: Output::new(sample_rate),
            frame_remainder: 0.0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.output.sample_rate()
    }

    /// Writes `value` to `register` ($4000-$4017; anything else is ignored).
    pub fn write(&mut self, register: u16, value: u8) {
        match register {
            0x4000..=0x4007 => {
                let pulse = &mut self.pulses[usize::from((register - 0x4000) / 4)];
                pulse.write(register & 3, value);
            }
            0x4008..=0x400B => self.triangle.write(register & 3, value),
            0x400C..=0x400F => self.noise.write(register & 3, value),
            0x4011 => self.dmc_level = value & 0x7F,
            0x4015 => {
                self.pulses[0].set_enabled(value & 1 != 0);
                self.pulses[1].set_enabled(value & 2 != 0);
                self.triangle.set_enabled(value & 4 != 0);
                self.noise.set_enabled(value & 8 != 0);
            }
            0x4017 => {
                self.five_step = value & 0x80 != 0;
                self.sequence_cycle = 0;
                // Selecting the 5-step sequence clocks both units at once. (The hardware does
                // this, and the reset, 3 or 4 cycles after the write; ignored here.)
                if self.five_step {
                    self.quarter_frame();
                    self.half_frame();
                }
            }
            // $4010, $4012, $4013: DMC sample playback, not modelled; $4014 is not the APU's.
            _ => {}
        }
    }

    /// Runs `cycles` CPU cycles, appending the samples that complete to `out`.
    pub fn run(&mut self, cycles: u32, out: &mut Vec<f32>) {
        for _ in 0..cycles {
            self.clock_frame_counter();
            self.triangle.clock_timer();
            if self.odd_cycle {
                self.pulses[0].clock_timer();
                self.pulses[1].clock_timer();
            }
            self.noise.clock_timer();
            self.odd_cycle = !self.odd_cycle;
            self.output.add_cycle(self.levels(), out);
        }
    }

    /// Runs one video frame (29 780 or 29 781 CPU cycles, averaging [`CYCLES_PER_FRAME`]),
    /// applying `writes` at their cycles; writes past the frame's end apply at its end.
    pub fn run_frame(&mut self, writes: &[TimedWrite], out: &mut Vec<f32>) {
        let exact = CYCLES_PER_FRAME + self.frame_remainder;
        let length = exact.floor() as u32;
        self.frame_remainder = exact - f64::from(length);
        let mut done = 0;
        let mut sorted = writes.to_vec();
        sorted.sort_by_key(|write| write.cycle);
        for write in sorted {
            let at = write.cycle.min(length);
            self.run(at - done, out);
            done = at;
            self.write(write.register, write.value);
        }
        self.run(length - done, out);
    }

    /// The four channels' current output levels (0-15 each): pulse 1, pulse 2, triangle,
    /// noise.
    pub fn channel_levels(&self) -> [u8; 4] {
        let levels = self.levels();
        [levels.pulse1, levels.pulse2, levels.triangle, levels.noise]
    }

    /// The length counters of pulse 1, pulse 2, triangle and noise.
    pub fn length_counters(&self) -> [u8; 4] {
        [
            self.pulses[0].length(),
            self.pulses[1].length(),
            self.triangle.length(),
            self.noise.length(),
        ]
    }

    fn levels(&self) -> output::Levels {
        output::Levels {
            pulse1: self.pulses[0].output(),
            pulse2: self.pulses[1].output(),
            triangle: self.triangle.output(),
            noise: self.noise.output(),
            dmc: self.dmc_level,
        }
    }

    fn clock_frame_counter(&mut self) {
        self.sequence_cycle += 1;
        let (steps, length) = if self.five_step {
            (FIVE_STEP, FIVE_STEP_LENGTH)
        } else {
            (FOUR_STEP, FOUR_STEP_LENGTH)
        };
        if let Some(step) = steps.iter().position(|&at| at == self.sequence_cycle) {
            self.quarter_frame();
            if step == 1 || step == 3 {
                self.half_frame();
            }
        }
        if self.sequence_cycle >= length {
            self.sequence_cycle = 0;
        }
    }

    /// Envelopes and the triangle's linear counter.
    fn quarter_frame(&mut self) {
        self.pulses[0].quarter_frame();
        self.pulses[1].quarter_frame();
        self.triangle.quarter_frame();
        self.noise.quarter_frame();
    }

    /// Length counters and sweep units.
    fn half_frame(&mut self) {
        self.pulses[0].half_frame();
        self.pulses[1].half_frame();
        self.triangle.half_frame();
        self.noise.half_frame();
    }
}

/// The non-linear mixer ("APU Mixer"): pulse levels 0-15, triangle 0-15, noise 0-15, DMC
/// 0-127, giving 0.0 to about 1.0.
pub fn mix(pulse1: u8, pulse2: u8, triangle: u8, noise: u8, dmc: u8) -> f32 {
    let pulses = f32::from(pulse1) + f32::from(pulse2);
    let pulse_out = if pulses == 0.0 {
        0.0
    } else {
        95.88 / (8128.0 / pulses + 100.0)
    };
    let tnd = f32::from(triangle) / 8227.0 + f32::from(noise) / 12241.0 + f32::from(dmc) / 22638.0;
    let tnd_out = if tnd == 0.0 {
        0.0
    } else {
        159.79 / (1.0 / tnd + 100.0)
    };
    pulse_out + tnd_out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts the rises of one channel's level over `cycles` CPU cycles.
    fn rises(apu: &mut Apu, channel: usize, cycles: u32) -> u32 {
        let mut out = Vec::new();
        let mut last = apu.channel_levels()[channel];
        let mut count = 0;
        for _ in 0..cycles {
            apu.run(1, &mut out);
            let level = apu.channel_levels()[channel];
            if level > last {
                count += 1;
            }
            last = level;
        }
        count
    }

    fn pulse_apu(period: u16) -> Apu {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x0F);
        apu.write(0x4000, 0xBF); // duty 2, halt, constant volume 15
        apu.write(0x4002, (period & 0xFF) as u8);
        apu.write(0x4003, 0x08 | (period >> 8) as u8);
        apu
    }

    #[test]
    fn pulse_frequency_follows_the_period() {
        // f = CPU / (16 (t + 1)): t = $1FD gives 218.5 Hz, so 218 or 219 cycles in a second.
        let mut apu = pulse_apu(0x1FD);
        let count = rises(&mut apu, 0, CPU_HZ as u32);
        let expected = CPU_HZ / (16.0 * (0x1FD as f64 + 1.0));
        assert!(
            (f64::from(count) - expected).abs() <= 1.0,
            "{count} vs {expected}"
        );
        assert_eq!(apu.channel_levels()[0] % 15, 0);
    }

    #[test]
    fn pulse_periods_below_8_are_muted() {
        let mut apu = pulse_apu(7);
        assert_eq!(rises(&mut apu, 0, 10_000), 0);
        let mut apu = pulse_apu(8);
        assert!(rises(&mut apu, 0, 10_000) > 0);
    }

    #[test]
    fn duty_cycles() {
        for (duty, high_steps) in [(0u8, 1u32), (1, 2), (2, 4), (3, 6)] {
            let mut apu = pulse_apu(100);
            apu.write(0x4000, duty << 6 | 0x3F);
            let mut out = Vec::new();
            let mut high = 0;
            let cycles = 16 * 101 * 10;
            for _ in 0..cycles {
                apu.run(1, &mut out);
                high += u32::from(apu.channel_levels()[0] > 0);
            }
            assert_eq!(high, cycles / 8 * high_steps, "duty {duty}");
        }
    }

    #[test]
    fn length_counter_silences_and_halt_holds() {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x01);
        apu.write(0x4000, 0x9F); // halt clear, constant volume 15
        apu.write(0x4002, 0x00);
        apu.write(0x4003, 0x01); // length index 0: 10 half frames; period $100
        assert_eq!(apu.length_counters()[0], 10);
        let mut out = Vec::new();
        // 4-step sequence: two half frames per 29 830 cycles.
        apu.run(4 * FOUR_STEP_LENGTH, &mut out);
        assert_eq!(apu.length_counters()[0], 2);
        apu.run(FOUR_STEP_LENGTH, &mut out);
        assert_eq!(apu.length_counters()[0], 0);
        assert_eq!(rises(&mut apu, 0, 5000), 0);
        // Halted, the counter keeps its value.
        apu.write(0x4000, 0xBF);
        apu.write(0x4003, 0x09); // index 1: 254
        apu.run(3 * FOUR_STEP_LENGTH, &mut out);
        assert_eq!(apu.length_counters()[0], 254);
        // Disabling the channel clears it; while disabled, writes do not load it.
        apu.write(0x4015, 0x00);
        assert_eq!(apu.length_counters()[0], 0);
        apu.write(0x4003, 0x09);
        assert_eq!(apu.length_counters()[0], 0);
    }

    #[test]
    fn envelope_decays_once_per_quarter_frame() {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x08);
        apu.write(0x400C, 0x00); // no loop, envelope with period 0
        apu.write(0x400E, 0x00);
        apu.write(0x400F, 0xF8); // start the envelope
        let mut out = Vec::new();
        let mut volumes = Vec::new();
        for _ in 0..17 {
            // Run to just past each quarter frame of the 4-step sequence.
            let next = FOUR_STEP
                .iter()
                .chain(&[FOUR_STEP_LENGTH + FOUR_STEP[0]])
                .find(|&&at| at > apu.sequence_cycle)
                .copied()
                .unwrap();
            apu.run(next - apu.sequence_cycle, &mut out);
            volumes.push(apu.noise.envelope_volume());
        }
        // The first clock starts the envelope at 15, then it decays by one per clock to 0.
        let expected: Vec<u8> = (0..=15).rev().chain([0]).collect();
        assert_eq!(volumes, expected);
    }

    #[test]
    fn sweep_lowers_pulse_1_with_ones_complement() {
        let mut apu = pulse_apu(0x100);
        // Enabled, divider period 0, negate, shift 1.
        apu.write(0x4001, 0x89);
        let mut out = Vec::new();
        apu.run(FOUR_STEP[1] + 1, &mut out); // first half frame
        // Pulse 1: 0x100 - 0x80 - 1.
        assert_eq!(apu.pulses[0].period(), 0x7F);
        let mut apu2 = pulse_apu(0x100);
        apu2.write(0x4004, 0xBF);
        apu2.write(0x4006, 0x00);
        apu2.write(0x4007, 0x09);
        apu2.write(0x4005, 0x89);
        apu2.run(FOUR_STEP[1] + 1, &mut out);
        // Pulse 2: two's complement.
        assert_eq!(apu2.pulses[1].period(), 0x80);
    }

    #[test]
    fn sweep_target_above_7ff_mutes_even_when_disabled() {
        let mut apu = pulse_apu(0x500);
        apu.write(0x4001, 0x01); // disabled, shift 1: target $780, fine
        assert!(rises(&mut apu, 0, 20_000) > 0);
        apu.write(0x4002, 0x60);
        apu.write(0x4003, 0x0D); // period $560: target $810
        assert_eq!(rises(&mut apu, 0, 20_000), 0);
    }

    #[test]
    fn triangle_runs_while_both_counters_are_non_zero() {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x04);
        apu.write(0x4008, 0x04); // control clear, linear reload 4
        apu.write(0x400A, 0x40);
        apu.write(0x400B, 0x08); // reload flag set
        let mut out = Vec::new();
        // Before the first quarter frame the linear counter is 0: no stepping.
        assert_eq!(rises(&mut apu, 2, 7000), 0);
        // Loaded at the first quarter frame, then 4 more clocks to reach 0.
        apu.run(FOUR_STEP[0] - 7000 + 1, &mut out);
        assert!(rises(&mut apu, 2, 2000) > 0);
        apu.run(4 * FOUR_STEP_LENGTH, &mut out);
        assert_eq!(rises(&mut apu, 2, 20_000), 0);
        // One octave below a pulse with the same period: 32 steps per period of t + 1 cycles.
        apu.write(0x4008, 0xFF);
        apu.write(0x400B, 0x08);
        apu.run(FOUR_STEP_LENGTH, &mut out);
        let steps = 32 * (0x40 + 1);
        // 15 rising steps per period.
        let count = rises(&mut apu, 2, steps * 100);
        assert_eq!(count, 15 * 100);
    }

    #[test]
    fn noise_sequences_have_the_documented_lengths() {
        for (mode, period) in [(0u8, 32767u32), (0x80, 93)] {
            let mut noise = Noise::default();
            noise.write(2, mode);
            let start = noise.shift_register();
            let mut steps = 0;
            loop {
                noise.clock_shift_register();
                steps += 1;
                if noise.shift_register() == start {
                    break;
                }
            }
            assert_eq!(steps, period, "mode {mode:#x}");
        }
    }

    #[test]
    fn mixer_matches_the_formula() {
        assert_eq!(mix(0, 0, 0, 0, 0), 0.0);
        let full_pulses = mix(15, 15, 0, 0, 0);
        assert!((full_pulses - 0.2585).abs() < 0.001, "{full_pulses}");
        let tnd = mix(0, 0, 15, 15, 127);
        assert!((tnd - 0.7417).abs() < 0.001, "{tnd}");
        // A DMC level of 16 lowers the triangle's contribution.
        assert!(mix(0, 0, 15, 0, 16) - mix(0, 0, 0, 0, 16) < mix(0, 0, 15, 0, 0));
    }

    #[test]
    fn a_second_of_frames_gives_a_second_of_samples() {
        let mut apu = pulse_apu(0x100);
        let mut out = Vec::new();
        for _ in 0..601 {
            apu.run_frame(&[], &mut out);
        }
        let expected = 48_000.0 * 601.0 / FRAME_HZ;
        assert!((out.len() as f64 - expected).abs() <= 1.0, "{}", out.len());
        // The filters centre the output on 0 and it stays well within ±1.
        let tail = &out[out.len() - 4800..];
        let mean: f32 = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(mean.abs() < 0.01, "{mean}");
        assert!(out.iter().all(|s| s.abs() < 1.0));
        assert!(tail.iter().any(|s| s.abs() > 0.05));
    }

    #[test]
    fn timed_writes_apply_at_their_cycle() {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x01);
        let mut out = Vec::new();
        apu.run_frame(
            &[
                TimedWrite {
                    cycle: 20_000,
                    register: 0x4003,
                    value: 0x08,
                },
                TimedWrite {
                    cycle: 10,
                    register: 0x4000,
                    value: 0x9F,
                },
            ],
            &mut out,
        );
        // Loaded at cycle 20 000 with 254, after the half frame at 14 913, before 29 829.
        assert_eq!(apu.length_counters()[0], 254);
    }

    #[test]
    fn five_step_mode_clocks_at_once_and_has_two_half_frames_per_37282_cycles() {
        let mut apu = Apu::new(48_000);
        apu.write(0x4015, 0x01);
        apu.write(0x4000, 0x9F);
        apu.write(0x4003, 0x08); // 254
        apu.write(0x4017, 0x80);
        assert_eq!(apu.length_counters()[0], 253);
        let mut out = Vec::new();
        apu.run(FIVE_STEP_LENGTH * 2, &mut out);
        assert_eq!(apu.length_counters()[0], 249);
    }
}
