//! The object table (`docs/spec/objects.md` 2-3): 31 slots, the placement scan and despawning,
//! destruction and explosions, the items list, and the level memory (3.6).
//!
//! Behaviours are in [`super::behave`], collisions in [`super::collide`].

use super::objdata::ObjectData;
use super::pod::Pod;
use super::rng::Rng;
use crate::game::level::{Level, Placement, Point};
use crate::game::sound::{Channel, Request};

/// Slots 0-30; slot 0 is the pod and lives in [`Pod`], so the table's slot 0 stays free.
pub const SLOTS: usize = 31;
pub const SHOT_SLOTS: std::ops::RangeInclusive<usize> = 3..=10;
pub const ITEM_SLOTS: std::ops::RangeInclusive<usize> = 11..=14;
pub const CHILD_SLOTS: std::ops::RangeInclusive<usize> = 15..=22;
pub const PLACED_SLOTS: std::ops::RangeInclusive<usize> = 23..=30;

/// States used by the rules here (objects.md 2.4, behaviour.md 5).
pub mod state {
    pub const FREE: u8 = 0;
    pub const SPARK: u8 = 1;
    /// A spark that rises: the sparks of a picked-up crystal.
    pub const SPARK_RISING: u8 = 2;
    pub const EXHAUST: u8 = 3;
    pub const EXPLOSION_SMALL: u8 = 4;
    pub const EXPLOSION_LARGE: u8 = 5;
    pub const MARKER: u8 = 6;
    pub const MOTHERSHIP: u8 = 7;
    pub const BULLET: u8 = 8;
    /// Slot 2 as the tractor beam (player.md 8.4).
    pub const BEAM: u8 = 9;
    /// Slot 2 as the shield ring (player.md 4.7a).
    pub const SHIELD_RING: u8 = 0x0C;
    /// The pick-up sparkle (objects.md 7, behaviour.md 6.4).
    pub const SPARKLE: u8 = 0x0B;
    /// The warp level's countdown digit (behaviour.md 6.3).
    pub const COUNTDOWN: u8 = 0x0D;
    pub const ITEM: u8 = 0x0A;
    pub const RING: u8 = 0x0E;
    pub const MISSILE: u8 = 0x0F;
    pub const TIME_BOMB: u8 = 0x10;
    pub const STAR_PARENT: u8 = 0x12;
    pub const STAR_CHILD: u8 = 0x13;
    pub const MULTI_WARHEAD: u8 = 0x14;
    pub const ENEMY_SHOT: u8 = 0x15;
    pub const PIECE: u8 = 0x21;
    pub const FLAME: u8 = 0x17;
    pub const FLAME_LOW: u8 = 0x18;
    pub const DRONE: u8 = 0x19;
    pub const GUN_SHOT: u8 = 0x1A;
    pub const HEAD_SHOT: u8 = 0x1B;
    pub const HEAD_A: u8 = 0x1C;
    pub const HEAD_B: u8 = 0x1D;
    pub const MINE: u8 = 0x1E;
    pub const HOPPER: u8 = 0x1F;
    pub const THRUSTER: u8 = 0x22;
    pub const GREEN_ORB: u8 = 0x23;
    pub const BONUS_STAR: u8 = 0x24;
    pub const BONUS_BOX: u8 = 0x25;
    pub const TURRET_FIRST: u8 = 0x86;
    pub const TURRET_LAST: u8 = 0x95;
    pub const DOME: u8 = 0x97;
    pub const GUN_12: u8 = 0x98;
    pub const WALKER_A: u8 = 0x99;
    pub const WALKER_B: u8 = 0x9A;
    pub const PILLAR_A: u8 = 0x9B;
    pub const PILLAR_B: u8 = 0x9C;
    pub const GUN_17: u8 = 0x9D;
    pub const GUN_18: u8 = 0x9E;
    pub const HOURGLASS: u8 = 0x9F;
    pub const HOURGLASS_UP: u8 = 0xA0;
    pub const BLOCK: u8 = 0xA1;
    pub const MACHINE_A: u8 = 0xA2;
    pub const MACHINE_B: u8 = 0xA3;
    pub const DOME_LIFT: u8 = 0xA4;
    pub const MAGNET_PULL: u8 = 0xA5;
    pub const MAGNET_PUSH: u8 = 0xA6;
    pub const CREATURE: u8 = 0xA7;
    pub const BIG_MACHINE: u8 = 0xA8;
    pub const FORMATION_A: u8 = 0xA9;
    pub const FORMATION_B: u8 = 0xAA;
    pub const GREEN: u8 = 0xAB;
    pub const GUN_47: u8 = 0xAC;
    pub const GENERATOR_FIRST: u8 = 0xAD;
    pub const SPHERE: u8 = 0xAF;
    pub const GENERATOR_LAST: u8 = 0xB3;
    pub const FINAL_FIRST: u8 = 0xB4;
    pub const FINAL_LAST: u8 = 0xB6;
    pub const CORE: u8 = 0xB7;
    pub const EGG: u8 = 0xB8;
    pub const SATELLITE: u8 = 0xB9;
    pub const COMPANION: u8 = 0xBA;
}

/// Frame $FF: invisible.
pub const INVISIBLE: u8 = 0xFF;

/// One object slot (objects.md 2.1). Speeds are magnitudes in 1/256 px per step, the direction
/// is in the flags (behaviour.md 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Object {
    pub state: u8,
    pub x: u16,
    pub x_fraction: u8,
    pub y: u16,
    pub y_fraction: u8,
    /// Work 1/2 and work 3/4: the horizontal and vertical speed (some placed types keep their
    /// home position here).
    pub speed_x: u16,
    pub speed_y: u16,
    pub flags_x: u8,
    pub flags_y: u8,
    pub frame: u8,
    /// RAM $0393: list index of a placed object, else a timer or work byte.
    pub index: u8,
    /// RAM $03B2: object type or item kind, else a work byte.
    pub kind: u8,
    /// Work 5 and 6, RAM $0403 and $0422.
    pub work5: u8,
    pub work6: u8,
    pub hit_points: u8,
}

impl Object {
    pub fn left(&self) -> bool {
        self.flags_x & 1 != 0
    }

    pub fn down(&self) -> bool {
        self.flags_y & 1 != 0
    }

    /// Whether the slot holds a placed object (state bit 7, objects.md 2.4).
    pub fn is_placed(&self) -> bool {
        self.state & 0x80 != 0
    }

    /// The standard move (behaviour.md 2.1): each axis as a 24-bit sum of position and
    /// sub-pixel. A horizontal whole byte with bit 7 set is a negative speed.
    pub fn advance(&mut self) {
        self.advance_x();
        self.advance_y();
    }

    pub fn advance_x(&mut self) {
        let mut speed = i32::from(self.speed_x);
        let mut left = self.left();
        if self.speed_x & 0x8000 != 0 {
            // A signed (negative) speed moves the other way by its magnitude.
            speed = i32::from(0u16.wrapping_sub(self.speed_x));
            left = !left;
        }
        let position = i32::from(self.x) << 8 | i32::from(self.x_fraction);
        let moved = if left {
            position - speed
        } else {
            position + speed
        };
        self.x = (moved >> 8) as u16;
        self.x_fraction = moved as u8;
    }

    pub fn advance_y(&mut self) {
        let position = i32::from(self.y) << 8 | i32::from(self.y_fraction);
        let speed = i32::from(self.speed_y);
        let moved = if self.down() {
            position + speed
        } else {
            position - speed
        };
        self.y = (moved >> 8) as u16;
        self.y_fraction = moved as u8;
    }

    /// Moves horizontally by the speed, left or right as given (some behaviours keep the
    /// direction in another bit than bit 0).
    pub fn advance_x_toward(&mut self, left: bool) {
        let position = i32::from(self.x) << 8 | i32::from(self.x_fraction);
        let speed = i32::from(self.speed_x);
        let moved = if left {
            position - speed
        } else {
            position + speed
        };
        self.x = (moved >> 8) as u16;
        self.x_fraction = moved as u8;
    }

    /// Homing on a target's x (behaviour.md 2.3), then the move on that axis: speeds up when the
    /// direction points at the target (a target at the same coordinate counts as right),
    /// else slows down and turns at 0, capped at `max`.
    pub fn home_x(&mut self, target: u16, accel: u16, max: u16) {
        let mut left = self.left();
        // Spec gap: the side is the sign of the 16-bit difference (an orb above the map's top
        // edge, y wrapped to $FFxx, still heads down toward the pod; level 10).
        let toward = if (target.wrapping_sub(self.x) as i16) >= 0 {
            !left
        } else {
            left
        };
        if toward {
            Object::speed_up(&mut self.speed_x, accel);
        } else {
            self.speed_x = self.speed_x.saturating_sub(accel);
            if self.speed_x == 0 {
                left = !left;
                self.flags_x = (self.flags_x & !1) | u8::from(left);
            }
        }
        if self.speed_x >= max {
            self.speed_x = max;
        }
        self.advance_x();
    }

    /// The same on y (down counts as the direction of a target at the same coordinate).
    pub fn home_y(&mut self, target: u16, accel: u16, max: u16) {
        let mut down = self.down();
        let toward = if (target.wrapping_sub(self.y) as i16) >= 0 {
            down
        } else {
            !down
        };
        if toward {
            Object::speed_up(&mut self.speed_y, accel);
        } else {
            self.speed_y = self.speed_y.saturating_sub(accel);
            if self.speed_y == 0 {
                down = !down;
                self.flags_y = (self.flags_y & !1) | u8::from(down);
            }
        }
        if self.speed_y >= max {
            self.speed_y = max;
        }
        self.advance_y();
    }

    /// Speeds up one axis by `amount`: a whole part of 3 or more becomes exactly 3.00
    /// (behaviour.md 2.2).
    pub fn speed_up(speed: &mut u16, amount: u16) {
        *speed = speed.saturating_add(amount);
        if *speed >> 8 >= 3 {
            *speed = 0x0300;
        }
    }
}

/// An item of the level's items list (levels.md 3.3, economy.md 2.1); kind 0 is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItemEntry {
    pub kind: u8,
    pub x: u16,
    pub y: u16,
}

/// Frame and state of an item kind (levels.md 3.3, behaviour.md 6.1).
pub fn item_start(kind: u8) -> (u8, u8) {
    match kind {
        1 => (0x54, state::ITEM),
        3 => (0x59, state::ITEM),
        4 => (0x5A, state::ITEM),
        5 => (0x58, state::ITEM),
        6 => (0x57, state::ITEM),
        7 => (0x5A, state::ITEM),
        8 => (0x5B, state::ITEM),
        9 => (0x5E, state::ITEM),
        10 => (0x5C, state::ITEM),
        11 => (0x60, state::ITEM),
        12 => (0x61, state::ITEM),
        13 => (0x62, state::ITEM),
        14 => (0x64, state::ITEM),
        15 => (0x65, state::ITEM),
        16 => (0x66, state::ITEM),
        17 | 26 => (0x67, state::RING),
        18 => (0x52, state::ITEM),
        19 => (0x53, state::ITEM),
        20 => (0x52, state::ITEM),
        21 => (INVISIBLE, state::MOTHERSHIP),
        22 => (0x8A, state::MOTHERSHIP),
        23 => (0x8B, state::MOTHERSHIP),
        24 => (0x8C, state::MOTHERSHIP),
        25 => (0x8D, state::MOTHERSHIP),
        27 => (0x7E, state::ITEM),
        // Kind 2 and the kinds no list uses have no valid start.
        _ => (INVISIBLE, 0),
    }
}

/// The frames of an explosion by position in its sequence (objects.md 2.5).
pub(crate) const EXPLOSION_SEQUENCE: [u8; 19] = [
    0x00, 0x01, 0x02, 0x03, 0xFF, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
    0x0E, 0x0F, 0xFF,
];

/// Points added by the kill of a placed object, by explosion state 4 or 5 (objects.md 2.5).
pub(crate) const KILL_POINTS: [u32; 2] = [3, 5];

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
    /// Validation mode (`Gameplay::set_original_rules`): the original's quirks run instead of
    /// the remake's deviations (docs/deviations.md).
    pub original_rules: bool,
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

/// The score RAM $CB-$D0: six decimal digits, added with a wrap past 999999 (economy.md 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Score(pub u32);

impl Score {
    /// The start of a new game (economy.md 1).
    pub const NEW_GAME: Score = Score(1000);

    pub fn add(&mut self, points: u32) {
        self.0 = (self.0 + points) % 1_000_000;
    }

    /// The digits as hexadecimal digits of a number, as the capture script reads RAM.
    pub fn as_digits(self) -> u32 {
        u32::from_str_radix(&format!("{:06}", self.0), 16).unwrap_or(0)
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
    /// RAM $05 as a slot's behaviour finds it, where the distance routine would have left |dy|
    /// (behaviour.md 2.4, Q24): 255 unless fed from the original.
    pub scratch_dy: std::cell::Cell<u8>,
    /// Validation only: the value RAM $05 holds when the sweep reaches a slot, from the
    /// original (behaviour.md 2.4, Q24; not modelled).
    pub scratch_feed: [Option<u8>; SLOTS],
    /// The sound requests the objects made since the pass took them (sound.md 8).
    pub sfx: Vec<Request>,
    /// The warp level's countdown ran out in the last sweep (behaviour.md 6.3).
    pub countdown_done: bool,
    /// A shot hit an open core of the final level, RAM $EC (behaviour.md 4.16).
    pub core_hit: bool,
}

/// Penetration depth of two boxes on one axis (behaviour.md 3.1): `d` is the distance from the
/// target to the mover, `w` the target's half size and `p` the mover's. None if they do not
/// overlap.
pub fn penetration(d: i16, w: i32, p: i32) -> Option<i32> {
    let d = i32::from(d);
    if d < 0 {
        let c = i32::from(-d <= w);
        let depth = w + p + d + c;
        (depth >= 0).then_some(depth)
    } else {
        let c = i32::from(d < w);
        let depth = w + p - d + c;
        (depth > 0).then_some(depth)
    }
}

/// `value - origin` as a signed 16-bit number (objects.md 3.4).
pub fn relative(value: u16, origin: u16) -> i16 {
    value.wrapping_sub(origin) as i16
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
            scratch_dy: std::cell::Cell::new(0xFF),
            scratch_feed: [None; SLOTS],
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
    fn spawn(&mut self, data: &ObjectData, level: &Level, index: usize, stale: bool) -> bool {
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
        let old = self.slots[slot];
        let object = self.create(data, slot, state, frame, place.x, place.y);
        if stale {
            // Spec gap and quirk Q17: objects.md 3.4 says all fields are cleared; a type 22
            // machine that took over a moving machine's slot started with its x fraction.
            object.x_fraction = old.x_fraction;
            object.y_fraction = old.y_fraction;
        }
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
                self.spawn(data, level, index, false);
            }
        }
    }

    /// The scan of one logic step's B pass: at most 8 entries, at most 1 spawn (3.4). Not run
    /// on warp levels. Returns the thrust's carry-in byte RAM $0D as the scan leaves it: cleared
    /// at the start, then the `dy` byte of the last entry it examines when that is a group member;
    /// `None` when the scan does not run.
    pub fn step_scan(
        &mut self,
        data: &ObjectData,
        level: &Level,
        camera: Point,
        stale: bool,
    ) -> Option<u8> {
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
            let spawned = wanted && self.spawn(data, level, index, stale);
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
    pub fn fire_pod_shot(
        &mut self,
        data: &ObjectData,
        pod: &Pod,
        strength: u8,
        stale: bool,
    ) -> bool {
        let Some(nose) = self.nose_shot(pod) else {
            return false;
        };
        let shot = self.make_pod_shot(data, pod, nose, state::BULLET, 0x37, strength, stale);
        shot.index = 32;
        true
    }

    /// Makes the shot `nose` describes in its slot, with the shooter's velocity added to its own
    /// (player.md 10). The sub-pixel fractions stay as the slot's last occupant left them when
    /// `stale` (a spec gap, as with enemy shots: behaviour.md 2.1 says they are cleared on
    /// creation).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn make_pod_shot(
        &mut self,
        data: &ObjectData,
        pod: &Pod,
        nose: NoseShot,
        state: u8,
        frame: u8,
        hit_points: u8,
        stale: bool,
    ) -> &mut Object {
        let (speed_x, left) =
            add_velocity(nose.speed.0, nose.left, pod.body.h_speed, pod.body.h_left);
        let (speed_y, down) =
            add_velocity(nose.speed.1, nose.down, pod.body.v_speed, pod.body.v_down);
        let old = self.slots[nose.slot];
        let shot = self.create(data, nose.slot, state, frame, nose.x, nose.y);
        if stale {
            shot.x_fraction = old.x_fraction;
            shot.y_fraction = old.y_fraction;
        }
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
        stale: bool,
    ) {
        self.make_sparks_as(data, rng, requests, stale, state::SPARK);
    }

    /// [`make_sparks`](Self::make_sparks) with the sparks in `spark_state`: 1 for the impact
    /// sparks, 2 for the rising ones a crystal makes when it is picked up.
    pub fn make_sparks_as(
        &mut self,
        data: &ObjectData,
        rng: &mut Rng,
        requests: &[(u8, u8, (u16, u16))],
        stale: bool,
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
                // Spec gap: like shots, a spark keeps the hit points and sub-pixel fractions its
                // slot's last occupant left.
                let old = self.slots[slot as usize];
                let spark = self.create(data, slot as usize, spark_state, 0x62, at.0, at.1);
                if stale {
                    spark.hit_points = old.hit_points;
                    spark.x_fraction = old.x_fraction;
                    spark.y_fraction = old.y_fraction;
                }
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

/// The start of the gun's next shot ([`ObjectTable::nose_shot`]).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct NoseShot {
    pub slot: usize,
    pub x: u16,
    pub y: u16,
    /// Horizontal and vertical speed (8.8) before the shooter's velocity is added.
    pub speed: (u16, u16),
    pub left: bool,
    pub down: bool,
}

/// The offsets of the pod's nose and flame by facing (player.md 10, bank 0 $CAC3).
pub(crate) const NOSE: [u8; 40] = [
    0x0A, 0x0A, 0x08, 0x07, 0x07, 0x05, 0x03, 0x01, 0x00, 0xFE, 0xFC, 0xFA, 0xF9, 0xF8, 0xF7, 0xF6,
    0xF6, 0xF6, 0xF8, 0xF8, 0xF9, 0xFB, 0xFD, 0xFE, 0x00, 0x02, 0x03, 0x06, 0x07, 0x08, 0x09, 0x0A,
    0x0A, 0x0A, 0x09, 0x08, 0x07, 0x05, 0x03, 0x02,
];

/// Adds two velocities on one axis, each a magnitude with a direction. Spec gap: a sum in
/// opposite directions keeps the larger one's direction.
pub(crate) fn add_velocity(a: u16, a_negative: bool, b: u16, b_negative: bool) -> (u16, bool) {
    if a_negative == b_negative {
        (a.saturating_add(b), a_negative)
    } else if a >= b {
        (a - b, a_negative)
    } else {
        (b - a, b_negative)
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
    fn penetration_follows_the_worked_case() {
        // behaviour.md 3.1: object w = 4, pod 3.
        assert_eq!(penetration(-7, 4, 3), Some(0));
        assert_eq!(penetration(-4, 4, 3), Some(4));
        assert_eq!(penetration(3, 4, 3), Some(5));
        assert_eq!(penetration(4, 4, 3), Some(3));
        assert_eq!(penetration(7, 4, 3), None);
        assert_eq!(penetration(-8, 4, 3), None);
    }

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

    #[test]
    fn score_adds_in_decimal_and_wraps() {
        let mut score = Score(999_998);
        score.add(5);
        assert_eq!(score, Score(3));
        assert_eq!(Score(1000).as_digits(), 0x001000);
    }
}
