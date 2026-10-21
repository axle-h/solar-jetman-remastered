//! Scaled rope model: all quantities per tick of fraction f (Q16, 65536 = one 1/30 s step).
//! Angle `a`: 7.24 fixed (0..2^31 = 128 units); W `w`: the original's 24-bit W << 8.
use solar_jetman::game::gameplay::tables::SIN;

pub const ONE_W: u32 = 1 << 24; // W = 1.00 in ext units

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rope {
    pub a: u32,
    pub w: u32,
    pub sense: bool,
    pub mass: u8,
    pub snap0: u32,
    pub snap1: u32,
    /// transient: Q16 fraction of the current step still to run after this piece
    pub rest: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    /// The original's carry rule (sawtooth between 1.00 and 2.00).
    Glitch,
    /// The original's carry rule, but the retained low byte is scaled by f (rate-free hover).
    GlitchScaled,
    /// GlitchScaled, and the snap lands where the original's step-end snap would be, 1.00 minus
    /// what the push would still add before the step ends.
    GlitchPend,
    GlitchPendHalf,
    /// The carry rule with the low byte dropped (W = 1.00 exactly).
    GlitchDrop,
    /// A true clamp at 1.00.
    Clamp,
    /// No cap at all.
    None,
}

impl Rope {
    pub fn from_real(angle: u16, speed: u32, sense: bool, mass: u8) -> Rope {
        Rope { a: u32::from(angle) << 16, w: speed << 8, sense, mass, snap0: 0, snap1: 0, rest: 0 }
    }
    pub fn angle8(&self) -> u8 {
        (self.a >> 24) as u8
    }
    pub fn angle16(&self) -> u16 {
        (self.a >> 16) as u16
    }
    pub fn speed24(&self) -> u32 {
        self.w >> 8
    }

    /// Adds or subtracts `amount` (W24 units, 16 bit) scaled by f.
    pub fn push(&mut self, amount: u32, add: bool, f: u32, cap: Cap) {
        let amt = ((u64::from(amount) << 8) * u64::from(f) >> 16) as u32; // ext units
        if add {
            match cap {
                Cap::Glitch => {
                    let low = u64::from(self.w & 0x00FF_FFFF) + u64::from(amt);
                    self.w = if low > 0x00FF_FFFF {
                        if self.w >> 24 == 0 { self.snap0 += 1 } else { self.snap1 += 1 }
                        ONE_W | (low as u32 & 0xFFFF)
                    } else {
                        (self.w & 0xFF00_0000) + low as u32
                    };
                }
                Cap::GlitchScaled | Cap::GlitchDrop | Cap::GlitchPend | Cap::GlitchPendHalf => {
                    let low = u64::from(self.w & 0x00FF_FFFF) + u64::from(amt);
                    self.w = if low > 0x00FF_FFFF {
                        if self.w >> 24 == 0 { self.snap0 += 1 } else { self.snap1 += 1 }
                        let keep = if cap == Cap::GlitchDrop { 0 } else { ((low & 0xFFFF) * u64::from(f)) >> 16 };
                        let comp = if cap == Cap::GlitchPendHalf { (((u64::from(amount) << 8) * u64::from(self.rest)) >> 17) as u32 } else if cap == Cap::GlitchPend { (((u64::from(amount) << 8) * u64::from(self.rest)) >> 16) as u32 } else { 0 };
                        (ONE_W | keep as u32).saturating_sub(comp)
                    } else {
                        (self.w & 0xFF00_0000) + low as u32
                    };
                }
                Cap::Clamp => {
                    self.w = (self.w + amt).min(ONE_W.max(self.w));
                }
                Cap::None => self.w += amt,
            }
        } else if self.w >= amt {
            self.w -= amt;
        } else {
            self.w = amt - self.w;
            self.sense = !self.sense;
        }
    }

    /// The angle the step's forces were sampled at: the angle at the start of the current step
    /// (`tau` = Q16 fraction of the step elapsed at the tick's start).
    pub fn hold_angle(&self, tau: u32) -> u8 {
        let back = u64::from(self.w >> 16) * u64::from(tau);
        let a = if self.sense { i64::from(self.a) + back as i64 } else { i64::from(self.a) - back as i64 };
        (a.rem_euclid(1 << 31) >> 24) as u8
    }

    pub fn pull(&mut self, a: u16, up: bool, f: u32, cap: Cap) {
        let angle = self.angle8();
        self.pull_at(angle, a, up, f, cap)
    }
    pub fn pull_at(&mut self, angle: u8, a: u16, up: bool, f: u32, cap: Cap) {
        let amount = u32::from(SIN[usize::from((2 * angle) & 0x7F)]) * ((8 * u32::from(a)) & 0xFF);
        let add = !(up ^ (angle >= 64) ^ self.sense);
        self.push(amount, add, f, cap);
    }

    pub fn torque(&mut self, facing: u8, active: bool, scratch: u8, f: u32, cap: Cap) {
        let angle = self.angle8();
        self.torque_at(angle, facing, active, scratch, f, cap)
    }
    pub fn torque_at(&mut self, angle: u8, facing: u8, active: bool, scratch: u8, f: u32, cap: Cap) {
        let own = (4 * facing) & 63;
        let index = if active { 2 * (angle & 63).abs_diff(own) } else { scratch };
        let amount = u32::from(SIN[usize::from(index) & 0x7F] >> 4) << 8;
        let ahead = (angle.wrapping_sub(4u8.wrapping_mul(facing)) & 127) >= 64;
        self.push(amount, ahead == self.sense, f, cap);
    }

    /// Signed W in 2^-8 angle units per step (hi bytes), sense applied.
    pub fn ws(&self) -> i64 {
        let hi = i64::from(self.w >> 16);
        if self.sense { -hi } else { hi }
    }

    /// Swing with integrator variants: `ws_old` is the signed W before this tick's forces.
    pub fn swing_v(&mut self, f: u32, ws_old: i64, trap: bool, damp_comp: bool) {
        let ws_new = self.ws();
        let f = i64::from(f);
        let mut d: i64 = if trap { (f * (ws_old + ws_new) + 65536 * (ws_new - ws_old)) / 2 } else { f * ws_new };
        if damp_comp && self.w >= 65536 {
            let c = f * (65536 - f) / 131072;
            d += if self.sense { -c } else { c };
        }
        self.a = (i64::from(self.a) + d).rem_euclid(1 << 31) as u32;
        if self.w >= 65536 {
            self.w -= f as u32;
        }
    }

    pub fn swing(&mut self, f: u32) {
        let step = u64::from(self.w >> 16) * u64::from(f); // 2^-24 units
        let step = step as u32;
        self.a = if self.sense { self.a.wrapping_sub(step) } else { self.a.wrapping_add(step) } & 0x7FFF_FFFF;
        if self.w >= 65536 {
            self.w -= f;
        }
    }

    /// Pull on the pod, (right, down) signed, units 2^-24 px/step (amount * f).
    pub fn pull_on_pod(&self, f: u32) -> (i64, i64) {
        let angle = self.angle8();
        let u = (self.w >> 21) & 0xFF;
        let m = u32::from(self.mass.min(2));
        let size = |index: u8| ((u * u32::from(SIN[usize::from(index) & 0x7F])) << m) & 0xFFFF;
        let vertical = i64::from(size(2 * (angle & 31)));
        let horizontal = i64::from(size(2 * (32u8.wrapping_sub(angle) & 31)));
        let sc = |x: i64| (x * i64::from(f)) >> 8;
        (
            sc(if angle >= 64 { -horizontal } else { horizontal }),
            sc(if (32..96).contains(&angle) { vertical } else { -vertical }),
        )
    }

    /// Lift amount (units 2^-24 px/step) and the real engine's index; sign by the remake's rule.
    pub fn lift(&self, a: u16, f: u32) -> i64 {
        let angle = self.angle8();
        let s = u32::from(SIN[0x7F - usize::from((2 * angle) & 0x7F)]);
        let amount = (s * u32::from(a)) >> (2 - self.mass.min(2));
        (i64::from(amount) * i64::from(f)) >> 8
    }

    pub fn thrust_scale(&self, facing: u8) -> u32 {
        0x38 + 32 * u32::from(2 - self.mass.min(2)) + u32::from((4 * facing) & 63)
    }
}
