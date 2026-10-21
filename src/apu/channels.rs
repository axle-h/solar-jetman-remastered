//! The tone channels and their shared units: envelope, length counter and sweep. Timers count
//! CPU cycles; the caller clocks pulse timers only on every other CPU cycle (APU cycles).

/// Length counter load values by the 5-bit index in bits 3-7 of the fourth register.
const LENGTHS: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

/// Pulse duty sequences, read with a step index counting down from 0 (0, 7, 6, ...).
const DUTIES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0],
];

/// Noise timer periods in CPU cycles (NTSC).
const NOISE_PERIODS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// Volume envelope: a constant volume, or a decay level from 15 to 0 (looping or not).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
struct Envelope {
    start: bool,
    looping: bool,
    constant: bool,
    /// Constant volume, or the divider's period.
    parameter: u8,
    divider: u8,
    decay: u8,
}

impl Envelope {
    /// Bits 0-5 of the first register: loop (also length halt), constant, volume/period.
    fn write(&mut self, value: u8) {
        self.looping = value & 0x20 != 0;
        self.constant = value & 0x10 != 0;
        self.parameter = value & 0x0F;
    }

    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.parameter;
        } else if self.divider == 0 {
            self.divider = self.parameter;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looping {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    fn volume(&self) -> u8 {
        if self.constant {
            self.parameter
        } else {
            self.decay
        }
    }
}

/// Length counter with its channel's enable bit from $4015.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
struct Length {
    enabled: bool,
    halt: bool,
    count: u8,
}

impl Length {
    fn load(&mut self, value: u8) {
        if self.enabled {
            self.count = LENGTHS[usize::from(value >> 3)];
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.count = 0;
        }
    }

    fn clock(&mut self) {
        if !self.halt && self.count > 0 {
            self.count -= 1;
        }
    }
}

/// A pulse channel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Pulse {
    /// Pulse 1 negates the sweep change with ones' complement, pulse 2 with two's complement.
    ones_complement: bool,
    duty: u8,
    step: u8,
    period: u16,
    timer: u16,
    envelope: Envelope,
    length: Length,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_divider: u8,
    sweep_reload: bool,
}

impl Pulse {
    pub(crate) fn new(ones_complement: bool) -> Pulse {
        Pulse {
            ones_complement,
            duty: 0,
            step: 0,
            period: 0,
            timer: 0,
            envelope: Envelope::default(),
            length: Length::default(),
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_divider: 0,
            sweep_reload: false,
        }
    }

    /// Writes register 0-3 of the channel.
    pub(crate) fn write(&mut self, register: u16, value: u8) {
        match register {
            0 => {
                self.duty = value >> 6;
                self.envelope.write(value);
                self.length.halt = value & 0x20 != 0;
            }
            1 => {
                self.sweep_enabled = value & 0x80 != 0;
                self.sweep_period = (value >> 4) & 7;
                self.sweep_negate = value & 0x08 != 0;
                self.sweep_shift = value & 7;
                self.sweep_reload = true;
            }
            2 => self.period = (self.period & 0x700) | u16::from(value),
            _ => {
                self.period = (self.period & 0xFF) | (u16::from(value & 7) << 8);
                self.length.load(value);
                self.step = 0;
                self.envelope.start = true;
            }
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.length.set_enabled(enabled);
    }

    pub(crate) fn length(&self) -> u8 {
        self.length.count
    }

    #[cfg(test)]
    pub(crate) fn period(&self) -> u16 {
        self.period
    }

    /// One APU cycle.
    pub(crate) fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.step = (self.step + 7) & 7;
        } else {
            self.timer -= 1;
        }
    }

    pub(crate) fn quarter_frame(&mut self) {
        self.envelope.clock();
    }

    pub(crate) fn half_frame(&mut self) {
        self.length.clock();
        let target = self.sweep_target();
        if self.sweep_divider == 0
            && self.sweep_enabled
            && self.sweep_shift != 0
            && !self.muted()
            && (0..=0x7FF).contains(&target)
        {
            self.period = target as u16;
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    fn sweep_target(&self) -> i32 {
        let period = i32::from(self.period);
        let change = period >> self.sweep_shift;
        if !self.sweep_negate {
            period + change
        } else if self.ones_complement {
            period - change - 1
        } else {
            period - change
        }
    }

    /// The sweep unit mutes the channel when the period is below 8 or the target above $7FF,
    /// whether the sweep is enabled or not.
    fn muted(&self) -> bool {
        self.period < 8 || self.sweep_target() > 0x7FF
    }

    pub(crate) fn output(&self) -> u8 {
        if self.length.count == 0
            || self.muted()
            || DUTIES[usize::from(self.duty)][usize::from(self.step)] == 0
        {
            0
        } else {
            self.envelope.volume()
        }
    }
}

/// The triangle channel.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Triangle {
    /// Control flag: halts the length counter and keeps the linear counter reloading.
    control: bool,
    linear_reload_value: u8,
    linear: u8,
    linear_reload: bool,
    length: Length,
    period: u16,
    timer: u16,
    step: u8,
}

impl Triangle {
    pub(crate) fn write(&mut self, register: u16, value: u8) {
        match register {
            0 => {
                self.control = value & 0x80 != 0;
                self.length.halt = self.control;
                self.linear_reload_value = value & 0x7F;
            }
            1 => {}
            2 => self.period = (self.period & 0x700) | u16::from(value),
            _ => {
                self.period = (self.period & 0xFF) | (u16::from(value & 7) << 8);
                self.length.load(value);
                self.linear_reload = true;
            }
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.length.set_enabled(enabled);
    }

    pub(crate) fn length(&self) -> u8 {
        self.length.count
    }

    /// One CPU cycle. Periods 0 and 1 would run the waveform at an ultrasonic rate; the step is
    /// held instead (allowed by `docs/spec/sound.md` 4.1, "Rests").
    pub(crate) fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            if self.linear > 0 && self.length.count > 0 && self.period >= 2 {
                self.step = (self.step + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }

    pub(crate) fn quarter_frame(&mut self) {
        if self.linear_reload {
            self.linear = self.linear_reload_value;
        } else if self.linear > 0 {
            self.linear -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    pub(crate) fn half_frame(&mut self) {
        self.length.clock();
    }

    /// 15 down to 0, then 0 up to 15.
    pub(crate) fn output(&self) -> u8 {
        if self.step < 16 {
            15 - self.step
        } else {
            self.step - 16
        }
    }
}

/// The noise channel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Noise {
    envelope: Envelope,
    length: Length,
    /// Feedback from bit 6 instead of bit 1 (the short sequence).
    short: bool,
    period: u16,
    timer: u16,
    shift: u16,
}

impl Default for Noise {
    fn default() -> Noise {
        Noise {
            envelope: Envelope::default(),
            length: Length::default(),
            short: false,
            period: NOISE_PERIODS[0],
            timer: 0,
            shift: 1,
        }
    }
}

impl Noise {
    pub(crate) fn write(&mut self, register: u16, value: u8) {
        match register {
            0 => {
                self.envelope.write(value);
                self.length.halt = value & 0x20 != 0;
            }
            1 => {}
            2 => {
                self.short = value & 0x80 != 0;
                self.period = NOISE_PERIODS[usize::from(value & 0x0F)];
            }
            _ => {
                self.length.load(value);
                self.envelope.start = true;
            }
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.length.set_enabled(enabled);
    }

    pub(crate) fn length(&self) -> u8 {
        self.length.count
    }

    /// One CPU cycle.
    pub(crate) fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period - 1;
            self.clock_shift_register();
        } else {
            self.timer -= 1;
        }
    }

    pub(crate) fn clock_shift_register(&mut self) {
        let tap = if self.short { 6 } else { 1 };
        let feedback = (self.shift ^ (self.shift >> tap)) & 1;
        self.shift = (self.shift >> 1) | (feedback << 14);
    }

    #[cfg(test)]
    pub(crate) fn shift_register(&self) -> u16 {
        self.shift
    }

    #[cfg(test)]
    pub(crate) fn envelope_volume(&self) -> u8 {
        self.envelope.volume()
    }

    pub(crate) fn quarter_frame(&mut self) {
        self.envelope.clock();
    }

    pub(crate) fn half_frame(&mut self) {
        self.length.clock();
    }

    pub(crate) fn output(&self) -> u8 {
        if self.length.count == 0 || self.shift & 1 != 0 {
            0
        } else {
            self.envelope.volume()
        }
    }
}
