//! Objects made by other objects (`docs/spec/behaviour.md` 2.8, 2.9, 4.7-4.8, 5): aimed shots,
//! homing missiles, flames, exhaust, drones, hoppers and the heads of walkers and pillars.

use super::behave::{Shot, near, slow_and_turn, slow_down};
use super::objects::{CHILD_SLOTS, Env, Object, ObjectTable, SLOTS, state};
use crate::game::gameplay::minions::{AIM, FLAME_FRAMES};
use crate::game::sound::{Channel, Request};

impl ObjectTable {
    /// Turns the object in `slot` into an explosion in place (behaviour.md 3.5, without the
    /// score and the type list): `sequence` is its position in the frame sequence.
    pub(super) fn explode_in_place(&mut self, slot: usize, state: u8, sequence: u8) {
        let o = &mut self.slots[slot];
        o.state = state;
        o.index = 1;
        o.kind = sequence;
    }

    // ---- Aimed shot (behaviour.md 2.8) ---------------------------------------------------------

    /// An aimed shot with spread from the maker in `maker`, placed by table entry `entry`.
    pub(super) fn aimed_shot(&mut self, env: &Env, maker: usize, entry: usize) -> bool {
        let (dx, dy, flags_y, flags_x) = AIM[entry];
        let m = self.slots[maker];
        let (x, y) = (m.x.wrapping_add_signed(dx), m.y.wrapping_add_signed(dy));
        let pod = &env.pod.body;
        let (mut a, mut b) = (u32::from(x.abs_diff(pod.x)), u32::from(y.abs_diff(pod.y)));
        // Up to three doublings of both distances together; the later ones follow only a first
        // (Mesen: a shot with |dy| 135 kept its distances).
        if a < 128 && b < 128 {
            a *= 2;
            b *= 2;
            for _ in 0..2 {
                if a < 256 && b < 256 {
                    a *= 2;
                    b *= 2;
                } else {
                    break;
                }
            }
        }
        let horizontal = a as i32 + i32::from(env.rng.high % 64) - 32;
        let vertical = b as i32 + i32::from(env.rng.low % 64) - 32;
        let pod_right = x < pod.x;
        let pod_below = y < pod.y;
        let flags_y = if flags_y & 0x80 != 0 {
            u8::from(pod_below)
        } else {
            flags_y
        };
        let flags_x = if flags_x & 0x80 != 0 {
            u8::from(!pod_right)
        } else {
            flags_x
        };
        // A negative horizontal sum is a signed speed. A negative vertical sum is read as a
        // huge magnitude by the original (Q12); the remake signs it.
        let (speed_y, flags_y) = if vertical >= 0 {
            (vertical as u16, flags_y)
        } else {
            (vertical.unsigned_abs() as u16, flags_y ^ 1)
        };
        let shot = Shot {
            state: state::ENEMY_SHOT,
            frame: 0x33,
            x,
            y,
            speed: (horizontal as u16, speed_y),
            flags: (flags_x, flags_y),
            life: 0xF0,
        };
        self.shoot(env, CHILD_SLOTS, false, shot).is_some()
    }

    // ---- Homing missile (behaviour.md 2.9) -----------------------------------------------------

    /// Fires a missile from `maker` by one of two table entries chosen by bit 0 of R1.
    /// Returns whether a slot was free.
    pub(super) fn fire_missile(&mut self, env: &Env, maker: usize, entries: (usize, usize)) {
        let entry = if env.rng.high & 1 == 0 {
            entries.0
        } else {
            entries.1
        };
        let (dx, dy, flags_y, flags_x) = AIM[entry];
        let m = self.slots[maker];
        let Some(slot) = self.first_free(CHILD_SLOTS) else {
            return;
        };
        let left = flags_x != 0;
        let upside_down = m.flags_y & 0x80 != 0;
        let frame = match (upside_down, left) {
            (false, false) => 0x38,
            (false, true) => 0x3E,
            (true, false) => 0x3A,
            (true, true) => 0x3C,
        };
        let o = self.create(
            env.data,
            slot,
            state::MISSILE,
            frame,
            m.x.wrapping_add_signed(dx),
            m.y.wrapping_add_signed(dy),
        );
        o.hit_points = 0;
        o.flags_x = flags_x;
        o.flags_y = flags_y;
        o.work5 = 0xC0;
        o.work6 = 0x20;
    }

    pub(super) fn step_missile(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let k = env.f() / 2;
        let o = &mut self.slots[slot];
        if o.work6 > 0 {
            o.work6 -= 1;
            Object::speed_up(&mut o.speed_x, 0x20);
            Object::speed_up(&mut o.speed_y, 0x20);
            o.advance();
        } else {
            if (slot as u32 + k).is_multiple_of(8) {
                o.kind = missile_direction((o.x, o.y), pod);
                o.flags_x |= 2;
            }
            o.frame = 0x37 + o.kind;
            self.missile_motion(slot);
            let o = &mut self.slots[slot];
            // Mesen: the life of 192 steps is counted after the 32 straight ones.
            o.work5 = o.work5.wrapping_sub(1);
            if o.work5 == 0 {
                self.explode_in_place(slot, state::EXPLOSION_LARGE, 5);
                self.sfx.push(Request::a(Channel::Noise, 0x0C));
            }
        }
    }

    /// The speeds and the move of a missile flying in direction `kind` (behaviour.md 2.9 step 4),
    /// shared by the enemy's and the pod's missiles.
    pub(super) fn missile_motion(&mut self, slot: usize) {
        let o = &mut self.slots[slot];
        let direction = usize::from(o.kind);
        const X_ACCEL: [u16; 8] = [0, 0x14, 0x20, 0x14, 0, 0x14, 0x20, 0x14];
        const Y_ACCEL: [u16; 8] = [0x20, 0x14, 0, 0x14, 0x20, 0x14, 0, 0x14];
        // x: the sides of the directions 1-3 are right, 5-7 left.
        let want_left = (5..=7).contains(&direction);
        let mut left = o.left();
        if X_ACCEL[direction] == 0 {
            slow_down(&mut o.speed_x, 0x12);
        } else if left != want_left {
            slow_and_turn(&mut o.speed_x, &mut left, X_ACCEL[direction]);
        } else {
            Object::speed_up(&mut o.speed_x, X_ACCEL[direction]);
        }
        o.flags_x = (o.flags_x & !1) | u8::from(left);
        let want_down = (3..=5).contains(&direction);
        let mut down = o.down();
        if Y_ACCEL[direction] == 0 {
            slow_down(&mut o.speed_y, 0x12);
        } else if down != want_down {
            slow_and_turn(&mut o.speed_y, &mut down, Y_ACCEL[direction]);
        } else {
            Object::speed_up(&mut o.speed_y, Y_ACCEL[direction]);
        }
        o.flags_y = (o.flags_y & !1) | u8::from(down);
        o.advance();
    }

    // ---- Flames (behaviour.md 5.2) -------------------------------------------------------------

    /// A flame of `state` (FLAME or FLAME_LOW) at (x, y) in `slot`.
    pub(super) fn make_flame(&mut self, env: &Env, slot: usize, state: u8, x: u16, y: u16) {
        self.create(env.data, slot, state, 0x60, x, y);
    }

    pub(super) fn step_flame(&mut self, slot: usize) {
        let o = &mut self.slots[slot];
        o.index = o.index.wrapping_add(1);
        let age = usize::from(o.index);
        if age >= 12 {
            o.state = 0;
            return;
        }
        let low = if o.state == state::FLAME { 0 } else { 4 };
        o.frame = FLAME_FRAMES[age] - low;
        o.flags_x = u8::from((6..=10).contains(&age));
    }

    // ---- Exhaust (behaviour.md 5.1) ------------------------------------------------------------

    pub(super) fn step_exhaust(&mut self, slot: usize, env: &mut Env) {
        let owner = self.slots[usize::from(self.slots[slot].work6)];
        let r = env.rng.out;
        let o = &mut self.slots[slot];
        if env.f() % 256 >= 128 && owner.state != 0 {
            o.x = owner.x;
            o.y = owner.y.wrapping_add(22);
            o.frame = [0x44, 0x45, 0x46, 0x45][usize::from(r % 4)];
        } else {
            o.state = 0;
        }
    }

    /// The dome's exhaust: in the first free slot searched from 30 down to 23, else slot 22.
    pub(super) fn make_exhaust(&mut self, env: &Env, owner: usize) {
        let slot = (23..=30)
            .rev()
            .find(|&s| self.slots[s].state == 0)
            .unwrap_or(22);
        let m = self.slots[owner];
        let o = self.create(
            env.data,
            slot,
            state::EXHAUST,
            0x44,
            m.x,
            m.y.wrapping_add(24),
        );
        o.work6 = owner as u8;
    }

    // ---- Drone (behaviour.md 5.5) --------------------------------------------------------------

    /// Launches a drone in the first free slot of 26-30.
    pub(super) fn launch_drone(&mut self, env: &Env, x: u16, y: u16, down: bool) {
        let Some(slot) = self.first_free(26..=30) else {
            return;
        };
        self.sfx.push(Request::a(Channel::Pulse2, 0x18));
        let d = self.create(env.data, slot, state::DRONE, 0x4D, x, y);
        d.speed_y = 0x0200;
        d.flags_y = u8::from(down);
        d.index = 16;
    }

    pub(super) fn step_drone(&mut self, slot: usize, env: &mut Env) {
        let k = env.f() / 2;
        let n = near(&self.slots[slot], &*env.pod);
        let o = &mut self.slots[slot];
        let mut b = o.kind;
        let climb;
        o.index = o.index.wrapping_sub(1);
        if o.index == 0 {
            o.index = 18;
            b = u8::from(!n.pod_below);
            let dy = n.dy;
            if b == 1 && dy >= 16 {
                climb = true;
            } else {
                o.flags_x = (o.flags_x & !1) | u8::from(!n.pod_right);
                if n.dx >= dy {
                    b = 0;
                    climb = false;
                } else if n.dx < dy / 2 {
                    climb = true;
                } else {
                    b = 1;
                    climb = false;
                }
            }
            if climb {
                o.frame = 0x4D;
                o.index = 16;
            }
        } else {
            climb = o.frame >= 0x4D;
        }
        o.kind = b;
        let mut bobs = 0;
        if climb {
            if b == 1 {
                if o.down() {
                    slow_down(&mut o.speed_y, 8);
                    if o.speed_y == 0 {
                        o.flags_y &= !1;
                    }
                } else {
                    Object::speed_up(&mut o.speed_y, 0x0C);
                }
                o.frame = 0x4E + (k % 2) as u8;
            } else {
                bobs += 1;
                o.frame = 0x4D;
            }
        } else {
            let desired_left = o.flags_x & 1 != 0;
            let moving_left = o.flags_x & 0x80 != 0;
            let mut speed_up = true;
            if desired_left != moving_left {
                slow_down(&mut o.speed_x, if b == 1 { 0x0A } else { 0x0F });
                speed_up = false;
                if o.speed_x == 0 {
                    o.flags_x ^= 0x80;
                    speed_up = o.flags_x != 0;
                }
                if !speed_up {
                    bobs += 1;
                }
            }
            if speed_up {
                if b == 1 {
                    Object::speed_up(&mut o.speed_x, 2);
                } else {
                    Object::speed_up(&mut o.speed_x, 4);
                    bobs += 1;
                }
            }
            o.frame = 0x47 + [0, 1, 2, 2][(k % 4) as usize] + 3 * b;
        }
        // The bobs come before the move (behaviour.md 5.5, "Bob").
        for _ in 0..bobs {
            let o = &mut self.slots[slot];
            if o.down() {
                Object::speed_up(&mut o.speed_y, 4);
            } else {
                slow_down(&mut o.speed_y, 6);
                if o.speed_y == 0 {
                    o.flags_y |= 1;
                }
            }
            self.drone_tick(slot, env);
        }
        let o = &mut self.slots[slot];
        o.advance_x_toward(o.flags_x & 0x80 != 0);
        o.advance_y();
        self.drone_tick(slot, env);
    }

    /// One decrement of the drone's fire timer; a shot every 64th (behaviour.md 5.5 step 5).
    fn drone_tick(&mut self, slot: usize, env: &Env) {
        let o = &mut self.slots[slot];
        o.work5 = o.work5.wrapping_sub(1);
        if !o.work5.is_multiple_of(64) {
            return;
        }
        let o = *o;
        let dx: i16 = if o.flags_x != 0 { -10 } else { 10 };
        let shot = Shot {
            state: state::ENEMY_SHOT,
            frame: 0x50,
            x: o.x.wrapping_add_signed(dx),
            y: o.y.wrapping_sub(2),
            speed: (0x0400, o.speed_y),
            flags: (o.flags_x, o.flags_y),
            life: 0x80,
        };
        self.shoot(env, CHILD_SLOTS, false, shot);
    }

    // ---- Hopper (behaviour.md 5.7) -------------------------------------------------------------

    pub(super) fn step_hopper(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let n = near(&self.slots[slot], &*env.pod);
        if self.slots[slot].index.is_multiple_of(8) {
            let o = self.slots[slot];
            let facing = if o.flags_x == 0 {
                n.pod_right
            } else {
                !n.pod_right
            };
            if n.dy < 24 {
                self.slots[slot].kind = 1;
                if facing {
                    let dx: i16 = if o.flags_x == 0 { 8 } else { -8 };
                    let target = self.first_free(CHILD_SLOTS);
                    if let Some(target) = target {
                        let shot = Shot {
                            state: state::ENEMY_SHOT,
                            frame: 0x33,
                            x: o.x.wrapping_add_signed(dx),
                            y: o.y,
                            speed: (0x0400, o.speed_y),
                            flags: (o.flags_x, o.flags_y),
                            life: 0xC0,
                        };
                        self.make_shot(env, target, shot);
                        self.enemy_fires();
                    }
                }
            } else {
                self.slots[slot].kind = if n.pod_below {
                    5
                } else {
                    (2 + ((n.dy - 24) / 8) % 4).min(4) as u8
                };
            }
        }
        let o = &mut self.slots[slot];
        o.index = o.index.wrapping_add(1);
        let toward = if n.pod_right { !o.left() } else { o.left() };
        let mut fall = true;
        match o.kind {
            5 => {
                o.work5 = 0;
                slow_down(&mut o.speed_x, 0x0E);
                if f.is_multiple_of(4) && o.work6 < 0xA6 {
                    o.work6 += 1;
                }
            }
            1 => {
                if o.work6 != 0xA3 {
                    if f.is_multiple_of(4) {
                        o.work6 -= 1;
                    }
                } else if toward {
                    Object::speed_up(&mut o.speed_x, 0x0C);
                    o.work5 = 1;
                } else {
                    slow_down(&mut o.speed_x, 4);
                    o.work5 = 0;
                    if o.speed_x == 0 {
                        o.flags_x ^= 1;
                        Object::speed_up(&mut o.speed_x, 0x0C);
                        o.work5 = 1;
                    }
                }
            }
            b @ 2..=4 => {
                fall = false;
                o.work6 = 0xA2 + b;
                if toward {
                    o.work5 = 1;
                    Object::speed_up(&mut o.speed_x, u16::from(4 - b) * 8);
                    // Spec gap: the vertical change belongs to the approach only (Mesen).
                    let amount = u16::from(b - 2) * 8;
                    if o.down() {
                        let mut down = true;
                        slow_and_turn(&mut o.speed_y, &mut down, amount);
                        o.flags_y = (o.flags_y & !1) | u8::from(down);
                    } else {
                        Object::speed_up(&mut o.speed_y, amount);
                    }
                } else {
                    o.work5 = 0;
                    let mut left = o.left();
                    slow_and_turn(&mut o.speed_x, &mut left, 0x0A);
                    o.flags_x = (o.flags_x & !1) | u8::from(left);
                }
            }
            _ => {}
        }
        if fall {
            if o.down() {
                Object::speed_up(&mut o.speed_y, 0x1A);
            } else {
                slow_down(&mut o.speed_y, 0x1E);
                if o.speed_y == 0 {
                    o.flags_y |= 1;
                    Object::speed_up(&mut o.speed_y, 0x1A);
                }
            }
        }
        o.advance();
        o.frame = o.work6;
        if o.work5 == 1 {
            o.frame += [4, 8, 4, 0][usize::from(o.index % 4)];
        }
    }

    // ---- Heads of walkers and pillars (behaviour.md 5.3) -----------------------------------------

    pub(super) fn step_head(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let o = self.slots[slot];
        let owner = usize::from(o.work6);
        if owner == 0 || self.slots[owner].state == 0 {
            self.slots[slot].state = 0;
            return;
        }
        let owner = self.slots[owner];
        let height = if matches!(owner.state, state::PILLAR_A | state::PILLAR_B) {
            u16::from(owner.frame.wrapping_sub(0x57))
        } else {
            match owner.frame {
                0x5D => 25,
                0x5E => 20,
                _ => 30,
            }
        };
        let o = &mut self.slots[slot];
        o.x = owner.x;
        o.y = owner.y.wrapping_sub(height);
        let n = near(o, &*env.pod);
        // Spec gap: in a step in which the head shoots, it leaves its frame as it was
        // (Mesen: new pillar heads kept their first frame); one that does not shoot shows the
        // frame of its animation.
        let mut shoot = false;
        let frame;
        let flags;
        let mut retry = false;
        if o.state == state::HEAD_A {
            let p = usize::from((o.kind % 16) / 4);
            if p != 0 {
                o.kind = o.kind.wrapping_add(1);
                frame = [0x55, 0x56, 0x57, 0x56][p];
                flags = [0, 0, 0, 1][p];
            } else {
                frame = 0x55;
                flags = 0;
                if f % 16 == 8 {
                    o.kind = o.kind.wrapping_add(1);
                } else if f.is_multiple_of(16) {
                    shoot = true;
                    retry = true;
                }
            }
        } else {
            let q = usize::from((o.kind % 32) / 4);
            frame = [0x58, 0x59, 0x57, 0x59, 0x58, 0x5B, 0x5A, 0x5B][q];
            flags = [0, 0, 0, 1, 1, 1, 0, 0][q];
            if q % 4 != 0 || f % 16 == 8 {
                o.kind = o.kind.wrapping_add(1);
            } else if f.is_multiple_of(16) && ((q == 0 && !n.pod_right) || (q == 4 && n.pod_right))
            {
                shoot = true;
            }
        }
        let mut fired = false;
        if shoot {
            let dx: i16 = if n.pod_right { 24 } else { -24 };
            let shot = Shot {
                state: state::HEAD_SHOT,
                frame: 0x5C,
                x: self.slots[slot].x.wrapping_add_signed(dx),
                y: self.slots[slot].y,
                speed: (0x0800, 0),
                flags: (u8::from(!n.pod_right), 0),
                life: 0x40,
            };
            fired = self.shoot(env, CHILD_SLOTS, false, shot).is_some();
        }
        let o = &mut self.slots[slot];
        if !fired {
            o.frame = frame;
            o.flags_x = flags;
            if shoot && retry {
                o.kind = o.kind.wrapping_add(1);
            }
        }
    }

    /// Keeps the head of a walker or pillar: a new one in the first free slot above the owner
    /// when it has none; the owner is destroyed if its head is exploding. Returns whether the
    /// owner lives on.
    pub(super) fn keep_head(
        &mut self,
        env: &mut Env,
        slot: usize,
        head_state: u8,
        head_frame: u8,
        height: u16,
    ) -> bool {
        let head = usize::from(self.slots[slot].work6);
        if head != 0 {
            match self.slots[head].state {
                4 | 5 => {
                    self.destroy(env.data, slot, env.score);
                    return false;
                }
                0 => {}
                _ => return true,
            }
        }
        if let Some(free) = (slot + 1..SLOTS).find(|&s| self.slots[s].state == 0) {
            let o = self.slots[slot];
            let h = self.create(
                env.data,
                free,
                head_state,
                head_frame,
                o.x,
                o.y.wrapping_sub(height),
            );
            h.work6 = slot as u8;
            self.slots[slot].work6 = free as u8;
        }
        true
    }
}

/// The direction (0 up, 2 right, 4 down, 6 left, odd numbers diagonal) from a missile to its
/// target (behaviour.md 2.9).
pub(super) fn missile_direction(missile: (u16, u16), target: (u16, u16)) -> u8 {
    let left = target.0 < missile.0;
    let above = target.1 < missile.1;
    let ax = u32::from(target.0.abs_diff(missile.0));
    let ay = u32::from(target.1.abs_diff(missile.1));
    if ax >= ay {
        if ax >= 2 * ay {
            return if left { 6 } else { 2 };
        }
    } else {
        if ay >= 2 * ax {
            return if above { 0 } else { 4 };
        }
    }
    match (left, above) {
        (false, true) => 1,
        (false, false) => 3,
        (true, false) => 5,
        (true, true) => 7,
    }
}
