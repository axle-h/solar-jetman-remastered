//! The level-change gate: touching a wormhole, the bounce of a closed one and the gate state
//! (player state 9) that draws the player through an open one (`docs/spec/player.md` 9.2).

use super::Scaled;
use super::objects::{SLOTS, relative, state};
use super::pod::{STATE_GATE, STATE_POD};
use super::tables::SIN;
use crate::game::level::{Level, World};
use crate::game::sound::{Channel, Request};

/// Counts of the gate state: it ends when the burst counter reaches this (player.md 9.2.3).
const END_COUNT: u8 = 96;
/// The particles' picture: the star of object frame $BC.
const PARTICLE_FRAME: u8 = 0xBC;
/// First slot of the two particle groups (A: 15-22, B: 23-30).
const GROUP_FIRST: [usize; 2] = [15, 23];
/// The phase each group adds to the angle of its particles.
const GROUP_PHASE: [u8; 2] = [15, 0];

/// One ring of eight particles (player.md 9.2.3 step 5).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Group {
    /// Radius in whole pixels.
    r: u16,
    /// Radius speed, angular speed and angle, all 8.8.
    v: i32,
    w: u16,
    a: u16,
}

/// The gate state's own memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Gate {
    /// The wormhole's slot and position.
    slot: usize,
    at: (u16, u16),
    /// The player state to go back to, RAM $69.
    previous: u8,
    /// The step of the state, 1 on its first.
    n: u32,
    /// The burst counter, RAM $68.
    k: u8,
    groups: [Option<Group>; 2],
    /// Times the background palettes 2 and 3 have been darkened.
    pub fades: u8,
    /// Those fades in the palette on screen: the shadow is uploaded only by video job 0
    /// (`graphics.md` 7.1).
    #[serde(default)]
    pub fades_shown: u8,
    /// The pod's sprite palette bits, RAM $98 bits 0-1.
    pub flash: u8,
}

impl Gate {
    /// The player state the gate was entered in, RAM $69.
    pub fn previous(&self) -> u8 {
        self.previous
    }
}

/// The bit of a planet's warpship flag in `planet_flags` for a level of the planet or its second
/// level (game-flow.md 6): planet 12 uses planet 10's.
pub fn planet_flag_bit(level: usize) -> usize {
    let planet = match level {
        0..=12 => level,
        13..=25 => level - 13,
        29.. => level - 29,
        _ => 0,
    };
    if planet == 12 { 10 } else { planet }
}

/// (dx, dy) of a point at `angle` and radius `r` (scenes.md 5.4a).
fn polar(angle: u8, r: i32) -> (i32, i32) {
    let fold = |a: u8| {
        if a < 128 {
            usize::from(a)
        } else {
            256 - usize::from(a)
        }
    };
    let sine = |index: usize| i32::from(SIN.get(index).copied().unwrap_or(0));
    let dx = sine(fold(angle)) * r / 256;
    let j = angle.wrapping_add(64);
    let dy = sine(fold(j)) * r / 256;
    (
        if angle >= 128 { -dx } else { dx },
        if j >= 128 { -dy } else { dy },
    )
}

impl Scaled {
    /// Whether the wormhole at `at` is touched and what it does (player.md 9.2.1-9.2.2): an
    /// open gate starts the gate state unless the transition guard refuses, a closed one
    /// bounces the player.
    pub(super) fn touch_wormhole(&mut self, world: &World, level: &Level, slot: usize) {
        let item = self.objects.slots[slot];
        let open =
            usize::from(level.number) >= crate::game::level::PLANETS || self.fuel_count() == 25;
        if !open {
            self.bounce(world, item);
            return;
        }
        // A refused touch is ignored completely.
        if self.last_transition == 3 && self.guard != 0 {
            return;
        }
        let pod = &mut self.pod;
        self.gate = Some(Gate {
            slot,
            at: (item.x, item.y),
            previous: pod.state,
            n: 0,
            k: 0,
            groups: [None; 2],
            fades: 0,
            fades_shown: 0,
            flash: 0,
        });
        self.tow.angle = u32::from(pod.state) << 24;
        self.tow.mass = 0;
        pod.timer = slot as u8;
        pod.state = STATE_GATE;
        self.stars = super::stars::Stars::new(self.stars.mode);
        self.sfx.push(Request::Song {
            id: 2,
            always: false,
        });
    }

    /// A pick-up sparkle in `slot` at `at`: frame $69 first, life 8 (objects.md 7).
    fn make_sparkle(&mut self, world: &World, slot: usize, at: (u16, u16)) {
        let sparkle = self
            .objects
            .create(&world.objects, slot, state::SPARKLE, 0x69, at.0, at.1);
        sparkle.index = 8;
    }

    /// A closed wormhole throws the player back (player.md 9.2.2): a direction bit flips, a
    /// sparkle appears, sound $44; no damage.
    fn bounce(&mut self, world: &World, item: super::objects::Object) {
        let dx = i32::from(relative(self.pod.body.x, item.x));
        let dy = i32::from(relative(self.pod.body.y, item.y));
        if dy.abs() >= dx.abs() {
            self.pod.body.h_left = !self.pod.body.h_left;
        } else {
            self.pod.body.v_down = !self.pod.body.v_down;
        }
        // Slots 4-10 holding a sparkle already: none is made; the sparkle takes a free slot.
        let busy = (4..=10).any(|s| self.objects.slots[s].state == state::SPARKLE);
        let slot = self.objects.first_free(super::objects::SHOT_SLOTS);
        if let (false, Some(slot)) = (busy, slot) {
            self.make_sparkle(world, slot, (item.x, item.y));
        }
        self.sfx.push(Request::a(Channel::Pulse1, 0x44));
        self.sfx.push(Request::b(Channel::Pulse2, 0x44));
        self.pod.hit_pause = 1;
    }

    /// The level a change of level (code 3) leads to (game-flow.md 7.4): the second level of a
    /// planet, from there the warp level if the planet's flag is set and else the planet,
    /// and from a warp level the planet.
    pub fn level_after_change(&self) -> usize {
        match self.level {
            0..=12 => self.level + 13,
            13..=25 => {
                let flagged = self.planet_flags & (1 << planet_flag_bit(self.level)) != 0;
                if flagged {
                    self.level + 16
                } else {
                    self.level - 13
                }
            }
            26..=28 => self.level + 16,
            _ => self.level - 29,
        }
    }

    /// The game on the level after a change of level (code 3), the player placed as
    /// game-flow.md 7.4 says: at the new level's home point, 128 px above the mothership on a
    /// planet, speeds cleared, fuel, state and direction bits as they were. RAM $97 is 64 after
    /// a gate that ended directly in the change (a second level from a planet, a planet from a
    /// second level) and 0 after the kind-6 screen or the badge screen (a warp level, the planet
    /// after it).
    pub fn change_level(&self, world: &World) -> Scaled {
        let target = self.level_after_change();
        let first_warp = crate::game::level::FIRST_WARP_LEVEL;
        let invulnerable = if target >= first_warp || self.level >= first_warp {
            0
        } else {
            64
        };
        let target = self.level_after_change();
        let to_planet = target < crate::game::level::PLANETS;
        let mut seed = self.seed();
        seed.counter = self.counter & !0xFFFF;
        seed.stale = self.objects.slots.to_vec();
        seed.pod_leftovers = self.pod_leftovers();
        seed.saved_fuel_count = if self.level < crate::game::level::PLANETS {
            self.fuel_count()
        } else {
            self.saved_fuel_count
        };
        seed.arrival = Some(super::Arrival {
            state: self.pod.state,
            fuel: self.pod.fuel,
            fuel_shown: self.pod.fuel_shown,
            h_left: self.pod.body.h_left,
            v_down: self.pod.body.v_down,
            move_angle: self.pod.move_angle,
            timer: self.pod.timer,
            counter: self.pod.counter,
            facing: self.pod.facing,
            rope: {
                let (angle, speed) = self.tow.rope();
                (angle, speed, self.tow.sense)
            },
            invulnerable,
            lowered: to_planet && self.level >= crate::game::level::PLANETS,
            fuel_count: to_planet.then_some(seed.saved_fuel_count),
        });
        Scaled::new(world, target, seed)
    }

    /// The darkening of the background palettes 2 and 3 so far, in fades (player.md 9.2.3
    /// step 6).
    pub fn gate_fades(&self) -> u8 {
        self.gate.as_ref().map_or(0, |gate| gate.fades_shown)
    }

    /// The pod's sprite palette during the gate state: both bits toggle every step.
    pub fn gate_flash(&self) -> Option<u8> {
        self.gate.as_ref().map(|gate| gate.flash)
    }

    /// One step of the gate state (player.md 9.2.3), in the player's place in the A pass.
    pub(super) fn gate_step(&mut self, level: &Level) {
        let Some(mut gate) = self.gate.take() else {
            return;
        };
        gate.n += 1;
        if gate.n == 1 {
            // The first call forces the low nibble of the frame counter (a hardware cadence
            // the original keeps; the first two calls fall on consecutive frames).
            self.counter = (self.counter & !0xF) | 0xD;
            for slot in 1..SLOTS {
                if slot != gate.slot {
                    self.objects.slots[slot].state = 0;
                }
            }
            self.pod.flame = None;
            self.tow.mass = 0x80;
        }
        self.home_on_gate(level, gate.at);
        let counter = self.counter as u8;
        if gate.previous == STATE_POD {
            self.pod.facing = 2 * (counter & 3);
        }
        gate.flash ^= 3;
        // Both palette bits flip every step (game-flow.md 9.2, "Bits 0-1", 2).
        self.pod.palette_bits ^= 3;
        // The counter and the bursts.
        if counter & 3 == 0 {
            gate.k += 1;
            if gate.k < END_COUNT {
                let first = if gate.k % 2 == 0 { 0 } else { 1 };
                let target = (first..2).find(|&g| {
                    gate.groups[g].is_none() && self.objects.slots[GROUP_FIRST[g]].state == 0
                });
                if let Some(g) = target {
                    let v = i32::from(gate.k / 4 + 4) << 8;
                    gate.groups[g] = Some(Group {
                        r: 1,
                        v,
                        w: 0x0050,
                        a: 0,
                    });
                    for s in GROUP_FIRST[g]..GROUP_FIRST[g] + 8 {
                        self.objects.slots[s].state = state::MOTHERSHIP;
                        self.objects.slots[s].frame = PARTICLE_FRAME;
                    }
                }
            }
        }
        self.tow.angle = (self.tow.angle & 0xFF00_0000) | u32::from(gate.k) << 16;
        // The particle groups, A first.
        let mut died = false;
        for g in 0..2 {
            let Some(mut group) = gate.groups[g] else {
                continue;
            };
            if died {
                continue;
            }
            group.r += (group.v >> 8) as u16;
            let mut dead = group.r > 255;
            if !dead {
                group.v -= 0x12;
                dead = group.v < 0;
            }
            if dead {
                gate.groups[g] = None;
                for s in GROUP_FIRST[g]..GROUP_FIRST[g] + 8 {
                    self.objects.slots[s].state = 0;
                }
                died = true;
                continue;
            }
            group.a = group.a.wrapping_add(group.w);
            group.w = group.w.wrapping_add(0x60);
            for i in 0..8u8 {
                let angle = ((group.a >> 8) as u8)
                    .wrapping_add(32u8.wrapping_mul(i))
                    .wrapping_add(GROUP_PHASE[g]);
                let (dx, dy) = polar(angle, i32::from(group.r));
                let o = &mut self.objects.slots[GROUP_FIRST[g] + usize::from(i)];
                o.x = gate.at.0.wrapping_add_signed(dx as i16);
                o.y = gate.at.1.wrapping_add_signed(dy as i16);
            }
            gate.groups[g] = Some(group);
        }
        // The palette fades.
        if counter & 0x1F == 0 {
            gate.fades = gate.fades.saturating_add(1);
        }
        if gate.k >= END_COUNT {
            self.end_gate(&gate);
            return;
        }
        self.gate = Some(gate);
    }

    /// The player homes on the gate with an acceleration of 0.50, no brake (player.md 9.2.3
    /// step 2).
    fn home_on_gate(&mut self, level: &Level, at: (u16, u16)) {
        let body = &mut self.pod.body;
        let right = (at.0.wrapping_sub(body.x) as i16) >= 0;
        let toward_x = if right { !body.h_left } else { body.h_left };
        let down = (at.1.wrapping_sub(body.y) as i16) >= 0;
        let toward_y = if down { body.v_down } else { !body.v_down };
        if toward_x {
            body.set_h_speed((body.h_speed() + 0x80).min(0x0380));
        } else {
            body.set_h_speed(body.h_speed().saturating_sub(0x80));
            if body.h_speed() == 0 {
                body.h_left = !body.h_left;
            }
        }
        if toward_y {
            body.set_v_speed((body.v_speed() + 0x80).min(0x0300));
        } else {
            body.set_v_speed(body.v_speed().saturating_sub(0x80));
            if body.v_speed() == 0 {
                body.v_down = !body.v_down;
            }
        }
        let bottom_page = level.height.saturating_sub(1) >> 1;
        body.advance(true, bottom_page);
    }

    /// The end of the gate state (player.md 9.2.3 step 7): the previous state is back, the
    /// wormhole is gone, and the level changes.
    fn end_gate(&mut self, gate: &Gate) {
        self.pod.state = gate.previous;
        if gate.previous == STATE_POD {
            // The pod's picture turned with the counter; it goes back to facing up.
            self.pod.facing = 0;
        }
        self.pod.invulnerable = 64;
        let index = usize::from(self.objects.slots[gate.slot].index);
        if let Some(entry) = self.objects.items.get_mut(index) {
            entry.kind = 0;
        }
        self.objects.slots[gate.slot].state = 0;
        self.tow.mass = 0;
        self.tow.angle = 0;
        self.pod.body.set_h_speed(self.pod.body.h_speed() & 0x00FF);
        self.pod.body.set_v_speed(self.pod.body.v_speed() & 0x00FF);
        // Without the guard (game-flow.md 7.4).
        self.last_transition = 3;
        self.guard = 0xFF;
        self.transition = Some(super::economy::Transition::ChangeLevel);
    }
}
