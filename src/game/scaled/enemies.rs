//! The placed objects of levels 1-7 (`docs/spec/behaviour.md` 4): domes, machines, guns,
//! walkers, pillars, turrets, magnets, creatures, formations and the satellite with its
//! companion.

use super::behave::{Near, Shot, home_on, max_speed, near, slow_and_turn_overshoot};
use super::objects::{CHILD_SLOTS, Env, Object, ObjectTable, PLACED_SLOTS, SHOT_SLOTS, state};
use super::tables::SIN;
use crate::game::gameplay::enemies::{
    CIRCLE, FORMATION_A_FRAMES, FORMATION_B_FRAMES, FORMATION_MUZZLE, ROUTES,
};

/// What a green creature does after choosing its axis again.
enum Chosen {
    Go,
    Pause,
}

impl ObjectTable {
    // ---- Waypoint shuttle (behaviour.md 2.7) -----------------------------------------------------

    /// The target the creature in `slot` is heading for, after flipping to the other waypoint
    /// when it is within 16 of this one. None if the level has no route for it.
    fn waypoint(&mut self, slot: usize, env: &Env) -> Option<(u16, u16)> {
        let o = &mut self.slots[slot];
        let &(_, _, first, second) = ROUTES
            .iter()
            .find(|r| r.0 == env.level.number && r.1 == o.index)?;
        let target = if o.work5 & 0x80 == 0 { first } else { second };
        let dx = u32::from(o.x.abs_diff(target.0));
        let dy = u32::from(o.y.abs_diff(target.1));
        if dx < 256 && dy < 256 && dx + dy < 16 {
            o.work5 ^= 0x80;
            return Some(if o.work5 & 0x80 == 0 { first } else { second });
        }
        Some(target)
    }

    // ---- Domes (behaviour.md 4.7) ------------------------------------------------------------------

    pub(super) fn step_dome(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let s = slot as u32;
        let o = self.slots[slot];
        let lifted = o.work5 & 2 != 0;
        let pod = (env.pod.body.x, env.pod.body.y);
        if lifted {
            let m = (s + f) % 256;
            if m < 128 {
                self.slots[slot].advance();
            } else if m == 128 {
                self.make_exhaust(env, slot);
                return;
            } else {
                let o = &mut self.slots[slot];
                home_on(o, pod, 3, 0x0080);
                o.flags_x |= 2;
            }
        } else if o.state == state::DOME_LIFT {
            if f.is_multiple_of(64) && env.rng.high % 8 == 3 {
                self.slots[slot].work5 |= 2;
            }
        } else {
            self.dome_flames(slot, env);
        }
        // The aimed shot.
        let o = &mut self.slots[slot];
        o.work6 = o.work6.wrapping_sub(1);
        if o.work6.is_multiple_of(32) {
            let entry = match env.rng.out % 4 {
                1 => 0,
                2 => 1,
                _ => 2,
            };
            // Spec gap: a step that makes the aimed shot ends there (Mesen: the opening cycle
            // and the drone skip those steps; with no free slot they run).
            if self.aimed_shot(env, slot, entry) {
                return;
            }
        }
        self.opening_cycle(slot, env, 0x34, 0x36, (3, 4));
        if self.slots[slot].state == state::DOME && env.rng.high % 64 == 0x17 {
            let o = self.slots[slot];
            self.launch_drone(env, o.x, o.y.wrapping_add(16), true);
        }
    }

    /// The type 11 dome's pull on the pod and its flames (behaviour.md 4.7 step 3).
    fn dome_flames(&mut self, slot: usize, env: &mut Env) {
        let o = self.slots[slot];
        let n = near(&o, &*env.pod);
        if n.d < 255 && (16..56).contains(&n.dx) && n.dy < 8 && o.y < env.pod.body.y {
            let flame_slot = if n.pod_right { 25 } else { 24 };
            if self.slots[flame_slot].state == 0 {
                let body = &mut env.pod.body;
                body.v_down = false;
                body.y = body.y.wrapping_sub(n.dy as u16);
                let dx: i16 = if n.pod_right { 31 } else { -31 };
                self.make_flame(
                    env,
                    flame_slot,
                    state::FLAME,
                    o.x.wrapping_add_signed(dx),
                    o.y.wrapping_add(7),
                );
            }
        }
        let r1 = env.rng.high;
        if self.slots[24].state == 0 && self.slots[25].state == 0 && (r1 >> 1) % 64 == 0x17 {
            let right = r1 & 1 != 0;
            let dx: i16 = if right { 31 } else { -31 };
            self.make_flame(
                env,
                if right { 25 } else { 24 },
                state::FLAME,
                o.x.wrapping_add_signed(dx),
                o.y.wrapping_add(7),
            );
        }
    }

    /// The opening cycle with a missile (behaviour.md 4.8).
    fn opening_cycle(
        &mut self,
        slot: usize,
        env: &mut Env,
        rest: u8,
        open: u8,
        entries: (usize, usize),
    ) {
        let k = env.f() / 2;
        let frame = self.slots[slot].frame;
        if frame != rest {
            if !(slot as u32 + k).is_multiple_of(16) {
                return;
            }
            if self.slots[slot].work5 & 1 == 0 {
                if frame != open {
                    self.slots[slot].frame += 1;
                } else {
                    self.fire_missile(env, slot, entries);
                    self.slots[slot].work5 |= 1;
                }
            } else {
                let o = &mut self.slots[slot];
                o.frame -= 1;
                if o.frame == rest {
                    o.work5 &= !1;
                }
            }
        } else if env.f().is_multiple_of(32) && env.rng.out.is_multiple_of(8) {
            self.slots[slot].frame += 1;
            env.rng.out = env.rng.out.wrapping_add(0x49);
        }
    }

    // ---- Wide machines (behaviour.md 4.6) --------------------------------------------------------

    pub(super) fn step_machine(&mut self, slot: usize, env: &mut Env) {
        let upright = self.slots[slot].state == state::MACHINE_A;
        let o = &mut self.slots[slot];
        if !upright {
            o.flags_y = 0x81;
        }
        o.work6 = o.work6.wrapping_sub(1);
        if o.work6.is_multiple_of(32) && self.aimed_shot(env, slot, if upright { 7 } else { 10 }) {
            // Spec gap: the step ends with the aimed shot (see the domes).
            return;
        }
        self.opening_cycle(slot, env, 0x3F, 0x41, if upright { (5, 6) } else { (8, 9) });
    }

    // ---- Ground guns (behaviour.md 4.2) ----------------------------------------------------------

    pub(super) fn step_gun(&mut self, slot: usize, env: &mut Env) {
        let o = &mut self.slots[slot];
        match o.state {
            state::GUN_47 => o.flags_x = 1,
            state::GUN_18 => o.flags_y = 0x81,
            _ => {}
        }
        // One pixel a step back toward home.
        let (home_x, home_y) = (o.speed_x, o.speed_y);
        o.x = o.x.wrapping_add_signed(match o.x.cmp(&home_x) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -1,
        });
        o.y = o.y.wrapping_add_signed(match o.y.cmp(&home_y) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -1,
        });
        let o = *o;
        // The pair guns (17 on the floor, 18 on the ceiling) look for each other by the x low
        // byte; a gun without a partner is a single gun for that step (behaviour.md 4.2).
        let paired = match o.state {
            state::GUN_17 | state::GUN_18 => self.find_partner(slot),
            _ => None,
        };
        let work5 = o.work5.wrapping_sub(1);
        if let Some(partner) = paired {
            // Only the floor gun acts: the ceiling gun leaves its counter alone.
            if o.state == state::GUN_17 {
                self.slots[slot].work5 = work5;
                if work5 % 64 == 8 {
                    self.slots[slot].work5 = 0x30;
                    if let Some(free) = self.first_free(PLACED_SLOTS) {
                        let distance = (o.y as u8).wrapping_sub(self.slots[partner].y as u8);
                        let y = o.y.wrapping_sub(u16::from(distance / 2));
                        self.make_flame(env, free, state::FLAME_LOW, o.x, y);
                    }
                }
            }
            return;
        }
        self.slots[slot].work5 = work5;
        if work5.is_multiple_of(32) {
            let upside_down = o.flags_y & 0x80 != 0;
            let dx: i16 = if o.left() { -2 } else { 2 };
            let dy: i16 = if upside_down { 4 } else { -4 };
            let shot = Shot {
                state: state::GUN_SHOT,
                frame: 0x52,
                x: o.x.wrapping_add_signed(dx),
                y: o.y.wrapping_add_signed(dy),
                speed: (0, 0x0800),
                flags: (0, o.flags_y & 1),
                life: 0x80,
            };
            self.shoot(env, CHILD_SLOTS, false, shot);
        }
    }

    /// The partner of the pair gun in `slot`: the slot kept in its work 6 while that slot still
    /// holds the other kind of gun, else a search of slots 23-30 for the other kind with the same
    /// x low byte (behaviour.md 4.2). A link that broke is cleared and the gun acts alone for
    /// that step.
    fn find_partner(&mut self, slot: usize) -> Option<usize> {
        let o = self.slots[slot];
        let wanted = if o.state == state::GUN_17 {
            state::GUN_18
        } else {
            state::GUN_17
        };
        if o.work6 != 0 {
            let linked = usize::from(o.work6);
            if self.slots[linked].state == wanted {
                return Some(linked);
            }
            self.slots[slot].work6 = 0;
            return None;
        }
        let found = PLACED_SLOTS
            .into_iter()
            .find(|&s| self.slots[s].state == wanted && self.slots[s].x as u8 == o.x as u8)?;
        self.slots[slot].work6 = found as u8;
        Some(found)
    }

    // ---- Walkers (behaviour.md 4.3) and pillars (4.4) ----------------------------------------------

    pub(super) fn step_walker(&mut self, slot: usize, env: &mut Env) {
        let type_a = self.slots[slot].state == state::WALKER_A;
        let (head_state, head_frame) = if type_a {
            (state::HEAD_A, 0x50)
        } else {
            (state::HEAD_B, 0x53)
        };
        if !self.keep_head(env, slot, head_state, head_frame, 25) {
            return;
        }
        let r = env.rng.out;
        let o = &mut self.slots[slot];
        if o.speed_y == 0 {
            o.work5 = o.work5.wrapping_sub(1);
            if o.frame == 0x5E {
                if o.work5.is_multiple_of(8) {
                    if r.is_multiple_of(4) {
                        o.speed_x = o.y;
                        o.flags_y &= !1;
                        o.speed_y = 0x0800;
                        o.frame = 0x5F;
                    } else {
                        o.frame = 0x5D;
                    }
                }
            } else if o.work5.is_multiple_of(64) {
                o.frame = 0x5E;
            }
        } else if !o.down() {
            o.speed_y = o.speed_y.saturating_sub(0x80);
            if o.speed_y == 0 {
                o.flags_y |= 1;
                o.speed_y = 1;
            } else {
                o.advance_y();
            }
        } else {
            Object::speed_up(&mut o.speed_y, 0x80);
            o.advance_y();
            if o.y >= o.speed_x {
                o.y = o.speed_x;
                o.speed_y = 0;
                o.frame = 0x5E;
            }
        }
    }

    pub(super) fn step_pillar(&mut self, slot: usize, env: &mut Env) {
        let type_a = self.slots[slot].state == state::PILLAR_A;
        let (head_state, head_frame) = if type_a {
            (state::HEAD_A, 0x58)
        } else {
            (state::HEAD_B, 0x55)
        };
        if !self.keep_head(env, slot, head_state, head_frame, 17) {
            return;
        }
        let o = &mut self.slots[slot];
        o.speed_y = 0x0100;
        if !o.down() {
            if o.frame == 0x7D {
                o.flags_y |= 1;
            } else {
                o.frame += 1;
                o.advance_y();
            }
        } else if o.frame == 0x68 {
            o.flags_y &= !1;
        } else {
            o.frame -= 1;
            o.advance_y();
        }
    }

    // ---- Hourglass turrets (behaviour.md 4.5) ----------------------------------------------------

    pub(super) fn step_hourglass(&mut self, slot: usize, env: &mut Env) {
        let o = &mut self.slots[slot];
        if o.state == state::HOURGLASS_UP {
            o.flags_y = 0x81;
        }
        let before = o.work5;
        o.work5 = before.wrapping_sub(1);
        let o = *o;
        match o.state {
            state::HOURGLASS if before.is_multiple_of(128) => {
                self.launch_drone(env, o.x, o.y.wrapping_sub(16), false);
            }
            state::HOURGLASS_UP if before.is_multiple_of(128) => {
                self.launch_drone(env, o.x, o.y.wrapping_add(16), true);
            }
            state::BLOCK if before.is_multiple_of(64) => {
                if before & 0x40 == 0 {
                    self.launch_drone(env, o.x, o.y.wrapping_add(24), true);
                } else {
                    self.launch_drone(env, o.x, o.y.wrapping_sub(24), false);
                }
            }
            _ => {}
        }
    }

    // ---- Magnets (behaviour.md 4.9) ----------------------------------------------------------------

    pub(super) fn step_magnet(&mut self, slot: usize, env: &mut Env) {
        let pull = self.slots[slot].state == state::MAGNET_PULL;
        let o = self.slots[slot];
        let n = near(&o, &*env.pod);
        if n.d >= 191 {
            return;
        }
        let spin = u16::from(o.work5) << 8 | u16::from(o.work6);
        let spin = spin.wrapping_add(2 * (192 - n.d) as u16);
        let m = &mut self.slots[slot];
        m.work5 = (spin >> 8) as u8;
        m.work6 = spin as u8;
        let i = usize::from(m.work5 % 4);
        let table = if pull {
            [0x7E, 0x7F, 0x7F, 0x7E]
        } else {
            [0x80, 0x81, 0x81, 0x80]
        };
        m.frame = table[i];
        m.flags_x = [0, 0, 1, 1][i];
        if env.pod.antigravity_active() {
            // behaviour.md 4.9: the force is skipped while anti-gravity is on.
            return;
        }
        let vertical = ((192 - n.dy) / 8) as u16;
        let horizontal = if pull {
            (192 - n.d) / 8
        } else {
            (192 - n.dx) / 8
        } as u16;
        let body = &mut env.pod.body;
        let toward_x = if o.x >= body.x {
            !body.h_left
        } else {
            body.h_left
        };
        let toward_y = if o.y >= body.y {
            body.v_down
        } else {
            !body.v_down
        };
        let (mut h, mut v) = (body.h_speed(), body.v_speed());
        if toward_x == pull {
            Object::speed_up(&mut h, horizontal);
        } else {
            slow_and_turn_overshoot(&mut h, &mut body.h_left, horizontal);
        }
        if toward_y == pull {
            Object::speed_up(&mut v, vertical);
        } else {
            slow_and_turn_overshoot(&mut v, &mut body.v_down, vertical);
        }
        body.set_h_speed(h);
        body.set_v_speed(v);
    }

    // ---- Large creature, type 21 (behaviour.md 4.10) ---------------------------------------------

    pub(super) fn step_creature(&mut self, slot: usize, env: &mut Env) {
        let target = self.waypoint(slot, env);
        let (r1, r) = (env.rng.high, env.rng.out);
        let o = &mut self.slots[slot];
        let before = o.speed_y as u8;
        if let Some(target) = target {
            o.home_y(target.1, 1, 0x0080);
        }
        o.flags_y = if o.down() { 0x81 } else { 0 };
        let growing = (o.speed_y as u8) > before;
        let o = *o;
        if growing {
            let e: i16 = if o.down() { -25 } else { 25 };
            for (thruster, dx, left) in [(15usize, 18i16, 1u8), (16, -18, 0)] {
                self.write_thruster(env, thruster, slot, &o, dx, e, left, r);
            }
        } else {
            self.slots[15].frame = 0xFF;
            self.slots[16].frame = 0xFF;
        }
        let n = near(&o, &*env.pod);
        let m = &mut self.slots[slot];
        m.work6 = m.work6.wrapping_sub(1);
        let work6 = m.work6;
        // Spec gap: the distance test sees only the low byte of |dy| (Mesen: a creature 302 px
        // above the pod shot).
        if work6.is_multiple_of(32) && (n.dy & 0xFF) < 80 {
            let dx: i16 = if n.pod_right { 23 } else { -23 };
            let shot = Shot {
                state: state::ENEMY_SHOT,
                frame: 0x84,
                x: o.x.wrapping_add_signed(dx),
                y: o.y,
                speed: (0x0400, 0),
                flags: (u8::from(!n.pod_right), o.flags_y),
                life: 0xC0,
            };
            self.shoot_quietly(env, 17..=22, false, shot);
        } else if work6 % 32 == 16 {
            let dy: i16 = if o.down() { 32 } else { -32 };
            let shot = Shot {
                state: state::ENEMY_SHOT,
                frame: 0x33,
                x: o.x,
                y: o.y.wrapping_add_signed(dy),
                speed: (u16::from(r1), 0x0500),
                flags: (u8::from(r & 2 != 0), o.flags_y),
                life: 0xC0,
            };
            self.shoot_quietly(env, 17..=22, false, shot);
        }
    }

    /// A thruster flame of the creature in `owner`, written into a shot slot whatever it held
    /// (behaviour.md 4.10, Q8).
    #[allow(clippy::too_many_arguments)]
    fn write_thruster(
        &mut self,
        env: &Env,
        slot: usize,
        owner: usize,
        creature: &Object,
        dx: i16,
        dy: i16,
        left: u8,
        r: u8,
    ) {
        let x = creature.x.wrapping_add_signed(dx);
        let y = creature.y.wrapping_add_signed(dy);
        let frame = 0x89 + r % 4;
        let t = self.create(env.data, slot, state::THRUSTER, frame, x, y);
        t.flags_x = left;
        t.flags_y = creature.flags_y;
        t.kind = owner as u8;
    }

    // ---- Large machine, type 22 (behaviour.md 4.11) -----------------------------------------------

    pub(super) fn step_big_machine(&mut self, slot: usize, env: &mut Env) {
        let target = self.waypoint(slot, env);
        let r1 = env.rng.high;
        if let Some(target) = target {
            self.slots[slot].home_x(target.0, 1, 0x0080);
        }
        let o = &mut self.slots[slot];
        o.work6 = o.work6.wrapping_sub(1);
        let tick = o.work6.is_multiple_of(128);
        let mut volley = false;
        if tick {
            if o.frame == 0x85 {
                o.frame = 0x86;
                o.work6 = 7;
                o.work5 &= !7;
            } else {
                o.work5 = o.work5.wrapping_add(1);
                o.frame = [0x86, 0x87, 0x87, 0x86, 0x85][usize::from(o.work5 % 8)];
                volley = o.work5 % 8 == 2;
                o.work6 = if o.frame == 0x85 { 0 } else { 7 };
            }
        }
        let o = *o;
        if volley {
            let e: i16 = if o.left() { 1 } else { -1 };
            for i in 0..4usize {
                let down = i % 2 == 1;
                let y = if i < 2 {
                    o.y.wrapping_sub(20)
                } else {
                    o.y.wrapping_add(12)
                };
                let (speed_x, speed_y) = (0x0300, 0x00D8);
                let shot = Shot {
                    state: state::ENEMY_SHOT,
                    frame: 0x88,
                    x: o.x.wrapping_add_signed(e),
                    y,
                    speed: (speed_x, speed_y),
                    flags: (o.flags_x, u8::from(down)),
                    life: 0xC0,
                };
                let target = 15 + i;
                if self.slots[target].state == 0 {
                    // The large machine's shots make no sound (Mesen).
                    self.make_shot(env, target, shot);
                }
            }
        } else if !tick && r1 % 64 == 0x16 && self.slots[19].state == 0 {
            let dx: i16 = if o.left() { -12 } else { 12 };
            let shot = Shot {
                state: state::ENEMY_SHOT,
                frame: 0x33,
                x: o.x.wrapping_add_signed(dx),
                y: o.y.wrapping_add(1),
                speed: (0x0400, 0),
                flags: (o.flags_x & 1, 0),
                life: 0xC0,
            };
            self.make_shot(env, 19, shot);
        }
    }

    // ---- Formations (behaviour.md 4.12) ------------------------------------------------------------

    /// Makes the part in `slot` the leader of the parts of its state.
    fn become_leader(&mut self, slot: usize, reset_phase: bool) {
        let state = self.slots[slot].state;
        if reset_phase {
            self.slots[slot].x_fraction = 0;
        }
        self.slots[slot].work6 |= 2;
        for other in PLACED_SLOTS {
            if other != slot && self.slots[other].state == state {
                self.slots[other].work5 = slot as u8;
                self.slots[other].work6 &= !2;
            }
        }
    }

    pub(super) fn step_formation(&mut self, slot: usize, env: &mut Env) {
        let o = self.slots[slot];
        let state = o.state;
        if o.work6 & 1 == 0 {
            let m = &mut self.slots[slot];
            m.speed_x = m.x;
            m.speed_y = m.y;
            m.work6 |= 1;
            let leader = PLACED_SLOTS.into_iter().find(|&s| {
                s != slot && self.slots[s].state == state && self.slots[s].work6 & 2 != 0
            });
            match leader {
                Some(leader) => self.slots[slot].work5 = leader as u8,
                None => self.become_leader(slot, true),
            }
        } else if o.work6 & 2 == 0 && self.slots[usize::from(o.work5)].state != state {
            self.become_leader(slot, false);
        }
        // A follower copies its leader's phase.
        let m = self.slots[slot];
        if m.work6 & 2 == 0 {
            self.slots[slot].x_fraction = self.slots[usize::from(m.work5)].x_fraction;
        }
        // The shot comes first, from the previous step's place and picture.
        if state == state::FORMATION_B {
            let m = &mut self.slots[slot];
            m.y_fraction = m.y_fraction.wrapping_sub(1);
            if m.y_fraction == 0 {
                m.y_fraction = env.rng.out;
                let m = *m;
                let index = usize::from(m.frame.clamp(0x91, 0x95) - 0x91);
                let (mut dx, mut dy) = FORMATION_MUZZLE[index];
                let flags_x = m.flags_x ^ 1;
                if flags_x & 1 != 0 {
                    dx = -dx;
                }
                let up = m.flags_y & 0x80 != 0;
                if up {
                    dy = -dy;
                }
                let shot = Shot {
                    state: state::ENEMY_SHOT,
                    frame: 0x33,
                    x: m.x.wrapping_add_signed(dx),
                    y: m.y.wrapping_add_signed(dy),
                    speed: (dx.unsigned_abs() * 64, dy.unsigned_abs() * 64),
                    flags: (flags_x, u8::from(!up)),
                    life: 0xC0,
                };
                self.shoot(env, CHILD_SLOTS, false, shot);
            }
        }
        let m = &mut self.slots[slot];
        m.x_fraction = m.x_fraction.wrapping_add(1);
        let first = if state == state::FORMATION_A {
            0x23
        } else {
            0x33
        };
        let a = m
            .x_fraction
            .wrapping_add(16u8.wrapping_mul(m.kind.wrapping_sub(first)));
        let (q, i) = (a / 64, a % 64);
        let j = usize::from(if q % 2 == 1 { 63 - i } else { i });
        let (home_x, home_y) = (m.speed_x, m.speed_y);
        let along = u16::from(CIRCLE[j]);
        let across = u16::from(CIRCLE[j + 64]);
        m.x = if q < 2 {
            home_x.wrapping_add(along)
        } else {
            home_x.wrapping_sub(along)
        };
        m.y = if q == 0 || q == 3 {
            home_y.wrapping_sub(across)
        } else {
            home_y.wrapping_add(across)
        };
        if state == state::FORMATION_B {
            let n = u16::from(a / 8) + 1;
            m.frame = FORMATION_B_FRAMES[usize::from(a / 8)];
            m.flags_x = u8::from(n % 32 >= 16);
            m.flags_y = if (8..24).contains(&n) { 0x80 } else { 0 };
        } else {
            let k = usize::from(if q % 2 == 1 { i } else { 63 - i });
            m.frame = FORMATION_A_FRAMES[k / 8];
        }
    }

    // ---- Green creatures (behaviour.md 4.13) -------------------------------------------------------

    pub(super) fn step_green(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let n = near(&self.slots[slot], &*env.pod);
        let pod = (env.pod.body.x, env.pod.body.y);
        let r = (env.rng.out, env.rng.out);
        let o = &mut self.slots[slot];
        let base = if o.kind <= 0x44 { 0x96 } else { 0x98 };
        if o.work5 == 0 {
            (o.flags_x, o.flags_y) = match o.kind {
                0x43 => (0, 0),
                0x44 => (1, 0),
                0x45 => (0, 0),
                _ => (0, 0x81),
            };
            o.work5 = 0xFF;
        } else if o.work5 % 128 != 1 {
            o.work5 = o.work5.wrapping_sub(1);
        } else {
            self.green_chase(slot, f, &n, pod, r);
            return;
        }
        let o = &mut self.slots[slot];
        o.frame = base + (o.work5 / 4) % 2;
    }

    /// The chase of an awake green creature.
    fn green_chase(&mut self, slot: usize, f: u32, n: &Near, pod: (u16, u16), r: (u8, u8)) {
        let o = &mut self.slots[slot];
        // The choice of the axis: what the creature does next.
        let choose = |o: &mut Object| -> Chosen {
            // Q5: when the creature is left of the pod the low byte of |dx| is taken as 64.
            let dx = if n.pod_right {
                (n.dx & 0xFF00) | 64
            } else {
                n.dx
            };
            let mode = if n.dy_full < dx {
                o.y_fraction = r.0 % 16 + 64;
                o.speed_y = 0;
                1
            } else {
                o.x_fraction = r.1 % 16 + 65;
                o.speed_x = 0;
                0xFF
            };
            if o.work6 == 0 {
                o.work6 = mode;
                Chosen::Go
            } else if o.work6 == mode {
                // Quirk Q25: a repeated choice goes on.
                Chosen::Go
            } else {
                o.work6 = 16;
                Chosen::Pause
            }
        };
        // Choosing again and the action that follows happen in one step (Mesen: a pause starts
        // counting in the step that begins it, and its end starts the new mode in that step;
        // the new timer is counted down in the step it is set).
        for _ in 0..3 {
            match o.work6 {
                0 => {
                    choose(o);
                }
                1 => {
                    o.y_fraction = o.y_fraction.wrapping_sub(1);
                    if o.y_fraction == 0 {
                        match choose(o) {
                            Chosen::Pause => continue,
                            Chosen::Go => {}
                        }
                        o.y_fraction = o.y_fraction.wrapping_sub(1);
                    }
                    o.home_x(pod.0, 0x40, max_speed(0x83));
                    o.frame = 0x96 + ((f / 8) % 2) as u8;
                    return;
                }
                m if m & 0x80 != 0 => {
                    o.x_fraction = o.x_fraction.wrapping_sub(1);
                    if o.x_fraction == 0 {
                        match choose(o) {
                            Chosen::Pause => continue,
                            Chosen::Go => {}
                        }
                        o.x_fraction = o.x_fraction.wrapping_sub(1);
                    }
                    o.home_y(pod.1, 0x40, max_speed(0x83));
                    o.flags_y = if o.down() { 0x81 } else { 0 };
                    o.frame = 0x98 + ((f / 8) % 2) as u8;
                    return;
                }
                _ => {
                    let was = o.work6;
                    o.work6 -= 1;
                    if was != 2 {
                        return;
                    }
                    o.work6 = 0;
                }
            }
        }
    }

    // ---- Blue sphere (behaviour.md 4.15) ---------------------------------------------------------

    pub(super) fn step_sphere(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let o = &mut self.slots[slot];
        if o.work6 < 63 {
            o.work6 += 1;
            return;
        }
        if o.frame < 0xA1 {
            if env.f().is_multiple_of(8) {
                o.frame += 1;
            }
            return;
        }
        o.home_x(pod.0, 8, max_speed(4));
        o.home_y(pod.1, 0x18, max_speed(4));
        o.work5 = o.work5.wrapping_sub(1);
        let o = *o;
        let n = near(&o, &*env.pod);
        if o.work5.is_multiple_of(32) && n.dy < 24 && o.left() != n.pod_right {
            let dx: i16 = if o.left() { -24 } else { 24 };
            let shot = Shot {
                state: state::ENEMY_SHOT,
                frame: 0xA2,
                x: o.x.wrapping_add_signed(dx),
                y: o.y.wrapping_sub(2),
                speed: (0x0400, o.speed_y),
                flags: (o.flags_x, o.flags_y),
                life: 0xC0,
            };
            if let Some(free) = self.first_free(CHILD_SLOTS) {
                self.make_shot(env, free, shot);
                self.enemy_fires();
            }
        }
    }

    // ---- Satellite and companion (behaviour.md 4.18) -----------------------------------------------

    pub(super) fn step_satellite(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let o = &mut self.slots[slot];
        o.home_x(pod.0, 3, max_speed(0xC0));
        o.home_y(pod.1, 0x10, max_speed(0xC2));
        let o = *o;
        if env.rng.out & 0x3E == 0x16 {
            let left = o.x >= pod.0;
            let dx: i16 = if left { -16 } else { 16 };
            let shot = Shot {
                state: state::ENEMY_SHOT,
                frame: 0xC0,
                x: o.x.wrapping_add_signed(dx),
                y: o.y,
                speed: (0x0600, 0),
                flags: (u8::from(left), 0),
                life: 0xC0,
            };
            self.shoot(env, CHILD_SLOTS, false, shot);
        }
    }

    pub(super) fn step_companion(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let k = f / 2;
        let o = self.slots[slot];
        match o.work6 {
            0..=3 => {
                if o.work5 == 0 {
                    if !(k + slot as u32).is_multiple_of(8) {
                        return;
                    }
                    let found = PLACED_SLOTS
                        .into_iter()
                        .find(|&s| self.slots[s].state == state::SATELLITE);
                    match found {
                        Some(satellite) => {
                            self.slots[slot].work5 = satellite as u8;
                            self.orbit(slot, satellite, f);
                        }
                        None => self.slots[slot].work6 += 1,
                    }
                } else {
                    let satellite = usize::from(o.work5);
                    match self.slots[satellite].state {
                        0 => self.slots[slot].state = 0,
                        state::SATELLITE => self.orbit(slot, satellite, f),
                        _ => {
                            let m = &mut self.slots[slot];
                            m.work6 += 1;
                            m.work5 = 0;
                        }
                    }
                }
            }
            4 => {
                let pod = (env.pod.body.x, env.pod.body.y);
                let m = self.slots[slot];
                // Mesen: the check comes before the move, and the helper starts where the
                // companion stood.
                if near(&m, &*env.pod).d >= 48 || self.first_free(SHOT_SLOTS).is_none() {
                    home_on(&mut self.slots[slot], pod, 0x16, max_speed(2));
                }
                if near(&m, &*env.pod).d < 48
                    && let Some(shot_slot) = self.first_free(SHOT_SLOTS)
                {
                    // A fresh object at the companion's place (Mesen: speeds and fractions are
                    // cleared).
                    let helper = self.create(env.data, shot_slot, m.state, 0x51, m.x, m.y);
                    helper.work6 = 5;
                    helper.hit_points = 128;
                    helper.index = m.index;
                    helper.kind = m.kind;
                    self.slots[slot].state = 0;
                    if let Some(entry) = self.present.get_mut(usize::from(m.index)) {
                        *entry = 0;
                    }
                }
            }
            5 => self.step_helper(slot, env),
            _ => {}
        }
    }

    /// Phase 5, the companion in the pod's service (behaviour.md 4.18): fires at the placed
    /// objects, then orbits the pod.
    fn step_helper(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let me = self.slots[slot];
        // Fire: a free slot among 3-6, and a target in the slot 23 + (R1 and 7), else
        // 23 + (R and 7).
        let free = (3..=6).find(|&s| self.slots[s].state == 0);
        if let Some(free) = free {
            let first = 23 + usize::from(env.rng.high & 7);
            let second = 23 + usize::from(env.rng.out & 7);
            let target = [first, second]
                .into_iter()
                .find(|&t| self.slots[t].state != 0)
                .map(|t| self.slots[t]);
            if let Some(target) = target {
                let mut dx = u32::from(me.x.abs_diff(target.x));
                let mut dy = u32::from(me.y.abs_diff(target.y));
                for _ in 0..5 {
                    if dx >> 8 < 4 && dy >> 8 < 4 {
                        dx *= 2;
                        dy *= 2;
                    } else {
                        break;
                    }
                }
                let shot = Shot {
                    state: state::ENEMY_SHOT,
                    frame: 0x62,
                    x: me.x,
                    y: me.y,
                    speed: (dx as u16, dy as u16),
                    flags: (u8::from(me.x >= target.x), u8::from(me.y < target.y)),
                    life: 0xC0,
                };
                let made = self.make_shot(env, free, shot);
                made.hit_points = 1;
                self.enemy_fires();
            }
        }
        self.orbit_round(slot, (env.pod.body.x, env.pod.body.y), f);
    }

    /// Homes the companion on its place on the circle round the satellite.
    fn orbit(&mut self, slot: usize, satellite: usize, f: u32) {
        let sat = self.slots[satellite];
        self.orbit_round(slot, (sat.x, sat.y), f);
    }

    fn orbit_round(&mut self, slot: usize, centre: (u16, u16), f: u32) {
        let s = f as u8 as i8;
        let radius = |v: u8| -> i32 {
            let sine = if v >= 128 {
                0
            } else {
                i32::from(SIN[usize::from(v)])
            };
            (0x48 * sine) >> 8
        };
        let ox = radius(s.unsigned_abs()) * if s < 0 { -1 } else { 1 };
        let b = s.unsigned_abs().wrapping_add(0x40) as i8;
        let oy = radius(b.unsigned_abs()) * if b < 0 { -1 } else { 1 };
        let target = (
            centre.0.wrapping_add_signed(ox as i16),
            centre.1.wrapping_add_signed(oy as i16),
        );
        home_on(&mut self.slots[slot], target, 0xF0, max_speed(0x83));
    }
}
