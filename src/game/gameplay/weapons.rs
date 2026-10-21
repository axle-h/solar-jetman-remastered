//! The pod's special weapons (`docs/spec/player.md` 10.1, 10.2, `docs/spec/behaviour.md` 5.11):
//! homing missiles, the multi warhead, star bullets with their children, the time bomb and the
//! smart bomb. Their shots live in slots 3-10 like the plain bullet's.

use super::minions::missile_direction;
use super::objdata::ObjectData;
use super::objects::{
    Env, INVISIBLE, ObjectTable, PLACED_SLOTS, SHOT_SLOTS, SLOTS, Score, relative, state,
};
use super::pod::Pod;

/// The fitted weapon numbers RAM $D1 (economy.md 6) and what a shot costs the gauge.
pub const HOMING: u8 = 1;
pub const SMART_BOMB: u8 = 4;
pub const TIME_BOMB: u8 = 5;
pub const MULTI_WARHEAD: u8 = 6;
pub const STAR_BULLETS: u8 = 8;

/// The gauge units a shot of `weapon` costs (player.md 10.1), or `None` for a device.
pub fn cost(weapon: u8) -> Option<u8> {
    match weapon {
        HOMING => Some(4),
        TIME_BOMB => Some(32),
        MULTI_WARHEAD => Some(16),
        STAR_BULLETS => Some(32),
        SMART_BOMB => Some(128),
        _ => None,
    }
}

/// The frames of a star child by its stage (behaviour.md 5.11.2).
const CHILD_FRAMES: [u8; 6] = [0x27, 0x28, 0x29, 0x2F, 0x30, 0x31];

impl ObjectTable {
    /// Makes the shot of `weapon` (not the smart bomb) at the gun's nose, if the slot it would
    /// take is 3-6 (player.md 10.1). Returns whether a shot was made; with a higher slot the
    /// slot is cleared, nothing is made and nothing is charged.
    pub fn fire_weapon(
        &mut self,
        data: &ObjectData,
        pod: &Pod,
        weapon: u8,
        strength: u8,
        stale: bool,
    ) -> bool {
        let Some(nose) = self.nose_shot(pod) else {
            return false;
        };
        if nose.slot > 6 {
            // The plain bullet is set up in the slot and the slot is then cleared (a spark there
            // is taken over): the press is lost, the bullet's bytes stay behind.
            let shot = self.make_pod_shot(data, pod, nose, state::BULLET, 0x37, strength, stale);
            shot.index = 32;
            shot.state = state::FREE;
            return false;
        }
        let mut nose = nose;
        let (state, frame, hit_points, work5) = match weapon {
            HOMING => (state::MISSILE, 0x27, 2, 0xC0),
            TIME_BOMB => {
                nose.speed = (0, 0);
                (state::TIME_BOMB, 0x3D, 0x40, 0xC0)
            }
            MULTI_WARHEAD => (state::MULTI_WARHEAD, 0x27, 0xFF, 0xC0),
            STAR_BULLETS => {
                nose.speed = (nose.speed.0 >> 4, nose.speed.1 >> 4);
                (state::STAR_PARENT, 0x27, 0x50, 0x18)
            }
            _ => return false,
        };
        let shot = self.make_pod_shot(data, pod, nose, state, frame, hit_points, stale);
        // Field $0393 stays at the plain bullet's 32: "no target" slot for a missile.
        shot.index = 32;
        shot.work5 = work5;
        true
    }

    /// A missile of the pod (state $0F homing, $14 multi warhead; behaviour.md 5.11.1).
    pub(super) fn step_pod_missile(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let k = f / 2;
        let mut target = usize::from(self.slots[slot].index);
        // 1. The target.
        let alive = |table: &ObjectTable, t: usize| t < SLOTS && table.slots[t].state >= 6;
        let sum = 2 * slot as u32 + (f & 0xFF);
        let look = if target == 0 {
            sum & 0x0E == 0
        } else {
            !alive(self, target)
        };
        if look {
            let carry = if target == 0 { (sum >> 8) as u8 & 1 } else { 0 };
            env.rng.out = env.rng.out.wrapping_add(0x57).wrapping_add(carry);
            let candidate = 23 + usize::from(env.rng.out & 7);
            let c = &self.slots[candidate];
            target = if c.frame != INVISIBLE && c.state >= 6 {
                candidate
            } else {
                0
            };
        }
        self.slots[slot].index = target as u8;
        // 2. The direction, from the missile to the target; with none, the pod (Q27) in the
        // original, in play the missile keeps its direction.
        if (slot as u32 + k) & 7 == 0 {
            let aim = if target != 0 {
                Some((self.slots[target].x, self.slots[target].y))
            } else if env.original_rules {
                Some((env.pod.body.x, env.pod.body.y))
            } else {
                None
            };
            if let Some(aim) = aim {
                let o = &mut self.slots[slot];
                o.kind = missile_direction((o.x, o.y), aim, env.original_rules);
                o.flags_x |= 2;
            }
        }
        // 3. The frame, 4. the speeds and the move.
        let o = &mut self.slots[slot];
        let fast = (o.speed_x >> 8) + (o.speed_y >> 8) >= 3;
        o.frame = if fast { 0x27 } else { 0x2F } + o.kind;
        self.missile_motion(slot);
        // 5. The life.
        let multi = self.slots[slot].state == state::MULTI_WARHEAD;
        let tick = if multi { f & 0x0E == 0 } else { f & 2 == 0 };
        if tick {
            let o = &mut self.slots[slot];
            o.work5 = o.work5.wrapping_sub(1);
            if o.work5 == 0 {
                self.destroy(env.data, slot, env.score);
            }
        }
    }

    /// A star bullet's parent (state $12; behaviour.md 5.11.2): drifts on, and makes one child
    /// a step at a random placed object.
    pub(super) fn step_star_parent(&mut self, slot: usize, env: &mut Env) {
        let o = &mut self.slots[slot];
        o.frame = 0x27 + ((env.f() / 8) & 1) as u8;
        o.advance();
        let Some(child) = self.first_free(3..=6) else {
            return;
        };
        let (a, b) = (
            23 + usize::from(env.rng.high & 7),
            23 + usize::from(env.rng.out & 7),
        );
        let target = if self.slots[a].state != 0 {
            a
        } else if self.slots[b].state != 0 {
            b
        } else {
            return;
        };
        let parent = self.slots[slot];
        let aim = self.slots[target];
        let (mut dx, mut dy) = (parent.x.abs_diff(aim.x), parent.y.abs_diff(aim.y));
        for _ in 0..5 {
            if dx >> 8 >= 4 || dy >> 8 >= 4 {
                break;
            }
            dx <<= 1;
            dy <<= 1;
        }
        let old = self.slots[child];
        let c = self.create(env.data, child, state::STAR_CHILD, 0x27, parent.x, parent.y);
        c.hit_points = 0x10;
        c.speed_x = dx;
        c.speed_y = dy;
        c.flags_x = u8::from(aim.x < parent.x);
        c.flags_y = u8::from(aim.y >= parent.y);
        if env.original_rules {
            // Quirk Q17: the child keeps the slot's old sub-pixel position.
            c.x_fraction = old.x_fraction;
            c.y_fraction = old.y_fraction;
            c.index = old.index;
        }
        let o = &mut self.slots[slot];
        o.work5 -= 1;
        if o.work5 == 0 {
            o.state = state::FREE;
        }
    }

    /// A star bullet's child (state $13): flies at its target and ages through six frames.
    pub(super) fn step_star_child(&mut self, slot: usize, env: &Env) {
        let o = &mut self.slots[slot];
        o.advance();
        // Quirk Q28: a slot of even number never satisfies the original's test; in play every
        // child ages, as one in an odd slot does.
        let number = if env.original_rules { slot } else { slot | 1 };
        if (env.f() + number as u32 + 1) & 15 == 0 {
            o.kind += 1;
            if usize::from(o.kind) >= CHILD_FRAMES.len() {
                o.state = state::FREE;
            } else {
                o.frame = CHILD_FRAMES[usize::from(o.kind)];
            }
        }
    }

    /// A time bomb (state $10; behaviour.md 5.11.3): it stays where it is, counting frames, and
    /// goes off after the fifth.
    pub(super) fn step_time_bomb(&mut self, slot: usize, env: &mut Env) {
        if (slot as u32 + env.f() / 2) & 31 != 0 {
            return;
        }
        let o = &mut self.slots[slot];
        o.frame += 1;
        if o.frame == 0x42 {
            o.frame = 0x41;
            let bomb = *o;
            self.destroy(env.data, slot, env.score);
            for target in PLACED_SLOTS {
                let t = self.slots[target];
                if t.state == 0 || t.frame == INVISIBLE {
                    continue;
                }
                let reach = -64..=63;
                if reach.contains(&relative(t.x, bomb.x)) && reach.contains(&relative(t.y, bomb.y))
                {
                    self.destroy_with(env.data, target, env.score, env.original_rules);
                }
            }
        }
    }

    /// The smart bomb (player.md 10.2): everything in slots 15-30 that is in use and visible is
    /// destroyed, in slot order. In the original each destruction frees the explosions made
    /// before it (Q29); in play every victim explodes.
    pub fn smart_bomb(&mut self, data: &ObjectData, score: &mut Score, original_rules: bool) {
        for slot in 15..SLOTS {
            let o = self.slots[slot];
            if o.state != 0 && o.frame != INVISIBLE {
                self.destroy_with(data, slot, score, original_rules);
            }
        }
    }
}

/// Whether `slot` is one of the pod's shot slots.
pub(super) fn is_shot_slot(slot: usize) -> bool {
    SHOT_SLOTS.contains(&slot)
}
