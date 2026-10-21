//! The object table (`docs/spec/objects.md` 2-3): 31 slots, the placement scan and despawning,
//! destruction and explosions, the items list, and the level memory (3.6).
//!
//! Behaviours are in [`super::behave`], collisions in [`super::collide`].

use super::objdata::ObjectData;
use super::pod::Pod;
use super::rng::Rng;
use crate::game::level::{Level, Placement, Point};
use crate::game::sound::{Channel, Request};

pub(super) use crate::game::gameplay::objects::NOSE;
pub use crate::game::gameplay::objects::{
    CHILD_SLOTS, INVISIBLE, ITEM_SLOTS, ItemEntry, NoseShot, PLACED_SLOTS, SHOT_SLOTS, SLOTS,
    item_start, penetration, relative, state,
};
use crate::game::gameplay::objects::{EXPLOSION_SEQUENCE, KILL_POINTS, add_velocity};
pub use crate::game::gameplay::objects::{Object, Score};

/// What the object rules read of the surroundings.
pub struct Env<'a> {
    pub data: &'a ObjectData,
    pub level: &'a Level,
    pub camera: Point,
    pub pod: &'a mut Pod,
    pub rng: &'a mut Rng,
    /// The frame counter RAM $5D during the pass (behaviour.md 1, "F").
    pub counter: u32,
    pub score: &'a mut Score,
    /// The pickup tag RAM $D2 (economy.md 2.5), which the markers read.
    pub bonus_tag: u8,
    /// The tractor beam's swing count RAM $6E while it holds something (behaviour.md 5.4).
    pub beam_spin: Option<u8>,
    /// The slot the tractor beam holds (RAM $66), 0 for none.
    pub beam_slot: usize,
}

impl Env<'_> {
    /// F, the frame counter during the A pass, for the timing conditions of behaviour.md 1.
    pub fn f(&self) -> u32 {
        self.counter
    }
}

/// The object table and what goes with it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectTable {
    pub slots: [Object; SLOTS],
    /// The working copy of the level's type list, RAM $0460 on: the object type, or 0 once the
    /// object is destroyed or collected (objects.md 2.1).
    pub present: Vec<u8>,
    /// The placement cursor RAM $BD.
    pub cursor: usize,
    /// The items list copy (RAM $0513 on) and its cursor RAM $BB.
    pub items: Vec<ItemEntry>,
    pub item_cursor: usize,
    /// Respawn particles that reached the pod during the last sweep (player.md 7.3).
    pub arrivals: u8,
    /// The sound requests the objects made since the pass took them (sound.md 8).
    pub sfx: Vec<Request>,
    /// The warp level's countdown ran out in the last sweep (behaviour.md 6.3).
    pub countdown_done: bool,
    /// A shot hit an open core of the final level, RAM $EC (behaviour.md 4.16).
    pub core_hit: bool,
}

impl ObjectTable {
    /// An empty table for `level`, with its type list and items list copied (objects.md 3.6,
    /// levels.md 3.3).
    pub fn new(data: &ObjectData, level: &Level) -> ObjectTable {
        let present = data.placements[usize::from(level.number)]
            .iter()
            .map(|p| p.kind)
            .collect();
        let items = level
            .objects
            .iter()
            .map(|p| ItemEntry {
                kind: p.kind,
                x: p.x,
                y: p.y,
            })
            .collect();
        ObjectTable {
            slots: [Object::default(); SLOTS],
            present,
            cursor: 0,
            items,
            item_cursor: 0,
            arrivals: 0,
            sfx: Vec::new(),
            countdown_done: false,
            core_hit: false,
        }
    }

    pub fn first_free(&self, range: std::ops::RangeInclusive<usize>) -> Option<usize> {
        range.into_iter().find(|&s| self.slots[s].state == 0)
    }

    /// Makes an object in `slot`: every field cleared, hit points from the state table
    /// (behaviour.md 2.5).
    pub fn create(
        &mut self,
        data: &ObjectData,
        slot: usize,
        state: u8,
        frame: u8,
        x: u16,
        y: u16,
    ) -> &mut Object {
        self.slots[slot] = Object {
            state,
            frame,
            x,
            y,
            hit_points: data.initial_hit_points(state),
            ..Object::default()
        };
        &mut self.slots[slot]
    }

    // ---- Placement (objects.md 3.4) ----------------------------------------------------------

    fn placements<'a>(data: &'a ObjectData, level: &Level) -> &'a [Placement] {
        &data.placements[usize::from(level.number)]
    }

    /// Whether list entry `index` is in range of the camera (3.4).
    fn in_range(place: &Placement, camera: Point, margin: i32) -> bool {
        let rx = i32::from(relative(place.x, camera.x));
        let ry = i32::from(relative(place.y, camera.y));
        (-margin..256 + margin).contains(&rx) && (-margin..256 + margin).contains(&ry)
    }

    /// Spawns list entry `index` in the lowest free slot of 23-30, unless it is live already
    /// or all are busy. Returns whether it did.
    fn spawn(&mut self, data: &ObjectData, level: &Level, index: usize) -> bool {
        if self
            .slots
            .iter()
            .any(|o| o.is_placed() && usize::from(o.index) == index)
        {
            return false;
        }
        let Some(slot) = self.first_free(PLACED_SLOTS) else {
            return false;
        };
        let place = Self::placements(data, level)[index];
        let (frame, state) = data.type_start(place.kind);
        let object = self.create(data, slot, state, frame, place.x, place.y);
        object.index = index as u8;
        object.kind = place.kind;
        if matches!(place.kind, 0x12 | 0x17 | 0x18 | 0x47 | 0x5E) {
            // The home position, kept in the speed bytes (3.4).
            object.speed_x = place.x;
            object.speed_y = place.y;
        }
        true
    }

    /// The scan at the start of a level: every object in range and present is spawned, with
    /// the visibility test off (3.4).
    pub fn start_scan(&mut self, data: &ObjectData, level: &Level, camera: Point) {
        let margin = i32::from(level.object_margin);
        for index in 0..self.present.len() {
            let place = Self::placements(data, level)[index];
            if self.present[index] != 0 && Self::in_range(&place, camera, margin) {
                self.spawn(data, level, index);
            }
        }
    }

    /// The scan of one logic step's B pass: at most 8 entries, at most 1 spawn (3.4). Not run
    /// on warp levels. Returns the thrust's carry-in byte RAM $0D as the scan leaves it: cleared
    /// at the start, then the `dy` byte of the last entry it examines when that is a group member;
    /// `None` when the scan does not run.
    pub fn step_scan(&mut self, data: &ObjectData, level: &Level, camera: Point) -> Option<u8> {
        if level.is_warp_level() || self.present.is_empty() {
            return None;
        }
        let margin = i32::from(level.object_margin);
        let mut carry = 0;
        for _ in 0..8 {
            let index = self.cursor;
            // A group member loads its dy; any other entry leaves 0 (the byte is not kept from
            // an earlier entry of the pass).
            carry = data
                .group_dy
                .get(usize::from(level.number))
                .and_then(|list| list.get(index))
                .copied()
                .flatten()
                .unwrap_or(0);
            let place = Self::placements(data, level)[index];
            let rx = i32::from(relative(place.x, camera.x));
            let ry = i32::from(relative(place.y, camera.y));
            let on_screen = (0..256).contains(&rx) && (0..224).contains(&ry);
            // objects.md 3.4: a band of 16 px above the screen, for every column, waits.
            let above = (-16..0).contains(&ry);
            let wanted = self.present[index] != 0
                && Self::in_range(&place, camera, margin)
                && !on_screen
                && !above;
            self.cursor += 1;
            let spawned = wanted && self.spawn(data, level, index);
            if self.cursor >= self.present.len() {
                self.cursor = 0;
                return Some(carry);
            }
            if spawned {
                return Some(carry);
            }
        }
        Some(carry)
    }

    // ---- Despawning (objects.md 3.5) ---------------------------------------------------------

    /// Frees every object in slots 1-30 that left the activity window, using the camera as it
    /// is after the B pass's move.
    pub fn despawn(&mut self, level: &Level, camera: Point) {
        let margin = i32::from(level.object_margin);
        let wide = margin.max(32);
        for slot in 1..SLOTS {
            let object = &self.slots[slot];
            // The warp level's countdown digit stays (it is placed by the pod).
            if object.state == 0 || object.state == state::COUNTDOWN {
                continue;
            }
            let rx = i32::from(relative(object.x, camera.x));
            let ry = i32::from(relative(object.y, camera.y));
            let kept_x = (-wide..256 + wide).contains(&rx);
            let kept = if object.frame == INVISIBLE {
                // Invisible objects: y from -M to 256 + M (objects.md 3.5; a generator at ry
                // -224 was kept with M = 224, one at -227 freed).
                kept_x && (-margin..256 + margin).contains(&ry)
            } else if (-32..288).contains(&rx) {
                kept_x && (-wide..256 + margin).contains(&ry)
            } else {
                kept_x && (-margin..256 + margin).contains(&ry)
            };
            if !kept {
                self.slots[slot].state = 0;
            }
        }
    }

    /// Whether an object is drawn (3.5): frame not $FF and within 32 px of the screen.
    pub fn is_drawn(object: &Object, camera: Point) -> bool {
        let rx = i32::from(relative(object.x, camera.x));
        let ry = i32::from(relative(object.y, camera.y));
        object.state != 0
            && object.frame != INVISIBLE
            && (-32..288).contains(&rx)
            && (-32..256).contains(&ry)
    }

    // ---- Destruction and explosions (objects.md 2.5, behaviour.md 3.5) -------------------------

    /// Destroys the object in `slot`: it becomes an explosion in place, and a placed object is
    /// never spawned again in this visit. A kill of a placed object scores.
    pub fn destroy(&mut self, data: &ObjectData, slot: usize, score: &mut Score) {
        self.destroy_with(data, slot, score, true);
    }

    /// [`destroy`](Self::destroy), with the freeing of the explosions in progress in slots
    /// 23-30 (behaviour.md 3.5, step 1; quirk Q7) switched by `free_explosions`.
    pub fn destroy_with(
        &mut self,
        data: &ObjectData,
        slot: usize,
        score: &mut Score,
        free_explosions: bool,
    ) {
        if self.slots[slot].frame == INVISIBLE {
            return;
        }
        if free_explosions {
            for s in PLACED_SLOTS {
                if self.slots[s].state < 6 {
                    self.slots[s].state = 0;
                }
            }
        }
        let object = self.slots[slot];
        if object.is_placed()
            && let Some(entry) = self.present.get_mut(usize::from(object.index))
        {
            *entry = 0;
        }
        let (half_width, half_height) = data.half_size(object.frame);
        let size = u32::from(half_width) + u32::from(half_height);
        let (state, sequence) = if slot < *CHILD_SLOTS.start() || size >= 28 {
            (state::EXPLOSION_LARGE, 11)
        } else if size >= 8 {
            (state::EXPLOSION_LARGE, 5)
        } else {
            (state::EXPLOSION_SMALL, 0)
        };
        if PLACED_SLOTS.contains(&slot) {
            score.add(KILL_POINTS[usize::from(state - state::EXPLOSION_SMALL)]);
        }
        // Behaviour.md 3.5: a small explosion sounds $0A on the noise channel, the others $0C.
        let id = if state == state::EXPLOSION_SMALL {
            0x0A
        } else {
            0x0C
        };
        self.sfx.push(Request::a(Channel::Noise, id));
        let object = &mut self.slots[slot];
        object.state = state;
        object.index = 1;
        object.kind = sequence;
    }

    /// Turns a shot into a small explosion (behaviour.md 3.4), with sound $0A.
    pub fn explode_shot(&mut self, slot: usize) {
        self.sfx.push(Request::a(Channel::Noise, 0x0A));
        let object = &mut self.slots[slot];
        object.state = state::EXPLOSION_SMALL;
        object.index = 1;
        object.kind = 0;
    }

    /// One step of an explosion (objects.md 2.5): every 4 steps the next frame of its sequence;
    /// the end of the sequence frees the slot.
    pub fn step_explosion(&mut self, slot: usize) {
        let object = &mut self.slots[slot];
        object.index = object.index.wrapping_sub(1);
        if object.index == 0 {
            object.index = 4;
            let next = EXPLOSION_SEQUENCE
                .get(usize::from(object.kind))
                .copied()
                .unwrap_or(INVISIBLE);
            object.kind = object.kind.wrapping_add(1);
            if next == INVISIBLE {
                object.state = 0;
            } else {
                object.frame = next;
            }
        }
    }

    // ---- The pod's shots and sparks (player.md 5.5, 10) --------------------------------------

    /// Where and how the gun's next shot starts (player.md 10): the slot it takes (the first
    /// free one of 3-10, else the first spark's), the nose position and the shot's own speed
    /// and directions, before the shooter's velocity is added.
    pub fn nose_shot(&self, pod: &Pod) -> Option<NoseShot> {
        let slot = self.first_free(SHOT_SLOTS).or_else(|| {
            SHOT_SLOTS
                .into_iter()
                .find(|&s| self.slots[s].state == state::SPARK)
        })?;
        let f = usize::from(pod.facing & 31);
        let n = (f + 16) & 31;
        let signed = |index: usize| i16::from(NOSE[index] as i8);
        let magnitude = |index: usize| u16::from((NOSE[index] as i8).unsigned_abs()) * 128;
        let shot = if pod.state == super::pod::STATE_POD {
            NoseShot {
                slot,
                x: pod.body.x.wrapping_add_signed(signed(n + 8)),
                y: pod.body.y.wrapping_add_signed(signed(n)),
                speed: (magnitude(f + 8), magnitude(f)),
                left: match f {
                    17..=31 => true,
                    0 | 16 => pod.body.h_left,
                    _ => false,
                },
                down: match f {
                    9..=23 => true,
                    8 | 24 => pod.body.v_down,
                    _ => false,
                },
            }
        } else {
            // Jetman: (+-4, +1) on the side he faces, 6.00 px per step across (player.md 10).
            let left = pod.look.unwrap_or(pod.body.h_left);
            NoseShot {
                slot,
                x: pod.body.x.wrapping_add_signed(if left { -4 } else { 4 }),
                y: pod.body.y.wrapping_add(1),
                speed: (0x0600, 0),
                left,
                down: false,
            }
        };
        Some(shot)
    }

    /// The pod fires a shot of `strength` hit points (player.md 10): in the first free slot of
    /// 3-10, else in the first spark's. Returns whether a shot was made.
    pub fn fire_pod_shot(&mut self, data: &ObjectData, pod: &Pod, strength: u8) -> bool {
        let Some(nose) = self.nose_shot(pod) else {
            return false;
        };
        let shot = self.make_pod_shot(data, pod, nose, state::BULLET, 0x37, strength);
        shot.index = 32;
        true
    }

    /// Makes the shot `nose` describes in its slot, with the shooter's velocity added to its own
    /// (player.md 10).
    pub(super) fn make_pod_shot(
        &mut self,
        data: &ObjectData,
        pod: &Pod,
        nose: NoseShot,
        state: u8,
        frame: u8,
        hit_points: u8,
    ) -> &mut Object {
        let (speed_x, left) =
            add_velocity(nose.speed.0, nose.left, pod.body.h_speed(), pod.body.h_left);
        let (speed_y, down) =
            add_velocity(nose.speed.1, nose.down, pod.body.v_speed(), pod.body.v_down);
        let shot = self.create(data, nose.slot, state, frame, nose.x, nose.y);
        shot.hit_points = hit_points;
        shot.speed_x = speed_x;
        shot.speed_y = speed_y;
        shot.flags_x = u8::from(left);
        shot.flags_y = u8::from(down);
        shot
    }

    /// Makes up to three impact sparks for the requests of one step (player.md 5.5): each in
    /// the highest free slot of 3-10 below the previous one, with random directions from the
    /// generator's output byte.
    pub fn make_sparks(
        &mut self,
        data: &ObjectData,
        rng: &mut Rng,
        requests: &[(u8, u8, (u16, u16))],
    ) {
        self.make_sparks_as(data, rng, requests, state::SPARK);
    }

    /// [`make_sparks`](Self::make_sparks) with the sparks in `spark_state`: 1 for the impact
    /// sparks, 2 for the rising ones a crystal makes when it is picked up.
    pub fn make_sparks_as(
        &mut self,
        data: &ObjectData,
        rng: &mut Rng,
        requests: &[(u8, u8, (u16, u16))],
        spark_state: u8,
    ) {
        const SPEEDS: [(u16, u16); 3] = [(0x0180, 0x0140), (0x0040, 0x01C0), (0x0180, 0x0040)];
        for &(carry_if_free, life, at) in requests {
            let mut next = *SHOT_SLOTS.end() as isize;
            for (n, &(speed_x, speed_y)) in SPEEDS.iter().enumerate() {
                let found = (*SHOT_SLOTS.start() as isize..=next)
                    .rev()
                    .find(|&s| self.slots[s as usize].state == 0);
                let Some(slot) = found else { break };
                next = slot - 1;
                // The first spark's carry is the speed bit only when slot 10 was free.
                let carry = if n == 0 && slot == *SHOT_SLOTS.end() as isize {
                    carry_if_free
                } else {
                    1
                };
                let (left, down) = rng.rotate_for_directions(carry);
                let spark = self.create(data, slot as usize, spark_state, 0x62, at.0, at.1);
                spark.speed_x = speed_x;
                spark.speed_y = speed_y;
                spark.flags_x = u8::from(left);
                spark.flags_y = u8::from(down);
                spark.index = life;
            }
        }
    }

    // ---- Items (levels.md 3.3, objects.md 10) ---------------------------------------------------

    /// One call of the items scan (levels.md 3.3): up to 4 entries from the cursor, ending at
    /// the first in range or when the cursor passes the last entry (wrapping to 0). In range
    /// is −48 ≤ rx, ry < 304.
    pub fn item_scan(&mut self, data: &ObjectData, camera: Point) {
        let count = self.items.len();
        for _ in 0..4 {
            if count == 0 {
                return;
            }
            let index = self.item_cursor % count;
            let entry = self.items[index];
            let rx = i32::from(relative(entry.x, camera.x));
            let ry = i32::from(relative(entry.y, camera.y));
            let in_range = entry.kind != 0 && (-48..304).contains(&rx) && (-48..304).contains(&ry);
            let live = in_range
                && ITEM_SLOTS
                    .into_iter()
                    .any(|s| self.slots[s].state != 0 && usize::from(self.slots[s].index) == index);
            let free = self.first_free(ITEM_SLOTS);
            if in_range && !live && free.is_none() {
                // No slot: the entry is tried again next time.
                return;
            }
            // Spec gap: levels.md 3.3 does not say that passing the last entry also ends the
            // call; the cursor sequence of the original shows it does.
            self.item_cursor = (index + 1) % count;
            if in_range && !live {
                let slot = free.expect("checked");
                let (frame, state) = item_start(entry.kind);
                let old = self.slots[slot];
                let item = self.create(data, slot, state, frame, entry.x, entry.y);
                item.index = index as u8;
                item.kind = entry.kind;
                // Items get no hit points and keep work 5 from the slot's last occupant
                // (objects.md 10.1).
                item.hit_points = old.hit_points;
                item.work5 = old.work5;
            }
            if in_range || self.item_cursor == 0 {
                return;
            }
        }
    }

    /// Adds an item to the end of the list (the egg's exit, behaviour.md 3.5).
    pub fn add_item(&mut self, kind: u8, x: u16, y: u16) {
        self.items.push(ItemEntry { kind, x, y });
    }
}

/// The memory of destroyed placed objects across visits (objects.md 3.6): the presence of the
/// last level left in each group (levels 0-12 and 13-25).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct LevelMemory {
    /// Level index and presence of its entries, per group.
    groups: [Option<(u8, Vec<bool>)>; 2],
}

impl LevelMemory {
    fn group(level: usize) -> Option<usize> {
        match level {
            0..=12 => Some(0),
            13..=25 => Some(1),
            _ => None,
        }
    }

    /// Saves which objects of `level` are still present, as the player leaves it.
    pub fn save(&mut self, level: usize, table: &ObjectTable) {
        if let Some(group) = Self::group(level) {
            let present = table.present.iter().map(|&t| t != 0).collect();
            self.groups[group] = Some((level as u8, present));
        }
    }

    /// Applies the memory to a fresh type list of `level`: entries that were gone stay gone.
    pub fn restore(&self, level: usize, table: &mut ObjectTable) {
        let Some(group) = Self::group(level) else {
            return;
        };
        if let Some((saved, present)) = &self.groups[group]
            && usize::from(*saved) == level
        {
            for (entry, &here) in table.present.iter_mut().zip(present) {
                if !here {
                    *entry = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    #[test]
    fn explosions_run_through_their_sequence() {
        let world = synthetic_world();
        let data = &world.objects;
        let mut table = ObjectTable::new(data, &world.levels[0]);
        table.slots[23].state = 0x86;
        table.slots[23].frame = 0x1B;
        let mut score = Score(0);
        table.destroy(data, 23, &mut score);
        // A frame of half sizes 4 + 4 = 8: medium, sequence from 5.
        assert_eq!(table.slots[23].state, 5);
        assert_eq!(score, Score(5));
        let mut frames = Vec::new();
        let mut steps = 0;
        while table.slots[23].state != 0 {
            table.step_explosion(23);
            steps += 1;
            frames.push(table.slots[23].frame);
        }
        // Medium: 5 frames of 4 steps, freed 4 steps after the last (objects.md 2.5): 21 steps.
        assert_eq!(steps, 21);
        assert_eq!(frames[0], 4);
        assert_eq!(frames[16], 8);
    }

    #[test]
    fn memory_keeps_destroyed_objects_away() {
        let world = synthetic_world();
        let data = &world.objects;
        let mut table = ObjectTable::new(data, &world.levels[0]);
        table.present = vec![5, 0, 7];
        let mut memory = LevelMemory::default();
        memory.save(0, &table);
        let mut fresh = ObjectTable::new(data, &world.levels[0]);
        fresh.present = vec![5, 6, 7];
        memory.restore(0, &mut fresh);
        assert_eq!(fresh.present, vec![5, 0, 7]);
        // Another level of the group replaces the memory.
        memory.save(3, &fresh);
        let mut again = ObjectTable::new(data, &world.levels[0]);
        again.present = vec![5, 6, 7];
        memory.restore(0, &mut again);
        assert_eq!(again.present, vec![5, 6, 7]);
    }
}
