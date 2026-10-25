//! What each object does on its logic step (`docs/spec/behaviour.md`): the sweep, the helpers
//! the behaviours share, the pod-side objects, the turrets, the generators, mines and the green
//! orb. The placed objects of levels 1-7 are in [`super::enemies`], their children in
//! [`super::minions`].

use super::objects::{CHILD_SLOTS, Env, Object, ObjectTable, SHOT_SLOTS, SLOTS, state};
use super::pod::Pod;
use crate::game::sound::{Channel, Request};
use std::cell::Cell;
use std::ops::RangeInclusive;

/// The frames of a spark by its life counter (behaviour.md 6.4).
pub(crate) const SPARK_FRAMES: [u8; 4] = [0x37, 0x62, 0x63, 0x62];

/// Turret shots by type - 1 (behaviour.md 4.1): offset, speeds in 1/256 px and the vertical
/// direction (true = down).
pub(crate) const TURRET_SHOTS: [(i16, i16, u16, u16, bool); 16] = [
    (0, -6, 0x0000, 0x0200, false),
    (0, 7, 0x0000, 0x0200, true),
    (5, -8, 0x0080, 0x0180, false),
    (-5, -8, 0x0080, 0x0180, false),
    (5, 8, 0x0080, 0x0180, true),
    (-5, 8, 0x0080, 0x0180, true),
    (8, -6, 0x0140, 0x0140, false),
    (-8, -6, 0x0140, 0x0140, false),
    (8, 6, 0x0140, 0x0140, true),
    (-8, 6, 0x0140, 0x0140, true),
    (10, -4, 0x0180, 0x0080, false),
    (-10, -4, 0x0180, 0x0080, false),
    (10, 4, 0x0180, 0x0080, true),
    (-10, 4, 0x0180, 0x0080, true),
    (6, 1, 0x0200, 0x0000, false),
    (-6, 1, 0x0200, 0x0000, false),
];

/// Homing of an object on a target on both axes (behaviour.md 2.3): x first, moving x, then y.
pub(super) fn home_on(o: &mut Object, target: (u16, u16), accel: u16, max: u16) {
    o.home_x(target.0, accel, max);
    o.home_y(target.1, accel, max);
}

/// The speed limit a homing table writes as one byte M (behaviour.md 2.3): whole part M mod 8,
/// fraction M with the low three bits cleared.
pub(crate) fn max_speed(m: u8) -> u16 {
    u16::from(m & 7) << 8 | u16::from(m & 0xF8)
}

/// Slows an axis down by `amount`, stopping at 0 (behaviour.md 2.2).
pub(crate) fn slow_down(speed: &mut u16, amount: u16) {
    *speed = speed.saturating_sub(amount);
}

/// Slows an axis down and turns it at 0 (behaviour.md 2.2, the plain version).
pub(crate) fn slow_and_turn(speed: &mut u16, flip: &mut bool, amount: u16) {
    *speed = speed.saturating_sub(amount);
    if *speed == 0 {
        *flip = !*flip;
    }
}

/// The magnets' variant (behaviour.md 2.2): past 0 the overshoot becomes the speed and the
/// direction flips; reaching exactly 0 flips too.
pub(crate) fn slow_and_turn_overshoot(speed: &mut u16, flip: &mut bool, amount: u16) {
    if *speed >= amount {
        *speed -= amount;
        if *speed == 0 {
            *flip = !*flip;
        }
    } else {
        *speed = amount - *speed;
        *flip = !*flip;
    }
}

/// The distance to the pod (behaviour.md 2.4).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct Near {
    pub dx: u32,
    pub dy: u32,
    /// The uncapped |dy|, for the rules that use full distances.
    pub dy_full: u32,
    /// The |dy| a caller that reads the routine's scratch byte sees (quirk Q24).
    pub dy_read: u32,
    /// |dx| + |dy|, 255 when either is 256 or more.
    pub d: u32,
    /// The object's x is below the pod's.
    pub pod_right: bool,
    /// The object's y is below the pod's.
    pub pod_below: bool,
}

/// With `original_rules` the pod-below flag stays 0 when |dx| is 256 or more (quirk Q19, which
/// the remake corrects: the flag is computed at any distance). `scratch` is RAM $05, where the
/// routine leaves |dy| for its callers (quirk Q24).
pub(super) fn near(o: &Object, pod: &Pod, original_rules: bool, scratch: &Cell<u8>) -> Near {
    // Positions are signed: an object above the level's top edge has y just below 65536.
    let signed = |v: u16| i32::from(v as i16);
    // A difference of exactly -256 (the object 256 px before the pod) counts as 255
    // (behaviour.md 2.4).
    let distance = |difference: i32| {
        if difference == -256 {
            255
        } else {
            difference.unsigned_abs()
        }
    };
    let dx = distance(signed(o.x) - signed(pod.body.x));
    let diff_y = signed(o.y) - signed(pod.body.y);
    let dy = distance(diff_y);
    let d = if dx >= 256 || dy >= 256 {
        255
    } else {
        (dx + dy).min(255)
    };
    // What a caller reads from $05 (quirk Q24): with |dx| below 256 the routine stores |dy|
    // (its low byte, taken from the raw difference when the object is above the pod and |dy| is
    // 256 or more); otherwise it stores nothing and the caller reads what was there, which is
    // not modelled: `scratch` holds the original's value in validation and 255 (a drone then
    // climbs, as seen on level 4) otherwise.
    let read = if dx < 256 {
        u32::from(if dy < 256 || diff_y > 0 {
            dy as u8
        } else {
            diff_y as u8
        })
    } else {
        u32::from(scratch.get())
    };
    Near {
        dx,
        // Quirk Q24: the remake always measures |dy|, capped at 255.
        dy: if original_rules { dy } else { dy.min(255) },
        dy_full: dy,
        dy_read: if original_rules { read } else { dy.min(255) },
        d,
        pod_right: signed(o.x) < signed(pod.body.x),
        pod_below: if original_rules && dx < 256 && dy >= 256 {
            // Spec gap, fitted to five drone re-plans in the demos: with |dy| of 256 or more the
            // flag is an unsigned comparison of the two y positions (an object above the level's
            // top edge, y just below 65536, then counts as below the pod).
            o.y < pod.body.y
        } else {
            signed(o.y) < signed(pod.body.y) && (dx < 256 || !original_rules)
        },
    }
}

/// How a shot or child is to be made (behaviour.md 2.6).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct Shot {
    pub state: u8,
    pub frame: u8,
    pub x: u16,
    pub y: u16,
    pub speed: (u16, u16),
    pub flags: (u8, u8),
    pub life: u8,
}

impl ObjectTable {
    /// The A pass's sweep: every slot 1-30 with a state runs its behaviour in ascending order
    /// (behaviour.md 1). An object made in a higher slot runs in the same sweep.
    pub fn sweep(&mut self, env: &mut Env) {
        for slot in 1..SLOTS {
            if self.slots[slot].state != 0 {
                self.scratch_dy.set(self.scratch_feed[slot].unwrap_or(0xFF));
                self.step_slot(slot, env);
            }
        }
    }

    fn step_slot(&mut self, slot: usize, env: &mut Env) {
        let state = self.slots[slot].state;
        match state {
            state::SPARK | state::SPARK_RISING => {
                let o = &mut self.slots[slot];
                o.advance();
                if o.state == state::SPARK_RISING {
                    // Economy.md 5.4: the spark moves first; then the vertical speed
                    // changes by $40 towards up, turning at 0, to at most 3.00.
                    if o.down() {
                        o.speed_y = o.speed_y.saturating_sub(0x40);
                        if o.speed_y == 0 {
                            o.flags_y &= !1;
                        }
                    } else {
                        o.speed_y = (o.speed_y + 0x40).min(0x0300);
                    }
                }
                o.index = o.index.wrapping_add(1);
                if o.index == 144 {
                    o.state = 0;
                } else {
                    o.frame = SPARK_FRAMES[usize::from(o.index & 3)];
                }
            }
            state::EXHAUST => self.step_exhaust(slot, env),
            state::SPARKLE => {
                // The pick-up sparkle (behaviour.md 6.4): its life runs down one a step and
                // the picture follows it; the object goes at 0.
                let o = &mut self.slots[slot];
                o.index = o.index.wrapping_sub(1);
                if o.index == 0 {
                    o.state = 0;
                } else {
                    o.frame = 0x69 + ((o.index / 2) & 3);
                }
            }
            state::COUNTDOWN => self.step_countdown(slot, env),
            state::BULLET => self.step_bullet(slot, env),
            state::EXPLOSION_SMALL | state::EXPLOSION_LARGE => self.step_explosion(slot),
            state::ITEM => self.step_item(slot, env),
            state::RING => {
                // Behaviour.md 6.1 and objects.md 7 disagree on the speed; Mesen (level 7):
                // bit 3 of the frame counter, a change every 4 steps, as behaviour.md says.
                self.slots[slot].frame = 0x67 + ((env.f() >> 3) & 1) as u8;
            }
            state::MISSILE if super::weapons::is_shot_slot(slot) => {
                self.step_pod_missile(slot, env)
            }
            state::MISSILE => self.step_missile(slot, env),
            state::MULTI_WARHEAD => self.step_pod_missile(slot, env),
            state::STAR_PARENT => self.step_star_parent(slot, env),
            state::STAR_CHILD => self.step_star_child(slot, env),
            state::TIME_BOMB => self.step_time_bomb(slot, env),
            state::ENEMY_SHOT => {
                let o = &mut self.slots[slot];
                o.advance();
                o.index = o.index.wrapping_sub(1);
                if o.index == 0 {
                    o.state = 0;
                }
            }
            state::FLAME | state::FLAME_LOW => self.step_flame(slot),
            state::DRONE => self.step_drone(slot, env),
            state::GUN_SHOT => {
                let o = &mut self.slots[slot];
                o.advance_y();
                if o.frame < 0x54 {
                    o.frame += 1;
                }
                o.index = o.index.wrapping_sub(1);
                if o.index == 0 {
                    o.state = 0;
                }
            }
            state::HEAD_SHOT => {
                let o = &mut self.slots[slot];
                o.advance_x();
                o.index = o.index.wrapping_sub(1);
                if o.index == 0 {
                    o.state = 0;
                }
            }
            state::HEAD_A | state::HEAD_B => self.step_head(slot, env),
            state::MINE => self.step_mine(slot, env),
            state::HOPPER => self.step_hopper(slot, env),
            state::THRUSTER => {
                let owner = usize::from(self.slots[slot].kind);
                if self.slots[owner].state < 6 {
                    self.slots[slot].state = 0;
                }
            }
            state::GREEN_ORB => self.step_orb(slot, env),
            state::BONUS_STAR | state::BONUS_BOX => self.step_bonus(slot),
            // In the shot slots, state 7 is a particle of the new pod materialising.
            state::MOTHERSHIP if SHOT_SLOTS.contains(&slot) => self.step_particle(slot, env),
            state::MARKER => {
                // The invisible bonus sign (behaviour.md 6.2): rises 1 px a step for 256 steps.
                let o = &mut self.slots[slot];
                o.advance();
                o.kind = o.kind.wrapping_add(1);
                if o.kind == 0 {
                    o.state = 0;
                }
                // behaviour.md 6.2: it goes when the pickup tag's low nibble is not its number,
                // shows once the tag has no bits 5 and 6, and goes if bit 6 comes back.
                let tag = env.bonus_tag;
                if tag & 15 != o.index {
                    o.state = 0;
                } else if o.frame == super::objects::INVISIBLE {
                    if tag & 0x60 == 0 {
                        o.frame = 0xA0;
                    }
                } else if tag & 0x40 != 0 {
                    o.state = 0;
                }
            }
            state::TURRET_FIRST..=state::TURRET_LAST => self.step_turret(slot, env),
            state::DOME | state::DOME_LIFT => self.step_dome(slot, env),
            state::GUN_12 | state::GUN_17 | state::GUN_18 | state::GUN_47 => {
                self.step_gun(slot, env)
            }
            state::WALKER_A | state::WALKER_B => self.step_walker(slot, env),
            state::PILLAR_A | state::PILLAR_B => self.step_pillar(slot, env),
            state::HOURGLASS | state::HOURGLASS_UP | state::BLOCK => self.step_hourglass(slot, env),
            state::MACHINE_A | state::MACHINE_B => self.step_machine(slot, env),
            state::MAGNET_PULL | state::MAGNET_PUSH => self.step_magnet(slot, env),
            state::CREATURE => self.step_creature(slot, env),
            state::BIG_MACHINE => self.step_big_machine(slot, env),
            state::FORMATION_A | state::FORMATION_B => self.step_formation(slot, env),
            state::GREEN => self.step_green(slot, env),
            state::SPHERE => self.step_sphere(slot, env),
            state::GENERATOR_FIRST..=state::GENERATOR_LAST => self.step_generator(slot, env),
            state::FINAL_FIRST..=state::FINAL_LAST => self.step_final_enemy(slot, env),
            state::CORE => self.step_core(slot, env),
            state::PIECE => self.step_piece(slot, env),
            state::EGG => self.step_egg(slot),
            state::SATELLITE => self.step_satellite(slot, env),
            state::COMPANION => self.step_companion(slot, env),
            // Not implemented: the blue sphere (level 8 on), the level 26 objects and the
            // pod-side companion (spec gaps in behaviour.md 4.15, 4.16, 4.18).
            _ => {}
        }
    }

    /// The warp level's countdown digit (behaviour.md 6.3): 56 px below the pod, its speed
    /// bytes a 16-bit step counter, the picture $73 + counter / 64; past the last digit the
    /// level ends.
    fn step_countdown(&mut self, slot: usize, env: &Env) {
        let o = &mut self.slots[slot];
        o.x = env.pod.body.x;
        o.y = env.pod.body.y.wrapping_add(56);
        o.flags_x = 0;
        o.speed_x = o.speed_x.wrapping_add(1);
        let digit = 0x73 + u32::from(o.speed_x / 64);
        if digit > 0x7D {
            self.countdown_done = true;
        } else {
            o.frame = digit as u8;
        }
    }

    // ---- Pod-side objects --------------------------------------------------------------------

    /// A pod bullet (player.md 10): straight flight; every 4th step by slot and step count its
    /// life drops, and at 0 it is removed.
    fn step_bullet(&mut self, slot: usize, env: &Env) {
        let o = &mut self.slots[slot];
        o.advance();
        // Spec gap: player.md 10 says (slot + $5D / 2) AND 3 = 0; measured, it is 3.
        if (slot as u32 + env.f() / 2) & 3 == 3 {
            o.index = o.index.wrapping_sub(1);
            if o.index == 0 {
                o.state = 0;
            }
        }
        // Black-box fit (bullets-titanium and bullets-military replays): a shot of the upgraded
        // gun changes its picture as its life runs down: $62 once 4 ticks are gone, and with
        // 3 hit points $63 after 8.
        if o.hit_points > 1 && o.state == state::BULLET {
            o.frame = match o.index {
                0x1D.. => 0x37,
                0x19..=0x1C => 0x62,
                _ if o.hit_points == 2 => 0x62,
                _ => 0x63,
            };
        }
    }

    /// A particle of the new pod materialising (player.md 7.3): its angular speed grows by
    /// $2E/256 a step, the angle advances by it, and it is placed on an ellipse round the pod
    /// that shrinks by 3 a step. Spec gap: the placement, the start angle and the order of the
    /// steps were measured (the start angle is 8 per slot from slot 3; the polar placement of
    /// bank 7 is not specified).
    fn step_particle(&mut self, slot: usize, env: &Env) {
        use super::tables::SIN;
        let o = &mut self.slots[slot];
        o.speed_x = o.speed_x.wrapping_add(0x2E);
        let angle = (u16::from(o.index) << 8 | u16::from(o.work5)).wrapping_add(o.speed_x);
        o.index = (angle >> 8) as u8;
        o.work5 = angle as u8;
        let radius = i32::from(o.speed_y);
        let polar = |a: u8| {
            let magnitude = (i32::from(SIN[usize::from(a & 0x7F)]) * radius) >> 8;
            if a & 0x80 != 0 { -magnitude } else { magnitude }
        };
        // Measured: in the quarter turn after the horizontal offset turns negative, the vertical
        // offset is one pixel less negative than the mirror image of the positive half, unless
        // the horizontal offset is 0 (probably a carry left by the sign change).
        let dx = polar(o.index);
        let upper_quarter = (0x80..0xC0).contains(&o.index);
        let dy = polar(o.index.wrapping_add(0x40)) + i32::from(upper_quarter && dx != 0);
        // Every 32 frames the picture moves on along the spark dots (measured; player.md 7.3
        // gives picture $37 only).
        if env.f() & 0x1F == 0 {
            o.frame = match o.frame {
                0x37 => 0x62,
                0x62 => 0x63,
                f => f,
            };
        }

        o.x = env.pod.body.x.wrapping_add_signed(dx as i16);
        o.y = env.pod.body.y.wrapping_add_signed(dy as i16);
        if radius < 3 {
            o.state = 0;
            self.arrivals += 1;
        } else {
            o.speed_y -= 3;
        }
    }

    // ---- Items (behaviour.md 6.1) ------------------------------------------------------------

    /// A still item. Its position is written back to the list every step so a dropped item
    /// is found again where it was left; the star marker (kind 27) rises and goes.
    fn step_item(&mut self, slot: usize, env: &Env) {
        let o = &mut self.slots[slot];
        o.flags_x = 0;
        if o.kind == 27 {
            // objects.md 10.2: the speed's fraction grows by 4/256 a step; at $E0 the marker
            // vanishes (56 steps), else it rises by the speed.
            o.flags_y = 0;
            o.speed_y = o.speed_y.wrapping_add(4);
            o.frame = 0x7E + ((env.f() >> 2) & 1) as u8;
            if o.speed_y & 0xFF < 0xE0 {
                o.advance_y();
            } else {
                o.state = 0;
                let index = usize::from(o.index);
                if let Some(entry) = self.items.get_mut(index) {
                    entry.kind = 0;
                }
                return;
            }
        }
        if o.work6 != 0 {
            // Pulled into the bay: lowered at 1.00 px a step (economy.md 2.2); the speed is set
            // here, in the step it moves, not in the step the delivery snaps it to the column.
            o.speed_x = 0;
            o.speed_y = 0x0100;
            o.flags_y = 1;
            o.advance_y();
        }
        let (index, x, y) = (usize::from(o.index), o.x, o.y);
        if let Some(entry) = self.items.get_mut(index) {
            entry.x = x;
            entry.y = y;
        }
        // The fourth-slot pull-back (behaviour.md 6.1, quirk Q23): with slots 11 and 12 both
        // taken, the item in slot 14 (or 13 while the beam holds 14) creeps back to its place in
        // the level's list and the list entry is reset to that place. The remake leaves items
        // where they are.
        let pulled = slot == 14 || (slot == 13 && env.beam_slot == 14);
        if env.original_rules
            && pulled
            && env.beam_slot != slot
            && self.slots[slot].kind != 27
            && self.slots[11].state != 0
            && self.slots[12].state != 0
            && let Some(home) = env.level.objects.get(index)
        {
            let o = &mut self.slots[slot];
            o.x = o.x.wrapping_add_signed(match o.x.cmp(&home.x) {
                std::cmp::Ordering::Less => 1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => -1,
            });
            o.y = o.y.wrapping_add_signed(match o.y.cmp(&home.y) {
                std::cmp::Ordering::Less => 1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => -1,
            });
            if let Some(entry) = self.items.get_mut(index) {
                entry.x = home.x;
                entry.y = home.y;
            }
        }
    }

    // ---- Shots made by objects (behaviour.md 2.5, 2.6) -----------------------------------------

    /// A standard enemy shot in the first free slot of `range` (the range's last slot itself
    /// when `overwrite` and none is free). Returns the slot used.
    pub(super) fn shoot(
        &mut self,
        env: &Env,
        range: RangeInclusive<usize>,
        overwrite: bool,
        shot: Shot,
    ) -> Option<usize> {
        let slot = self.shoot_quietly(env, range, overwrite, shot)?;
        self.enemy_fires();
        Some(slot)
    }

    /// [`shoot`](Self::shoot) without the sound.
    pub(super) fn shoot_quietly(
        &mut self,
        env: &Env,
        range: RangeInclusive<usize>,
        overwrite: bool,
        shot: Shot,
    ) -> Option<usize> {
        let slot = self
            .first_free(range.clone())
            .or_else(|| overwrite.then_some(*range.end()))?;
        self.make_shot(env, slot, shot);
        Some(slot)
    }

    /// Behaviour.md 9: an enemy's shot sounds $08 on both pulses.
    pub(super) fn enemy_fires(&mut self) {
        self.sfx.push(Request::a(Channel::Pulse1, 0x08));
        self.sfx.push(Request::b(Channel::Pulse2, 0x08));
    }

    /// Writes a shot into `slot`. Spec gap and quirk Q17: behaviour.md 2.5 gives a new object
    /// its state's hit points, but shots were seen with the slot's old value, and the
    /// sub-pixel fractions stay as the last occupant left them. The remake clears them.
    pub(super) fn make_shot(&mut self, env: &Env, slot: usize, shot: Shot) -> &mut Object {
        let old = self.slots[slot];
        let o = self.create(env.data, slot, shot.state, shot.frame, shot.x, shot.y);
        if env.original_rules {
            o.hit_points = old.hit_points;
            o.x_fraction = old.x_fraction;
            o.y_fraction = old.y_fraction;
        }
        o.speed_x = shot.speed.0;
        o.speed_y = shot.speed.1;
        o.flags_x = shot.flags.0;
        o.flags_y = shot.flags.1;
        o.index = shot.life;
        o
    }

    /// The common enemy shot of state $15, frame $33 unless given otherwise.
    #[allow(clippy::too_many_arguments)]
    fn enemy_shot(
        &mut self,
        env: &Env,
        x: u16,
        y: u16,
        speed: (u16, u16),
        left: bool,
        down: bool,
        life: u8,
        overwrite: bool,
    ) {
        let shot = Shot {
            state: state::ENEMY_SHOT,
            frame: 0x33,
            x,
            y,
            speed,
            flags: (u8::from(left), u8::from(down)),
            life,
        };
        self.shoot(env, CHILD_SLOTS, overwrite, shot);
    }

    // ---- Turrets, types 01-10 (behaviour.md 4.1) -----------------------------------------------

    fn step_turret(&mut self, slot: usize, env: &mut Env) {
        let t = usize::from(self.slots[slot].state - state::TURRET_FIRST);
        // Types 04, 06, ... 10 are the mirrored variants.
        if t >= 3 && t % 2 == 1 {
            self.slots[slot].flags_x = 1;
        }
        let f_mod_8 = env.f().is_multiple_of(8);
        let (r, r1) = (env.rng.out, env.rng.high);
        let mut fire = false;
        let o = &mut self.slots[slot];
        match o.work6 {
            0 => {
                o.work5 = o.work5.wrapping_sub(1);
                if o.work5.is_multiple_of(64) {
                    o.work5 = 0;
                    o.work6 = 1;
                }
            }
            1 => {
                if f_mod_8 {
                    o.work5 += 1;
                    o.frame = o.frame.wrapping_add(1);
                    if o.work5 == 2 {
                        o.work6 = 2;
                        o.work5 = (r % 16) + 4;
                    }
                }
            }
            2 => {
                o.work5 = o.work5.wrapping_sub(1);
                if o.work5 == 0 {
                    o.work6 = 3;
                    o.work5 = 4;
                }
            }
            3 => {
                o.work5 = o.work5.wrapping_sub(1);
                if o.work5 == 0 {
                    fire = true;
                    o.work6 = 4;
                    o.work5 = 0;
                }
            }
            _ => {
                if f_mod_8 {
                    o.frame = o.frame.wrapping_sub(1);
                    o.work5 += 1;
                    if o.work5 == 2 {
                        o.work6 = 0;
                        o.work5 = (r1 & 0x2F) + 9;
                    }
                }
            }
        }
        if fire {
            let o = self.slots[slot];
            let (dx, dy, sx, sy, down) = TURRET_SHOTS[t];
            let left = if t < 2 { false } else { o.left() };
            self.enemy_shot(
                env,
                o.x.wrapping_add_signed(dx),
                o.y.wrapping_add_signed(dy),
                (sx, sy),
                left,
                down,
                0xC0,
                false,
            );
        }
    }

    // ---- Generators (behaviour.md 4.14) and mines (5.6) ------------------------------------------

    fn step_generator(&mut self, slot: usize, env: &mut Env) {
        let state = self.slots[slot].state;
        // States AE, B1 and B3 act every 8th step; AD, B0 and B2 every step.
        let gated = matches!(state, 0xAE | 0xB1 | 0xB3);
        if gated {
            let o = &mut self.slots[slot];
            o.work6 = o.work6.wrapping_add(1);
            if !o.work6.is_multiple_of(8) {
                return;
            }
        }
        env.rng.out = env.rng.out.wrapping_add(0x61);
        if env.rng.out % 64 != 0x1F || (env.f() >> 8) & 1 != 0 {
            return;
        }
        let (x, y) = edge_position(env);
        match state {
            0xAD | 0xAE => {
                // A mine in the first free slot of 27-30.
                if let Some(free) = self.first_free(27..=30) {
                    let old = self.slots[free];
                    let mine = self.create(env.data, free, state::MINE, 0x9A, x, y);
                    if env.original_rules {
                        // Spec gap: the fire timer a drone later finds in the slot is the one
                        // the slot held before the mine (behaviour.md 2.5).
                        mine.work5 = old.work5;
                    }
                }
            }
            0xB2 | 0xB3 => {
                // A hopper in the first free slot of 27-30.
                if let Some(free) = self.first_free(27..=30) {
                    let hopper = self.create(env.data, free, state::HOPPER, 0xA3, x, y);
                    hopper.work6 = 0xA3;
                }
            }
            _ => {
                // 4B, 4C: a drone in the first free slot of 26-30.
                self.launch_drone(env, x, y, true);
            }
        }
    }

    fn step_mine(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let o = &mut self.slots[slot];
        let c = u16::from(o.index % 32);
        let max = 0x0100 + (c % 8) * 0x20 + if c >= 16 { 0x0100 } else { 0 };
        home_on(o, pod, 8, max);
        if env.f().is_multiple_of(8) {
            o.index = o.index.wrapping_add(1);
        }
        o.frame = 0x9A_u8.wrapping_add(4u8.wrapping_sub(o.hit_points));
    }

    // ---- Egg (behaviour.md 4.17) ---------------------------------------------------------------

    fn step_egg(&mut self, slot: usize) {
        let o = &mut self.slots[slot];
        o.work6 = o.work6.wrapping_add(1);
        if o.work6 >= 64 {
            o.state = 0;
            let index = usize::from(o.index);
            if let Some(entry) = self.present.get_mut(index) {
                *entry = 0;
            }
        } else {
            o.frame = 0x9A + o.work6 / 16;
        }
    }

    // ---- Green orb and bonus objects (behaviour.md 5.4, 5.10) ---------------------------------

    /// The green-orb spawner, after the A pass's split (behaviour.md 5.4): once every 128
    /// steps, on planets. `counter` is the frame counter as the split left it.
    pub fn spawn_orb(&mut self, env: &mut Env, counter_after: u32) {
        let planet = usize::from(env.level.number) < crate::game::level::PLANETS;
        if !planet || counter_after & 0xFF != 1 {
            return;
        }
        // Holding something with at least 3 swings counted makes an orb at every check; else
        // one in four checks, if R's bit 7 is clear.
        let held = env.beam_spin.is_some_and(|spin| spin % 128 >= 3);
        let gate = (counter_after >> 8) & 3 == 3 && env.rng.out & 0x80 == 0;
        if (held || gate)
            && let Some(free) = self.first_free(23..=25)
        {
            let (x, y) = edge_position(env);
            self.create(env.data, free, state::GREEN_ORB, 0xB9, x, y);
        }
    }

    fn step_orb(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let f = env.f();
        let o = &mut self.slots[slot];
        o.frame = [0xB9, 0xBA, 0xBB, 0xB9][((f / 8) % 4) as usize];
        let s_plus_f = (slot as u32 + f) % 256;
        if s_plus_f <= 1 {
            o.index = o.index.wrapping_add(1);
        }
        if o.index >= 16 {
            super::objects::Object::speed_up(&mut o.speed_x, 2);
            o.advance_x();
            o.advance_y();
        } else {
            // While the beam holds a load with 3 or more swings the orb homes faster.
            if env.beam_spin.is_some_and(|spin| spin % 128 >= 3) {
                home_on(o, pod, 0x10, max_speed(0x82));
            } else {
                home_on(o, pod, 2, 0x0080);
            }
        }
        // (s + F) mod 64 is 0 or 1 (behaviour.md 5.4 says 0; the slot parity makes it 1 for odd
        // slots, as with the counter's own test).
        if o.index % 2 == 1 && (slot as u32 + f) & 0x3E == 0 {
            let o = *o;
            self.enemy_shot(
                env,
                o.x,
                o.y,
                (o.speed_x, o.speed_y),
                o.left(),
                o.down(),
                0xFF,
                true,
            );
        }
    }

    fn step_bonus(&mut self, slot: usize) {
        let o = &mut self.slots[slot];
        o.kind = o.kind.wrapping_add(1);
        if o.kind == 0 {
            o.state = 0;
            return;
        }
        if o.state == state::BONUS_STAR {
            let half = o.kind / 2;
            o.frame = 0x55 + (half & 1);
            o.flags_y = if half & 3 == 2 { 0x80 } else { 0 };
            o.flags_x = u8::from(half & 3 == 3);
        }
    }

    /// Releases a bonus object at a hit object's position (behaviour.md 3.5): a star, or a box
    /// for a green orb, in the first free item slot.
    pub fn release_bonus(&mut self, env_data: &super::objdata::ObjectData, from: usize) {
        let origin = self.slots[from];
        let Some(slot) = self.first_free(super::objects::ITEM_SLOTS) else {
            return;
        };
        let (state, frame) = if origin.state == state::GREEN_ORB {
            (state::BONUS_BOX, 0x64)
        } else {
            (state::BONUS_STAR, 0x55)
        };
        let bonus = self.create(env_data, slot, state, frame, origin.x, origin.y);
        bonus.index = 0x82;
    }

    /// Slots 3-10 holding shots (state 7 and up, behaviour.md 3.2).
    pub fn shot_slots(&self) -> impl Iterator<Item = usize> + '_ {
        SHOT_SLOTS.filter(|&s| self.slots[s].state >= 7)
    }
}

/// A random position just outside the screen near the pod's row or column (behaviour.md 2.10;
/// the side edges are the view's, `docs/widescreen.md`).
pub(super) fn edge_position(env: &Env) -> (u16, u16) {
    let cy = env.camera.y;
    let view_left = env.view.left_x();
    let view_right = view_left + env.view.w() + 32;
    let (r1, r2) = (env.rng.high, env.rng.low);
    let level = env.level;
    let map_width = u32::from(level.width) * 128;
    let map_height = level.map_rows() as u32 * 128;
    let pod = &env.pod.body;
    // A side edge, the other side if the first is off the map; neither: top or bottom.
    let (left_fits, right_fits) = (view_left >= 32, view_right < map_width as i32);
    let side = match r2 & 4 != 0 {
        true if left_fits => Some(view_left - 32),
        false if right_fits => Some(view_right),
        true if right_fits => Some(view_right),
        false if left_fits => Some(view_left - 32),
        _ => None,
    };
    if r2 & 2 != 0
        && let Some(x) = side
    {
        let x = x as u16;
        let r = i16::from(r1 % 128);
        let offset = r - 64 + i16::from(r >= 64);
        if env.original_rules {
            // Q10: the offset is added to the low byte only.
            let low = (pod.y as u8).wrapping_add(offset as u8);
            (x, pod.y & 0xFF00 | u16::from(low))
        } else {
            (x, pod.y.saturating_add_signed(offset))
        }
    } else {
        let mut top = r2 & 4 != 0;
        if !top && u32::from(cy) + 208 >= map_height {
            top = true;
        }
        let y = if top { cy.wrapping_sub(48) } else { cy + 208 };
        let r = i16::from(r2 % 64);
        let offset = r - 32 + i16::from(r >= 32);
        if env.original_rules {
            let low = (pod.x as u8).wrapping_add(offset as u8);
            (pod.x & 0xFF00 | u16::from(low), y)
        } else {
            (pod.x.saturating_add_signed(offset), y)
        }
    }
}
