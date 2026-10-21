//! The tractor beam and towing (`docs/spec/player.md` 8, `docs/spec/economy.md` 2.2 and 2.6): an
//! item hangs on a rigid rope of length 32 from the pod and swings under gravity and thrust.

use super::tables::SIN;

/// The rope table (player.md 8.2): entries 0-31 are the quarter circle, 32-63 the catch radius.
pub const ROPE: [u8; 64] = [
    0x00, 0x02, 0x03, 0x05, 0x06, 0x08, 0x09, 0x0B, 0x0C, 0x0E, 0x0F, 0x10, 0x12, 0x13, 0x14, 0x15,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1B, 0x1C, 0x1D, 0x1E, 0x1E, 0x1F, 0x1F, 0x1F, 0x20, 0x20, 0x20,
    0x20, 0x20, 0x20, 0x20, 0x1F, 0x1F, 0x1F, 0x1F, 0x1E, 0x1E, 0x1E, 0x1D, 0x1D, 0x1C, 0x1B, 0x1B,
    0x1B, 0x1A, 0x19, 0x18, 0x17, 0x15, 0x15, 0x14, 0x13, 0x12, 0x10, 0x0E, 0x0C, 0x0B, 0x08, 0x04,
];

/// Load class by item kind 1-19 (player.md 8.1).
const MASS: [u8; 19] = [2, 0, 1, 1, 1, 2, 2, 0, 1, 0, 1, 1, 0, 0, 0, 0, 2, 0, 2];

/// W's whole unit in its 24-bit form ($6C.$6B.$6A) (player.md 8.3).
const W_ONE: u32 = 0x01_0000;

/// The picture of the beam in slot 2 for rope angle `angle` (0-127): the rope is a line, so the
/// frame repeats every half turn, in sectors of four angle units. Spec gap, measured in the
/// original over a long tow (the frame byte is read by the lift's sign, player.md 8.3 b).
pub fn beam_frame(angle: u8) -> u8 {
    const FRAMES: [u8; 16] = [
        0x38, 0x6F, 0x39, 0x70, 0x3A, 0x71, 0x3B, 0x72, 0x3C, 0x72, 0x3B, 0x71, 0x3A, 0x70, 0x39,
        0x6F,
    ];
    FRAMES[usize::from((angle >> 2) + 1) & 15]
}

/// The load class of an item kind (RAM $67).
pub fn mass_of(kind: u8) -> u8 {
    usize::from(kind)
        .checked_sub(1)
        .and_then(|i| MASS.get(i))
        .copied()
        .unwrap_or(0)
}

/// The offset of the item from the pod at rope angle `angle` (0-127, player.md 8.2).
pub fn rope_offset(angle: u8) -> (i16, i16) {
    let r = usize::from(angle & 31);
    let (a, b) = (i16::from(ROPE[r]), i16::from(ROPE[31 - r]));
    match (angle >> 5) & 3 {
        0 => (a, -b),
        1 => (b, a),
        2 => (-a, b),
        _ => (-b, -a),
    }
}

/// The rope angle for an item at (`dx`, `dy`) from the pod (player.md 8.1): `r` is the largest
/// index of the rope table's first 32 entries that does not exceed |dx|, and the signs of dx and
/// dy choose the quadrant. dy is used only for its sign.
pub fn angle_for(dx: i32, dy: i32) -> u8 {
    let v = dx.unsigned_abs().min(31) as u8;
    let r = (0..32u8)
        .rev()
        .find(|&r| ROPE[usize::from(r)] <= v)
        .unwrap_or(0);
    match (dx >= 0, dy >= 0) {
        (true, false) => r,
        (true, true) => 64 - r,
        (false, true) => 64 + r,
        (false, false) => 128 - r,
    }
}

/// The tractor beam's state: RAM $66-$6E.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Tow {
    /// The slot of the item held, RAM $66; 0 = none.
    pub slot: usize,
    /// The held item's load class, RAM $67.
    pub mass: u8,
    /// The rope angle $69.$68 in 1/128 turn with an 8-bit fraction.
    pub angle: u16,
    /// The angular speed $6C.$6B.$6A (24 bits, whole part in the top byte) and its sense $6D
    /// (true: the angle decreases).
    pub speed: u32,
    pub sense: bool,
    /// The crystal spin count $6E (player.md 2.6).
    pub spin: u8,
    /// The held item's taken-in flag (its work 6, RAM $0422 + slot), as the step starts.
    pub taken_in: bool,
    /// Bit 0 of the 64 bytes the lift's sign is read from, `$0355 + X` (see [`Tow::lift`]),
    /// as the object table stands at the start of the step.
    pub sign_bits: u64,
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
        (self.angle >> 8) as u8
    }

    /// The beam takes the item in `slot` of `kind` hanging at (`dx`, `dy`) from the pod.
    pub fn catch(&mut self, slot: usize, kind: u8, dx: i32, dy: i32) {
        self.slot = slot;
        self.mass = mass_of(kind);
        // 8.3 "What persists": the fraction $68, W's lowest byte $6A and the sense $6D keep what
        // the last tow left; only the whole angle and W's upper two bytes are set.
        self.angle = u16::from(angle_for(dx, dy)) << 8 | (self.angle & 0xFF);
        self.speed &= 0xFF;
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

    /// Adds `amount` (at $6B.$6A) to W in the direction of the rule's sign, or subtracts it,
    /// negating W and flipping the sense when it goes below 0 (player.md 8.3 a and d). The
    /// "cap" is the program's own: only a carry out of $6B makes W exactly 1.00 ($6A holding the sum); W
    /// above 1.00 grows on until then (a glitch candidate awaiting Alex's decision).
    fn push_speed(&mut self, amount: u32, add: bool) {
        if add {
            let low = (self.speed & 0xFFFF) + amount;
            self.speed = if low > 0xFFFF {
                W_ONE | (low & 0xFF)
            } else {
                (self.speed & 0xFF_0000) + low
            };
        } else if self.speed >= amount {
            self.speed -= amount;
        } else {
            self.speed = amount - self.speed;
            self.sense = !self.sense;
        }
    }

    /// The gravity's effect on the swing (player.md 8.3): `a` is |g| / 4, `up` whether the pull is
    /// upward (g < 0).
    pub fn pull(&mut self, a: u16, up: bool) {
        let angle = self.angle8();
        let amount = u32::from(SIN[usize::from((2 * angle) & 0x7F)]) * ((8 * u32::from(a)) & 0xFF);
        let add = !(up ^ (angle >= 64) ^ self.sense);
        self.push_speed(amount, add);
    }

    /// The lift on the pod's vertical speed (player.md 8.3 b): the amount q in 1/65536 px per
    /// step, and the index X (0-63) of the byte whose bit 0 decides add (clear) or subtract
    /// (set) in the original. The original reads `$0355 + X`: the flags Y byte of object slot X
    /// for X below 32, the frame byte of slot X - 32 above ([`Tow::sign_bits`]). The remake
    /// decides with [`Tow::lift_adds`].
    pub fn lift(&self, a: u16) -> (u32, usize) {
        let angle = self.angle8();
        let s = i32::from(SIN[0x7F - usize::from((2 * angle) & 0x7F)]);
        let amount = (s as u32 * u32::from(a)) >> (2 - self.mass.min(2));
        let x = ((s - i32::from(a)).unsigned_abs() >> 1) as usize & 63;
        (amount, x)
    }

    /// Whether the lift adds to the pod's vertical speed (`docs/deviations.md`, "Tow lift
    /// sign"). With `original` the program's rule: bit `x` of [`Tow::sign_bits`] clear, a byte
    /// left over from an earlier multiplication. Otherwise the rope's actual pull: the lift adds
    /// when the pod moves the way the rope pulls it (down when the load is below, 32 <= A < 96,
    /// as in 8.3 e) and subtracts when it moves against it.
    pub fn lift_adds(&self, x: usize, pod_down: bool, original: bool) -> bool {
        if original {
            return self.sign_bits >> x & 1 == 0;
        }
        (32..96).contains(&self.angle8()) == pod_down
    }

    /// The pull of the swing on the pod's own speeds (player.md 8.3 e), at the start of the
    /// step from A and W as the last step left them: (horizontal, vertical) in 1/65536 px per
    /// step, right and down positive. u = (W >> 13) AND 255; vertically u times `S(2 (A AND 31))`,
    /// horizontally u times `S(2 ((32 - A) AND 31))`, each shifted left by the load class m and
    /// kept to 16 bits. Down when 32 <= A < 96, left when A >= 64.
    pub fn pull_on_pod(&self) -> (i32, i32) {
        let angle = self.angle8();
        let u = (self.speed >> 13) & 0xFF;
        let m = u32::from(self.mass.min(2));
        let size = |index: u8| ((u * u32::from(SIN[usize::from(index) & 0x7F])) << m) & 0xFFFF;
        let vertical = size(2 * (angle & 31)) as i32;
        let horizontal = size(2 * (32u8.wrapping_sub(angle) & 31)) as i32;
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
    /// and whether each points left and down. Nothing uses the speeds while towing, but the
    /// direction bits are read by the lift's sign (8.3 b).
    pub fn item_motion(&self) -> ((u16, u16), (bool, bool)) {
        let angle = self.angle8();
        let u = u32::from((self.speed >> 11) as u8);
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

    /// The torque of a thrust step on W (player.md 8.3 d), run on every thrust step, towed or
    /// not. Towing, h = S(2 d) / 16 with d the angle between the rope and the nose; otherwise
    /// (nothing held, or the item being taken in) the scaling code that computes the index is
    /// skipped and the leftover `scratch` stands in for it: 20 + (pod y AND 15) after the move.
    pub fn torque(&mut self, facing: u8, scratch: u8) {
        let angle = self.angle8();
        let own = (4 * facing) & 63;
        let index = if self.active() {
            2 * (angle & 63).abs_diff(own)
        } else {
            scratch
        };
        let amount = u32::from(SIN[usize::from(index) & 0x7F] >> 4) << 8;
        let ahead = (angle.wrapping_sub(4u8.wrapping_mul(facing)) & 127) >= 64;
        self.push_speed(amount, ahead == self.sense);
    }

    /// The swing, each pod step after the thrust: the angle moves by the angular speed, which
    /// then loses 1/256 (player.md 8.2); skipped while the item is being taken in.
    pub fn swing(&mut self) {
        if self.taken_in {
            return;
        }
        let step = (self.speed >> 8) as u16;
        self.angle = if self.sense {
            self.angle.wrapping_sub(step)
        } else {
            self.angle.wrapping_add(step)
        } & 0x7FFF;
        // $6C.$6B loses 1 (both 0 if it would go below 0); $6A is not used.
        self.speed = (self.speed >> 8).saturating_sub(1) << 8 | (self.speed & 0xFF);
    }

    /// The crystal spin count (economy.md 2.6): the points to add, and whether the ninth swing
    /// completed (the crystal becomes blue, a life and a jetpod are earned).
    pub fn count_spin(&mut self) -> Option<SpinReward> {
        const POINTS: [u32; 8] = [25, 50, 100, 200, 400, 1000, 2000, 5000];
        let angle = self.angle8();
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

/// What a counted swing of a crystal gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SpinReward {
    Points(u32),
    /// The ninth swing: a life, a jetpod and a blue crystal.
    Bonus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rope_hangs_below_at_angle_64() {
        assert_eq!(rope_offset(0), (0, -32));
        assert_eq!(rope_offset(64), (0, 32));
        assert_eq!(rope_offset(32), (32, 0));
        assert_eq!(rope_offset(96), (-32, 0));
    }

    #[test]
    fn the_angle_follows_the_item_around_the_pod() {
        assert_eq!(angle_for(0, -20), 0);
        assert_eq!(angle_for(20, 0) >> 5, 1);
        assert_eq!(angle_for(-5, 20) >> 5, 2);
        assert_eq!(angle_for(-20, -3) >> 5, 3);
    }

    #[test]
    fn the_catch_angle_is_a_staircase_in_the_horizontal_offset() {
        // Demo 1, step 698 (player.md 8.1): dx = 12, dy = 28 gives 64 - 8.
        assert_eq!(angle_for(12, 28), 56);
        assert_eq!(angle_for(13, 28), 56);
        assert_eq!(angle_for(9, -28), angle_for(10, -28));
        // Directly below the pod: 64; the quadrants mirror each other.
        assert_eq!(angle_for(0, 20), 64);
        assert_eq!(angle_for(-12, 28), 72);
        assert_eq!(angle_for(-12, -28), 120);
    }

    #[test]
    fn gravity_swings_the_load_towards_the_hanging_position() {
        let mut tow = Tow {
            angle: 32 << 8,
            ..Tow::default()
        };
        tow.pull(2, false);
        assert!(tow.speed > 0 && !tow.sense, "a level load starts to fall");
        let mut tow = Tow {
            angle: 96 << 8,
            ..Tow::default()
        };
        tow.pull(2, false);
        assert!(tow.speed > 0 && tow.sense);
    }

    #[test]
    fn the_swing_loses_speed_and_wraps() {
        let mut tow = Tow {
            angle: 0x7F00,
            speed: 0x0001_0000,
            ..Tow::default()
        };
        tow.swing();
        assert_eq!(tow.angle, 0);
        assert_eq!(tow.speed, 0xFF00);
    }

    #[test]
    fn a_carry_out_of_the_low_bytes_sets_one_keeping_the_sum_in_the_lowest_byte() {
        let mut tow = Tow {
            speed: 0x00_F0_80,
            ..Tow::default()
        };
        tow.push_speed(0x1010, true);
        assert_eq!(tow.speed, 0x01_00_90);
        // Above 1.00 there is no cap until the next carry.
        tow.push_speed(0x0200, true);
        assert_eq!(tow.speed, 0x01_02_90);
    }

    #[test]
    fn a_speed_below_zero_is_negated_and_the_sense_flips() {
        let mut tow = Tow {
            speed: 0x00_04_00,
            ..Tow::default()
        };
        tow.push_speed(0x04C0, false);
        assert_eq!((tow.speed, tow.sense), (0xC0, true));
    }

    #[test]
    fn the_swing_keeps_the_lowest_byte() {
        let mut tow = Tow {
            speed: 0x00_00_C0,
            ..Tow::default()
        };
        tow.swing();
        assert_eq!(tow.speed, 0xC0);
    }

    #[test]
    fn a_catch_keeps_the_fraction_and_the_lowest_byte() {
        let mut tow = Tow {
            angle: 0x1253,
            speed: 0x12_34_D0,
            ..Tow::default()
        };
        tow.catch(13, 10, 20, 0);
        assert_eq!(tow.angle & 0xFF, 0x53);
        assert_eq!(tow.speed, 0xD0);
    }

    #[test]
    fn the_lift_reads_the_sign_from_a_byte_chosen_by_the_sine() {
        let tow = Tow {
            angle: 26 << 8,
            ..Tow::default()
        };
        // Class 0, a = 2: q = S(75) * 2 / 4 = 61 and X = (123 - 2) / 2 = 60.
        assert_eq!(tow.lift(2), (61, 60));
    }

    #[test]
    fn the_lift_takes_the_sign_of_the_ropes_pull() {
        let below = Tow {
            angle: 64 << 8,
            ..Tow::default()
        };
        let above = Tow::default();
        // A load below pulls down: a pod moving down gains, one moving up loses.
        assert!(below.lift_adds(5, true, false));
        assert!(!below.lift_adds(5, false, false));
        // A load above pulls up.
        assert!(above.lift_adds(5, false, false));
        assert!(!above.lift_adds(5, true, false));
        // The leftover byte plays no part in the remake's rule...
        let stale = Tow {
            sign_bits: u64::MAX,
            ..below
        };
        assert!(stale.lift_adds(5, true, false));
        // ...and is all the original's rule reads.
        assert!(!stale.lift_adds(5, true, true));
        assert!(below.lift_adds(5, false, true));
    }

    #[test]
    fn the_pull_on_the_pod_follows_the_swing() {
        // W = 2.00 (u = 16), the load below and to the right (A = 48): right and down.
        let tow = Tow {
            angle: 48 << 8,
            speed: 0x02_00_00,
            ..Tow::default()
        };
        let size = 16 * i32::from(SIN[32]);
        assert_eq!(tow.pull_on_pod(), (size, size));
        // Above and to the left (A = 112) with a heavy load (class 2): left and up, four times
        // as hard.
        let tow = Tow {
            angle: 112 << 8,
            speed: 0x02_00_00,
            mass: 2,
            ..Tow::default()
        };
        assert_eq!(tow.pull_on_pod(), (-4 * size, -4 * size));
        // At A = 0 both terms are S(0) = 0.
        let tow = Tow {
            speed: 0x02_00_00,
            ..Tow::default()
        };
        assert_eq!(tow.pull_on_pod(), (0, 0));
    }

    #[test]
    fn the_torque_uses_the_leftover_when_nothing_is_towed() {
        // Nothing held: h = S(20 + y AND 15) / 16 = S(24) / 16 = 4, added at $6B.
        let mut tow = Tow {
            angle: 64 << 8,
            ..Tow::default()
        };
        tow.torque(0, 24);
        assert_eq!(tow.speed, 0x04_00);
        // Taken in: the same.
        let mut tow = Tow {
            slot: 12,
            taken_in: true,
            angle: 64 << 8,
            ..Tow::default()
        };
        tow.torque(8, 24);
        assert_eq!(tow.speed, 0x04_00);
    }

    #[test]
    fn an_item_being_taken_in_is_not_let_go() {
        let mut tow = Tow {
            slot: 12,
            taken_in: true,
            ..Tow::default()
        };
        tow.drop_item(3);
        assert_eq!((tow.slot, tow.release), (12, None));
        tow.taken_in = false;
        tow.drop_item(3);
        assert_eq!(tow.slot, 0);
        assert!(tow.release.is_some());
    }

    #[test]
    fn the_beam_picture_repeats_every_half_turn() {
        assert_eq!(beam_frame(60), 0x38);
        assert_eq!(beam_frame(61), 0x38);
        assert_eq!(beam_frame(64), 0x6F);
        assert_eq!(beam_frame(28), 0x3C);
        assert_eq!(beam_frame(25), 0x72);
    }

    #[test]
    fn crystal_swings_count_between_the_two_windows() {
        let mut tow = Tow {
            angle: 66 << 8,
            ..Tow::default()
        };
        assert_eq!(tow.count_spin(), None);
        tow.angle = 115 << 8;
        assert_eq!(tow.count_spin(), Some(SpinReward::Points(25)));
        assert_eq!(tow.count_spin(), None, "it must re-arm");
    }
}
