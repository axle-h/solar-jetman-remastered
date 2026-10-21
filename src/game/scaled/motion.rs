//! Motion at any tick length (`docs/scaled.md`): accelerations scaled by the tick's
//! fraction f of a logic step, then a move that is exact at every 1/30 s whatever the split of
//! the step into ticks.
//!
//! Speeds are magnitudes with a direction, as in the Original, in Q8.24 px per step
//! ([`UNIT`] is the Original's 1/256 px per step). Positions are Q16.16 px. With f = [`ONE`]
//! every rule here is the Original's 8.8 rule.

/// One logic step as a tick fraction (Q16).
pub const ONE: u32 = 1 << 16;
/// The Original's speed unit, 1/256 px per step, in Q8.24.
pub const UNIT: u32 = 1 << 16;
/// 1.00 px per step in Q8.24.
pub const PX: u32 = 1 << 24;

/// One axis of a speed: a magnitude and whether it points the negative way (left or up), as
/// [`super::pod::Body`] keeps its two axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Axis {
    pub magnitude: u32,
    pub negative: bool,
}

impl Axis {
    pub fn signed(self) -> i64 {
        if self.negative {
            -i64::from(self.magnitude)
        } else {
            i64::from(self.magnitude)
        }
    }

    /// The Original's 8.8 speed.
    pub fn speed88(self) -> u16 {
        (self.magnitude >> 16).min(0xFFFF) as u16
    }

    /// Adds `amount` (Q8.24) the way `negative`: toward the motion it grows, against it it
    /// shrinks, and through 0 the leftover becomes a speed the other way.
    pub fn accelerate(&mut self, amount: u32, negative: bool) -> bool {
        if amount == 0 {
            return false;
        }
        if self.negative == negative {
            self.magnitude += amount;
            false
        } else if self.magnitude > amount {
            self.magnitude -= amount;
            false
        } else {
            self.magnitude = amount - self.magnitude;
            self.negative = negative;
            true
        }
    }

    /// Shrinks the magnitude by `amount` toward 0, not past it.
    pub fn brake(&mut self, amount: u32) {
        self.magnitude = self.magnitude.saturating_sub(amount);
    }

    /// Holds the magnitude at `cap`.
    pub fn cap(&mut self, cap: u32) -> bool {
        let over = self.magnitude > cap;
        self.magnitude = self.magnitude.min(cap);
        over
    }
}

/// An acceleration per step (Q8.24) over a tick of fraction `f`.
pub fn per_tick(amount: u32, f: u32) -> u32 {
    ((u64::from(amount) * u64::from(f)) >> 16) as u32
}

/// An acceleration in the Original's units per step (1/256 px per step per step) over a tick.
pub fn units(per_step: u16, f: u32) -> u32 {
    u32::from(per_step) * f
}

/// The move of one tick (`docs/scaled.md`): the displacement in Q16.16 px for a tick of
/// fraction `f` whose accelerations took the signed speed from `before` to `after` (Q8.24 px per
/// step). `f * after + (after - before) * (1 - f) / 2` sums over any split of a step to the
/// Original's move by the speed after the step's accelerations.
pub fn displacement(before: i64, after: i64, f: u32) -> i64 {
    let k = i64::from(ONE - f);
    let q24 = ((after * i64::from(f)) >> 16) + (((after - before) * k) >> 17);
    // Q.24 to Q.16, rounded to nearest.
    (q24 + 128) >> 8
}

/// A thrust on one axis (`docs/scaled.md`): `a` per step (the Original's units) and the
/// carry-in byte `carry` that feeds the sub-unit accumulator `acc` (Q.24, below one unit).
/// `cap` is the 3.00 cap, lowered on the vertical axis against gravity. The vertical axis keeps
/// the Original's reversal kick: one more full step of thrust when the speed against it is no
/// more than this tick's thrust. Returns whether the axis turned round.
pub fn thrust(
    axis: &mut Axis,
    acc: &mut u32,
    negative: bool,
    (a, carry): (u16, u8),
    f: u32,
    vertical: bool,
    cap: u32,
) -> bool {
    let du = units(a, f);
    let dacc = (u32::from(carry) * f) >> 8;
    if axis.negative == negative {
        axis.magnitude += du;
        *acc += dacc;
        if *acc >= UNIT {
            *acc -= UNIT;
            axis.magnitude += UNIT;
        }
        axis.cap(cap);
        return false;
    }
    // A borrow from the accumulator takes one unit off the speed.
    let mut borrow = 0;
    if *acc >= dacc {
        *acc -= dacc;
    } else {
        *acc = *acc + UNIT - dacc;
        borrow = 1;
    }
    let magnitude = i64::from(axis.magnitude) - borrow * i64::from(UNIT);
    let mut speed = i64::from(du) - magnitude;
    let kick = vertical && magnitude <= i64::from(du);
    if kick {
        *acc = u32::from(carry) << 8;
        speed += (i64::from(a) + borrow) * i64::from(UNIT);
    } else if !vertical && speed >= 0 {
        *acc = 0;
    }
    let turned = speed >= 0;
    if turned {
        axis.magnitude = speed as u32;
        axis.negative = negative;
    } else {
        axis.magnitude = (-speed) as u32;
    }
    if kick {
        axis.cap(3 * PX);
    }
    turned
}

/// Gravity of `a` units per step pulling the way `negative` (`docs/scaled.md`): toward
/// the motion it adds up to the 3.00 cap, against it it shrinks and reflects through 0.
pub fn gravity(axis: &mut Axis, a: u16, negative: bool, f: u32) {
    let amount = units(a, f);
    if axis.negative == negative {
        axis.magnitude += amount;
        // The Original's gravity holds a speed at 3.00 once its whole part reaches 3.
        if axis.magnitude >= 3 * PX {
            axis.magnitude = 3 * PX;
        }
    } else {
        axis.accelerate(amount, negative);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(speed88: u16, negative: bool) -> Axis {
        Axis {
            magnitude: u32::from(speed88) << 16,
            negative,
        }
    }

    #[test]
    fn a_whole_step_moves_by_the_speed_after_its_accelerations() {
        // f = 1: the Original's move, `x += v` after the accelerations.
        assert_eq!(displacement(0x80 << 16, 0x0140 << 16, ONE), 0x0140 << 8);
        // Any split of the step into ticks gives the same move under a constant acceleration.
        for parts in [2u32, 3, 4, 5, 7] {
            let mut speed = Axis::default();
            let mut moved = 0i64;
            let mut done = 0u32;
            for i in 0..parts {
                let f = (u64::from(ONE) * u64::from(i + 1) / u64::from(parts)) as u32 - done;
                done += f;
                let before = speed.signed();
                gravity(&mut speed, 16, false, f);
                moved += displacement(before, speed.signed(), f);
            }
            assert_eq!(speed.magnitude, 16 * UNIT);
            // Exact but for rounding to Q16 at each tick.
            assert!(
                (moved - (16 << 8)).abs() <= i64::from(parts),
                "{parts}: {moved}"
            );
        }
    }

    #[test]
    fn thrust_matches_the_original_step_at_f_one() {
        // Same way: grows, carries from the accumulator, holds at 3.00.
        let mut speed = axis(0x0120, false);
        let mut acc = 0xF0 << 8;
        thrust(&mut speed, &mut acc, false, (0x10, 0x20), ONE, true, 3 * PX);
        assert_eq!((speed.speed88(), acc >> 8), (0x0131, 0x10));
        // Against: shrinks, and through 0 the reversal kick adds one more step (with the
        // borrow), the accumulator restarts at the carry-in.
        let mut speed = axis(0x0008, true);
        let mut acc = 0x05 << 8;
        let turned = thrust(&mut speed, &mut acc, false, (0x10, 0x20), ONE, true, 3 * PX);
        assert!(turned);
        assert_eq!(
            (speed.speed88(), speed.negative, acc >> 8),
            (0x001A, false, 0x20)
        );
        // Horizontal: no kick, the accumulator clears on the turn.
        let mut speed = axis(0x0008, true);
        let mut acc = 0x05 << 8;
        thrust(&mut speed, &mut acc, false, (0x10, 0), ONE, false, 3 * PX);
        assert_eq!((speed.speed88(), speed.negative, acc), (0x0008, false, 0));
    }

    #[test]
    fn gravity_reflects_through_zero_and_caps() {
        let mut speed = axis(0x0004, true);
        gravity(&mut speed, 10, false, ONE);
        assert_eq!((speed.speed88(), speed.negative), (6, false));
        let mut speed = axis(0x02FA, false);
        gravity(&mut speed, 10, false, ONE);
        assert_eq!(speed.speed88(), 0x0300);
    }
}
