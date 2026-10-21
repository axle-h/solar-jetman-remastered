//! The tractor beam and towing (`docs/spec/player.md` 8, `docs/spec/economy.md` 2.2 and 2.6): an
//! item hangs on a rigid rope of length 32 from the pod and swings under gravity and thrust.

use super::tables::SIN;
pub use crate::game::gameplay::tow::{
    ROPE, SpinReward, angle_for, beam_frame, mass_of, rope_offset,
};

/// W of 1.00 in [`Tow::speed`]'s units.
const W_ONE: u32 = 1 << 24;

/// The moment within a logic step that a tick covers, which the rope's rules read
/// (`docs/scaled.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// The tick's length (Q16 of a step).
    pub f: u32,
    /// How far into its step the tick starts (Q16).
    pub tau: u32,
    /// What is left of the step after the tick (Q16), 0 when the tick ends on a step.
    pub rest: u32,
}

impl Moment {
    /// A whole step.
    pub const STEP: Moment = Moment {
        f: 1 << 16,
        tau: 0,
        rest: 0,
    };
}

/// The tractor beam's state: RAM $66-$6E, the rope's angle and angular speed with the Scaled
/// engine's extra fraction bits (`docs/scaled.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Tow {
    /// The slot of the item held, RAM $66; 0 = none.
    pub slot: usize,
    /// The held item's load class, RAM $67.
    pub mass: u8,
    /// The rope angle in 1/128 turn, 7.24 fixed point ($69.$68 is the top 16 bits).
    pub angle: u32,
    /// The angular speed W ($6C.$6B.$6A) with 8 more fraction bits: 1.00 is 1 << 24. The sense
    /// $6D: true, the angle decreases.
    pub speed: u32,
    pub sense: bool,
    /// The crystal spin count $6E (player.md 2.6).
    pub spin: u8,
    /// The held item's taken-in flag (its work 6, RAM $0422 + slot), as the step starts.
    pub taken_in: bool,
    /// An item let go this step, for the game to push toward the pod: its slot, the
    /// distance in pixels and the rope angle (player.md 8.1).
    pub release: Option<(usize, i16, u8)>,
}

impl Tow {
    pub fn holding(&self) -> bool {
        self.slot != 0
    }

    /// Whether the load acts on the pod: held and not being taken in by the mothership
    /// (player.md 8.3: then the pod behaves untowed).
    pub fn active(&self) -> bool {
        self.holding() && !self.taken_in
    }

    /// The rope angle's whole part, RAM $69.
    pub fn angle8(&self) -> u8 {
        (self.angle >> 24) as u8
    }

    /// The rope's bytes in the Original's form: the angle $69.$68 and W $6C.$6B.$6A.
    pub fn rope(&self) -> (u16, u32) {
        ((self.angle >> 16) as u16, self.speed >> 8)
    }

    /// Takes the rope's bytes in the Original's form.
    pub fn set_rope(&mut self, angle: u16, speed: u32) {
        self.angle = u32::from(angle & 0x7FFF) << 16;
        self.speed = speed << 8;
    }

    /// The beam takes the item in `slot` of `kind` hanging at (`dx`, `dy`) from the pod. The
    /// angle's fraction stays; W starts from 0 (the Original keeps its lowest byte, the
    /// leftover of the untowed torque, which Scaled does not have: `docs/scaled.md`).
    pub fn catch(&mut self, slot: usize, kind: u8, dx: i32, dy: i32) {
        self.slot = slot;
        self.mass = mass_of(kind);
        self.angle = u32::from(angle_for(dx, dy)) << 24 | (self.angle & 0xFF_FFFF);
        self.speed = 0;
        self.spin = 0;
    }

    /// Lets the item go; `push` is how far it is moved toward the pod's side on each axis. An
    /// item being taken in is never let go: it goes on being lowered (player.md 8.1).
    pub fn drop_item(&mut self, push: i16) {
        if self.holding() && !self.taken_in {
            self.release = Some((self.slot, push, self.angle8()));
            self.slot = 0;
        }
    }

    /// The angle as it was at the start of the step the tick is in: the rope's forces read the
    /// angle once per step in the Original (`docs/scaled.md`).
    pub fn held_angle(&self, moment: Moment) -> u8 {
        let back = i64::from(self.speed >> 16) * i64::from(moment.tau);
        let angle = if self.sense {
            i64::from(self.angle) + back
        } else {
            i64::from(self.angle) - back
        };
        (angle.rem_euclid(1 << 31) >> 24) as u8
    }

    /// Adds the per-step `amount` (W's $6B.$6A units) over the tick to W in the direction of
    /// the rule's sign, or subtracts it, negating W and flipping the sense when it goes below
    /// 0 (player.md 8.3 a and d). The cap is the program's own (kept, `docs/deviations.md`): a
    /// carry out of the low bytes makes W 1.00, keeping the low part scaled by the tick and less
    /// half of what the push would still add before the step ends (`docs/scaled.md`).
    fn push_speed(&mut self, amount: u32, add: bool, moment: Moment) {
        let whole = u64::from(amount) << 8;
        let part = ((whole * u64::from(moment.f)) >> 16) as u32;
        if add {
            let low = u64::from(self.speed & 0xFF_FFFF) + u64::from(part);
            self.speed = if low > 0xFF_FFFF {
                let keep = ((low & 0xFFFF) * u64::from(moment.f)) >> 16;
                let still = (whole * u64::from(moment.rest)) >> 17;
                (W_ONE | keep as u32).saturating_sub(still as u32)
            } else {
                (self.speed & 0xFF00_0000) + low as u32
            };
        } else if self.speed >= part {
            self.speed -= part;
        } else {
            self.speed = part - self.speed;
            self.sense = !self.sense;
        }
    }

    /// The gravity's effect on the swing (player.md 8.3), over a tick: `a` is |g| / 4, `up`
    /// whether the pull is upward (g < 0), `angle` the held angle.
    pub fn pull(&mut self, angle: u8, a: u16, up: bool, moment: Moment) {
        let amount = u32::from(SIN[usize::from((2 * angle) & 0x7F)]) * ((8 * u32::from(a)) & 0xFF);
        let add = !(up ^ (angle >= 64) ^ self.sense);
        self.push_speed(amount, add, moment);
    }

    /// The lift on the pod's vertical speed (player.md 8.3 b) over a tick of fraction `f`, in
    /// Q8.24 px per step. The sign comes from [`Tow::lift_adds`].
    pub fn lift(&self, angle: u8, a: u16, f: u32) -> u32 {
        let s = u32::from(SIN[0x7F - usize::from((2 * angle) & 0x7F)]);
        let amount = (s * u32::from(a)) >> (2 - self.mass.min(2));
        ((u64::from(amount) * u64::from(f)) >> 8) as u32
    }

    /// Whether the lift adds to the pod's vertical speed (`docs/deviations.md`, "Tow lift
    /// sign"): the rope's actual pull. The lift adds when the pod moves the way the rope pulls
    /// it (down when the load is below, 32 <= A < 96, as in 8.3 e) and subtracts when it moves
    /// against it.
    pub fn lift_adds(angle: u8, pod_down: bool) -> bool {
        (32..96).contains(&angle) == pod_down
    }

    /// The pull of the swing on the pod's own speeds (player.md 8.3 e) over a tick of fraction
    /// `f`, from the held angle and W: (horizontal, vertical) in Q8.24 px per step, right and
    /// down positive. u = (W >> 13) AND 255; vertically u times `S(2 (A AND 31))`, horizontally
    /// u times `S(2 ((32 - A) AND 31))`, each shifted left by the load class m and kept to 16
    /// bits. Down when 32 <= A < 96, left when A >= 64.
    pub fn pull_on_pod(&self, angle: u8, f: u32) -> (i64, i64) {
        let u = (self.speed >> 21) & 0xFF;
        let m = u32::from(self.mass.min(2));
        let size = |index: u8| ((u * u32::from(SIN[usize::from(index) & 0x7F])) << m) & 0xFFFF;
        let scale = |amount: u32| (i64::from(amount) * i64::from(f)) >> 8;
        let vertical = scale(size(2 * (angle & 31)));
        let horizontal = scale(size(2 * (32u8.wrapping_sub(angle) & 31)));
        (
            if angle >= 64 { -horizontal } else { horizontal },
            if (32..96).contains(&angle) {
                vertical
            } else {
                -vertical
            },
        )
    }

    /// The towed item's own speed and directions, set at the start of every step from the state
    /// the last one left (player.md 8.3, "The item's own speed"): (horizontal, vertical) in 8.8
    /// and whether each points left and down.
    pub fn item_motion(&self) -> ((u16, u16), (bool, bool)) {
        let angle = self.angle8();
        let u = u32::from((self.speed >> 19) as u8);
        let i = usize::from((2 * angle) & 63);
        let p1 = ((u * u32::from(SIN[i])) >> 4) as u16;
        let p2 = ((u * u32::from(SIN[64 - i])) >> 4) as u16;
        let speeds = if angle & 32 == 0 { (p2, p1) } else { (p1, p2) };
        let left = (32..96).contains(&angle) != self.sense;
        let down = (angle < 64) != self.sense;
        (speeds, (left, down))
    }

    /// The scale F applied to the thrust accelerations (player.md 8.3 c): the load class keeps
    /// F/256 of the thrust (class 0 47-70 %, class 1 34-58 %, class 2 22-45 %).
    pub fn thrust_scale(&self, facing: u8) -> u8 {
        (0x38u32 + 32 * u32::from(2 - self.mass.min(2)) + u32::from((4 * facing) & 63)) as u8
    }

    /// The torque of a tick of thrust on W (player.md 8.3 d), towing only: h = S(2 d) / 16 with
    /// d the angle between the rope and the nose. The Original's torque without a load (on a
    /// leftover byte) has no visible effect and is dropped (`docs/scaled.md`).
    pub fn torque(&mut self, angle: u8, facing: u8, moment: Moment) {
        if !self.active() {
            return;
        }
        let own = (4 * facing) & 63;
        let index = 2 * (angle & 63).abs_diff(own);
        let amount = u32::from(SIN[usize::from(index) & 0x7F] >> 4) << 8;
        let ahead = (angle.wrapping_sub(4u8.wrapping_mul(facing)) & 127) >= 64;
        self.push_speed(amount, ahead == self.sense, moment);
    }

    /// W signed by the sense, in the angle's 2^-24 units per step.
    pub fn signed_speed(&self) -> i64 {
        let whole = i64::from(self.speed >> 16);
        if self.sense { -whole } else { whole }
    }

    /// The swing over a tick, after the forces (player.md 8.2): the angle moves by the mean of
    /// the angular speed before the forces (`before`, [`Tow::signed_speed`]) and after them,
    /// and W loses 1/256 per step as a rate, stopping below that. Skipped while the item is
    /// being taken in.
    pub fn swing(&mut self, before: i64, f: u32) {
        if self.taken_in {
            return;
        }
        let after = self.signed_speed();
        let f = i64::from(f);
        let mut moved = (f * (before + after) + 65536 * (after - before)) / 2;
        if self.speed >= 1 << 16 {
            // The Original damps after the move: half a tick of it is owed.
            let owed = f * (65536 - f) / 131_072;
            moved += if self.sense { -owed } else { owed };
            self.speed -= f as u32;
        }
        self.angle = (i64::from(self.angle) + moved).rem_euclid(1 << 31) as u32;
    }

    /// The crystal spin count (economy.md 2.6) on the held `angle`: the points to add, and
    /// whether the ninth swing completed (the crystal becomes blue, a life and a jetpod are
    /// earned).
    pub fn count_spin(&mut self, angle: u8) -> Option<SpinReward> {
        const POINTS: [u32; 8] = [25, 50, 100, 200, 400, 1000, 2000, 5000];
        if angle >> 4 == 4 {
            self.spin |= 0x80;
        } else if angle >> 4 == 7 && self.spin & 0x80 != 0 {
            let count = (self.spin & 0x7F) + 1;
            self.spin = count;
            return Some(if count >= 9 {
                self.spin = 0;
                SpinReward::Bonus
            } else {
                SpinReward::Points(POINTS[usize::from(count) - 1])
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_swings_the_load_towards_the_hanging_position() {
        let mut tow = Tow {
            angle: 32 << 24,
            ..Tow::default()
        };
        tow.pull(32, 2, false, Moment::STEP);
        assert!(tow.speed > 0 && !tow.sense, "a level load starts to fall");
        let mut tow = Tow {
            angle: 96 << 24,
            ..Tow::default()
        };
        tow.pull(96, 2, false, Moment::STEP);
        assert!(tow.speed > 0 && tow.sense);
    }

    #[test]
    fn a_whole_step_swings_as_the_original_step() {
        // f = 1: the angle moves by W, then W loses 1/256; it wraps at 128.
        let mut tow = Tow::default();
        tow.set_rope(0x7F00, 0x0001_0000);
        let before = tow.signed_speed();
        tow.swing(before, 1 << 16);
        assert_eq!(tow.rope(), (0, 0xFF00));
    }

    #[test]
    fn ticks_of_a_step_swing_as_far_as_the_step() {
        for parts in [2u32, 3, 4, 5] {
            let mut whole = Tow::default();
            whole.set_rope(0x2000, 0x0000_8000);
            let mut split = whole;
            whole.swing(whole.signed_speed(), 1 << 16);
            let mut done = 0;
            for i in 0..parts {
                let end = (65536 * (i + 1)) / parts;
                let before = split.signed_speed();
                split.swing(before, end - done);
                done = end;
            }
            let (a, b) = (whole.rope().0, split.rope().0);
            assert!(a.abs_diff(b) <= 1, "{parts}: {a:#x} {b:#x}");
        }
    }

    #[test]
    fn a_carry_out_of_the_low_bytes_sets_one() {
        let mut tow = Tow::default();
        tow.set_rope(0, 0x00_F0_80);
        tow.push_speed(0x1010, true, Moment::STEP);
        assert_eq!(tow.rope().1, 0x01_00_90);
        // Above 1.00 there is no cap until the next carry.
        tow.push_speed(0x0200, true, Moment::STEP);
        assert_eq!(tow.rope().1, 0x01_02_90);
    }

    #[test]
    fn a_speed_below_zero_is_negated_and_the_sense_flips() {
        let mut tow = Tow::default();
        tow.set_rope(0, 0x00_04_00);
        tow.push_speed(0x04C0, false, Moment::STEP);
        assert_eq!((tow.rope().1, tow.sense), (0xC0, true));
    }

    #[test]
    fn a_catch_keeps_the_angles_fraction_and_starts_w_from_zero() {
        let mut tow = Tow::default();
        tow.set_rope(0x1253, 0x12_34_D0);
        tow.catch(13, 10, 20, 0);
        assert_eq!(tow.rope().0 & 0xFF, 0x53);
        assert_eq!(tow.speed, 0);
    }

    #[test]
    fn the_lift_follows_the_sine_and_the_load_class() {
        // Class 0, a = 2: q = S(75) * 2 / 4 = 61 (in 1/65536 px per step).
        let tow = Tow::default();
        assert_eq!(tow.lift(26, 2, 1 << 16), 61 << 8);
    }

    #[test]
    fn the_lift_takes_the_sign_of_the_ropes_pull() {
        // A load below pulls down: a pod moving down gains, one moving up loses.
        assert!(Tow::lift_adds(64, true));
        assert!(!Tow::lift_adds(64, false));
        // A load above pulls up.
        assert!(Tow::lift_adds(0, false));
        assert!(!Tow::lift_adds(0, true));
    }
}
