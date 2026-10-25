//! The Scaled engine (`docs/scaled.md`): gameplay whose physics tick
//! runs at a rate from 60 to 200 Hz, playing like the Original's 30 Hz logic step in real time
//! but acting on input within a tick. It is a copy of the Original ([`crate::game::gameplay`]), which
//! it never calls per step: the step logic of the copied modules runs on the frames of the game
//! clock ([`clock`]), and what the results files scale runs in each tick ([`Scaled::tick`]):
//!
//! - the controller ([`controls`]): presses kept between ticks, rotation and autofire as
//!   game-time cadences;
//! - the player's motion ([`motion`], [`pod`], [`jetman`], [`final_level`]): accelerations
//!   times the tick's fraction f of a step and a move exact at every step, thrust acting in the
//!   tick that sees B;
//! - the terrain contact ([`pod::Pod::terrain_test`]): bands from the tick's move, a full
//!   response only when closing (or once a step), effects at most once a step per kind;
//! - the tow rope ([`tow`]), the camera ([`camera`]) and the stars ([`stars`]);
//! - the pod against the objects ([`collide`]): each object at most once a step.
//!
//! The objects' logic runs once a step at the step's start, against the pod a step ahead, and
//! they are drawn on their way to the places it gave them. The frame events run the random
//! generator and the rest of the passes in the Original's order.
//!
//! State is rate-free: game time and values in their Q formats (positions Q16.16, speeds Q8.24),
//! so a state saved at one rate plays on at another. [`rate`] chooses the rate from the display.

pub mod behave;
pub mod camera;
pub mod clock;
pub mod collide;
pub mod controls;
pub mod economy;
pub mod enemies;
pub mod final_level;
pub mod gate;
pub mod jetman;
pub mod minions;
pub mod motion;
pub mod objects;
pub mod pod;
pub mod rate;
pub mod stars;
pub mod tow;
pub mod warp;
pub mod weapons;

// The read-only data and the pure helpers are the Original's.
pub use crate::game::gameplay::{Arrival, Cheats, PodLeftovers, Seed};
pub use crate::game::gameplay::{input, inventory, objdata, rng, sprites, tables, terrain};

#[cfg(test)]
mod tests;

use crate::game::graphics::LevelGraphics;
use crate::game::level::{Point, World};
use crate::game::playfield::Frame;
use crate::game::sound::{Channel, Request};
use crate::game::view::Viewport;
use camera::Camera;
use economy::Transition;
use input::Buttons;
use objects::{Env, LevelMemory, ObjectTable, Score};
use pod::{Context, Pod, STATE_ASSEMBLING, STATE_POD_EXPLODING};
use rng::Rng;
use sprites::{SpriteFile, Sprites};
use stars::Stars;
use tow::{Moment, Tow};

/// The same state in the Original's type: the types that cross the scene layer's [`Seed`] (the
/// memory of destroyed objects, the parallax stars) have the same shape in both engines.
pub(crate) fn convert<A: serde::Serialize, B: serde::de::DeserializeOwned>(value: &A) -> B {
    let text = ron::ser::to_string(value).expect("the state serialises");
    ron::de::from_str(&text).expect("both engines' types have the same shape")
}

/// A state as a RON value.
fn as_value<T: serde::Serialize>(value: &T) -> ron::Value {
    let text = ron::ser::to_string(value).expect("the state serialises");
    ron::de::from_str(&text).expect("the state reads back")
}

/// What is drawn at one moment: the camera and the pod's position, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
struct View {
    camera: (f32, f32),
    pod: (f32, f32),
    /// Slots 1-30: position, and which object it is (state, list index) so that a slot taken
    /// over by another object is not blended.
    objects: [Option<(f32, f32, u8, u8)>; objects::SLOTS],
}

/// Scaled on one level.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Scaled {
    level: usize,
    /// Game time since the level started.
    #[serde(default)]
    clock: clock::Clock,
    /// The controller as the ticks read it.
    #[serde(default)]
    controls: controls::Controls,
    /// Crystals touched on a warp level, RAM $67 (economy.md 5.4).
    pub warp_crystals: u8,
    /// The item whose picture the status panel shows as the fitted weapon, set when the level
    /// loads and kept (graphics.md 1.2).
    panel_icon: Option<u8>,
    /// The final level's terrain colours (graphics.md 7.5).
    pub terrain: final_level::TerrainColours,
    /// The beam was made in this step's A pass: its picture is the catch's frame $38, the B
    /// pass's frame write coming after its sprites (player.md 8.4).
    beam_caught: bool,
    pub pod: Pod,
    pub camera: Camera,
    pub rng: Rng,
    /// The frame counter RAM $5F:$5E:$5D, 24 bits.
    counter: u32,
    /// The rotating job counter RAM $60 (set to 8 at level start).
    job: u8,
    held: Buttons,
    pressed: Buttons,
    /// The next controller read starts a scene's input: nothing counts as newly pressed.
    first_read: bool,
    /// The object slots 1-30 (slot 0 is the pod) and what goes with them.
    pub objects: ObjectTable,
    pub score: Score,
    /// Jetman lives and jetpods, shown in the status panel.
    pub lives: u8,
    pub jetpods: u8,
    /// The tractor beam (player.md 8).
    pub tow: Tow,
    /// The final level's camera route (levels.md 5.2).
    route: final_level::Route,
    effect: final_level::Effect,
    /// The final level's end has been asked for.
    finished: bool,
    /// The gate state, while the player is drawn through a wormhole (player.md 9.2).
    gate: Option<gate::Gate>,
    /// The pickup tag RAM $D2 without its anti-gravity bit 7 (economy.md 2.5).
    bonus_tag: u8,
    /// The points picture last written into the bonus sign's sprite tiles $BC and $D0 (the
    /// tag's low nibble, behaviour.md 6.2); none since the level started.
    #[serde(default)]
    sign_picture: Option<u8>,
    /// The port menu's cheats, set by the program (not part of a saved state).
    #[serde(skip)]
    cheats: Cheats,
    /// The view's width (`docs/widescreen.md`), set by the program (not part of a saved state).
    #[serde(skip, default = "crate::game::view::original_width")]
    view_width: u16,
    /// Paused: Start was seen in an A pass (game-flow.md 4); `pause_frame` runs the frames until
    /// the next Start.
    paused: bool,
    /// The sound requests made since the caller took them ([`Scaled::take_sound`]).
    sfx: Vec<Request>,
    pause_held: Buttons,
    /// Crystals stored, RAM $E0 (economy.md 2.4).
    pub crystals: u8,
    /// The mothership's fuel count saved while a second or warp level is played, RAM $0636.
    saved_fuel_count: u8,
    pub mapping_device: bool,
    pub super_mapping_device: bool,
    pub planet_flags: u16,
    /// The scene change requested, RAM $3F, the last one acted on $40 and the guard $41
    /// (game-flow.md 7.4).
    transition: Option<Transition>,
    last_transition: u8,
    guard: u8,
    memory: LevelMemory,
    /// The parallax stars, and the stars as the last B pass left them for drawing: screen x, y
    /// and sprite palette.
    stars: Stars,
    star_sprites: Vec<(u8, u8, u8)>,

    /// What the last two ticks showed, for drawing between them.
    shown_before: View,
    shown_now: View,
    /// The objects' places as the step's logic began (game time `step_at`), which they move
    /// from to the places its logic gave them over the step (`docs/scaled.md`).
    #[serde(default)]
    step_from: Vec<Option<Place>>,
    #[serde(default)]
    step_at: u64,
    /// When each object may next touch the pod (`docs/scaled.md`).
    #[serde(default)]
    touch_ready: [u64; objects::SLOTS],
    /// The camera after the last frames (game time, place), for the final level's screen
    /// limits, which are a step behind the camera (`docs/scaled.md`).
    #[serde(default)]
    camera_trail: Vec<(u64, Point)>,
    /// The pod's place as the step's logic began, while that logic moves it (the gate state),
    /// for drawing it on its way (`docs/scaled.md`).
    #[serde(default)]
    pod_from: Option<(u32, u32)>,
}

/// An object's place in 1/256 px and which object it is (state, list index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Place {
    x: u32,
    y: u32,
    state: u8,
    index: u8,
}

fn places(table: &ObjectTable) -> Vec<Option<Place>> {
    table
        .slots
        .iter()
        .map(|o| {
            (o.state != 0).then_some(Place {
                x: u32::from(o.x) << 8 | u32::from(o.x_fraction),
                y: u32::from(o.y) << 8 | u32::from(o.y_fraction),
                state: o.state,
                index: o.index,
            })
        })
        .collect()
}

/// The place `phase` (Q16) of the way from `from` to `to`: the same object, moved less than 16
/// pixels on each axis (a jump to a spawn point or across the 16-bit wrap is not blended).
fn between(from: Place, to: Place, phase: u32) -> Option<(u32, u32)> {
    if (from.state, from.index) != (to.state, to.index) {
        return None;
    }
    let axis = |a: u32, b: u32| {
        let d = i64::from((b.wrapping_sub(a) << 8) as i32 >> 8);
        (d.abs() < 16 << 8)
            .then(|| (i64::from(a) + d * i64::from(phase) / 65536) as u32 & 0xFF_FFFF)
    };
    Some((axis(from.x, to.x)?, axis(from.y, to.y)?))
}

fn object_views(table: &ObjectTable) -> [Option<(f32, f32, u8, u8)>; objects::SLOTS] {
    let mut views = [None; objects::SLOTS];
    for (view, o) in views.iter_mut().zip(&table.slots).skip(1) {
        if o.state != 0 {
            *view = Some((
                f32::from(o.x) + f32::from(o.x_fraction) / 256.0,
                f32::from(o.y) + f32::from(o.y_fraction) / 256.0,
                o.state,
                o.index,
            ));
        }
    }
    views
}

fn pod_position(pod: &Pod) -> (f32, f32) {
    (
        f32::from(pod.body.x) + f32::from(pod.body.x_fraction()) / 256.0,
        f32::from(pod.body.y) + f32::from(pod.body.y_fraction()) / 256.0,
    )
}

impl Scaled {
    /// Starts gameplay on `level`: the pod at home, the camera placed from it.
    pub fn new(world: &World, level: usize, seed: Seed) -> Scaled {
        let info = &world.levels[level];
        let mut pod = Pod::at_level_start(info);
        pod.equipment.owned = seed.equipment;
        if info.number == final_level::FINAL_LEVEL {
            pod.state = pod::STATE_FINAL;
        }
        pod.equipment.weapon = seed.weapon;
        pod.equipment.device = seed.device;
        pod.equipment.bullet_upgrade = seed.bullet_upgrade;
        if let Some(gauge) = seed.weapon_gauge {
            (pod.weapon_energy, pod.weapon_low) = ((gauge >> 8) as u8, gauge as u8);
        }
        let left = seed.pod_leftovers;
        pod.move_angle = left.move_angle;
        pod.invulnerable = left.invulnerable;
        pod.accumulator_h = u32::from(left.accumulator_h) << 8;
        pod.accumulator_v = u32::from(left.accumulator_v) << 8;
        pod.camera_carry = left.camera_carry;
        if let Some(a) = &seed.arrival {
            if info.number != final_level::FINAL_LEVEL {
                pod.state = a.state;
                if matches!(a.state, 2..=4) {
                    pod.half_height = 9;
                }
            }
            pod.fuel = a.fuel;
            pod.fuel_shown = a.fuel_shown;
            pod.body.set_h_speed(0);
            pod.body.set_v_speed(0x0040);
            pod.body.h_left = a.h_left;
            pod.body.v_down = a.v_down;
            pod.move_angle = a.move_angle;
            pod.timer = a.timer;
            pod.counter = a.counter;
            pod.facing = a.facing;
            pod.invulnerable = a.invulnerable;
            if a.lowered {
                pod.body.y = pod.body.y.saturating_sub(128);
            }
        }
        // The level's set-up runs the drawing stage once before play (game-flow.md 7.3): the F
        // gauge eases one step, so play after a launch starts at 3 (economy.md 3.1).
        pod.ease_fuel_display();
        if seed.final_entry {
            pod.body.set_h_speed(0);
            pod.body.set_v_speed(0x0180);
            (pod.body.h_left, pod.body.v_down) = (false, false);
            (pod.fuel, pod.invulnerable, pod.shield) = (pod::FULL_FUEL, 0, false);
        }
        // The pod starts inside the mothership's bay area (economy.md 2.2).
        pod.bay = usize::from(info.number) < crate::game::level::PLANETS
            && economy::in_bay_area(&pod, info);
        let camera = if seed.final_entry {
            Camera {
                position: info.camera_start,
            }
        } else {
            Camera::at_start(info, pod.body.x, pod.body.y)
        };
        let mut objects = ObjectTable::new(&world.objects, info);
        for (slot, old) in objects.slots.iter_mut().zip(&seed.stale) {
            // The load clears the state and the four speed arrays; the rest stays (objects.md 2.2).
            *slot = objects::Object {
                state: 0,
                speed_x: 0,
                speed_y: 0,
                ..*old
            };
        }
        for (slot, &(flags_y, frame)) in objects.slots.iter_mut().zip(&seed.leftovers) {
            slot.flags_y = flags_y;
            slot.frame = frame;
        }
        let memory: LevelMemory = convert(&seed.memory);
        memory.restore(level, &mut objects);
        if let Some(count) = seed.arrival.and_then(|a| a.fuel_count)
            && let Some(entry) = objects.items.first_mut()
        {
            entry.kind = count;
        }
        let view_width = crate::game::view::clamp_width(seed.view_width);
        let start_view = Viewport::around(info, camera.position, view_width);
        objects.start_scan(&world.objects, info, start_view);
        // One call of the items scan is part of the level start: the first item is in its slot
        // before the first step (levels.md 3.3; measured). Not on a warp level, where the first
        // crystal comes with the first step.
        if !info.is_warp_level() {
            objects.item_scan(&world.objects, start_view);
        }
        let view = View {
            camera: (f32::from(camera.position.x), f32::from(camera.position.y)),
            pod: pod_position(&pod),
            objects: object_views(&objects),
        };
        let panel_icon = inventory::WEAPON_CODES
            .iter()
            .position(|&code| code == seed.weapon && code != 0)
            .map(|item| item as u8);
        // The level load's frame runs the per-level routine too, and its colours show from
        // the next frame (levels.md 6.6).
        let mut terrain = final_level::TerrainColours::default();
        terrain.by_depth(info.number, camera.position.y, true);
        let mut game = Scaled {
            level,
            clock: clock::Clock::default(),
            controls: controls::Controls::default(),
            panel_icon,
            beam_caught: false,
            terrain,
            warp_crystals: 0,
            pod,
            camera,
            rng: seed.rng,
            counter: seed.counter & 0xFF_FFFF,
            job: 8,
            objects,
            score: seed.score,
            lives: seed.lives,
            jetpods: seed.jetpods,
            bonus_tag: 0,
            sign_picture: None,
            route: final_level::Route::default(),
            effect: final_level::Effect::default(),
            finished: false,
            gate: None,
            tow: Tow {
                spin: seed.pod_leftovers.tow_spin,
                angle: seed
                    .arrival
                    .map_or(0, |a| u32::from(a.rope.0 & 0x7FFF) << 16),
                speed: seed.arrival.map_or(0, |a| a.rope.1 << 8),
                sense: seed.arrival.is_some_and(|a| a.rope.2),
                ..Tow::default()
            },
            cheats: Cheats::default(),
            view_width,
            paused: false,
            sfx: Vec::new(),
            pause_held: Buttons::default(),
            crystals: seed.crystals,
            saved_fuel_count: seed.saved_fuel_count,
            mapping_device: seed.mapping_device,
            super_mapping_device: seed.super_mapping_device,
            planet_flags: seed.planet_flags,
            transition: None,
            last_transition: match usize::from(info.number) {
                0..=12 => 2,
                // Second levels and warp levels are entered by code 3.
                13..=25 | 29.. => 3,
                _ => 0,
            },
            // One decrement has happened by the first step (measured): the level-start pass.
            guard: if info.is_warp_level() { 64 } else { 63 },
            memory,
            stars: seed.stars.as_ref().map(convert::<_, Stars>).map_or_else(
                || Stars::new(seed.star_mode),
                |mut stars| {
                    stars.level_load();
                    stars
                },
            ),
            star_sprites: Vec::new(),
            held: Buttons::default(),
            pressed: Buttons::default(),
            first_read: false,
            shown_before: view,
            shown_now: view,
            step_from: Vec::new(),
            step_at: 0,
            touch_ready: [0; objects::SLOTS],
            camera_trail: Vec::new(),
            pod_from: None,
        };
        if info.is_warp_level() {
            game.start_warp_level(world);
        }
        game
    }

    /// The memory of destroyed objects including this level's, to hand to the next level
    /// (objects.md 3.6).
    pub fn memory_on_leaving(&self) -> LevelMemory {
        let mut memory = self.memory.clone();
        memory.save(self.level, &self.objects);
        memory
    }

    /// The star job's mode, to hand to the next level (graphics.md 5.7.3).
    pub fn star_mode(&self) -> u8 {
        self.stars.mode
    }

    /// A test setup for replays: the pod at rest at (`x`, `y`) and the camera at (`cx`, `cy`).
    pub fn warp(&mut self, x: u16, y: u16, cx: u16, cy: u16) {
        let body = &mut self.pod.body;
        (body.x, body.y, body.x_sub, body.y_sub) = (x, y, 0, 0);
        (body.h_mag, body.v_mag) = (0, 0);
        self.camera.position = Point { x: cx, y: cy };
    }

    /// A test setup for replays: the transition guard RAM $41 and the items scan's cursor RAM
    /// $BB as the original's level start left them.
    pub fn set_start_bytes(&mut self, guard: u8, item_cursor: usize) {
        self.guard = guard;
        self.objects.item_cursor = item_cursor;
    }

    /// A test setup for replays: entry `index` of the items list becomes `kind` at (`x`, `y`).
    pub fn set_item(&mut self, index: usize, kind: u8, x: u16, y: u16) {
        if let Some(entry) = self.objects.items.get_mut(index) {
            *entry = objects::ItemEntry { kind, x, y };
        }
    }

    /// A test setup for replays: slot `slot` becomes a fresh object of `state` (hit points from
    /// the state's table) at (`x`, `y`) with `frame` and list index `index`.
    pub fn set_object(&mut self, slot: usize, state: u8, x: u16, y: u16, frame: u8, index: u8) {
        if slot < objects::SLOTS {
            self.objects.slots[slot] = objects::Object {
                state,
                x,
                y,
                frame,
                index,
                ..objects::Object::default()
            };
        }
    }

    /// The pod's bytes a later level start inherits.
    pub fn pod_leftovers(&self) -> PodLeftovers {
        PodLeftovers {
            move_angle: self.pod.move_angle,
            invulnerable: self.pod.invulnerable,
            accumulator_h: (self.pod.accumulator_h >> 8) as u8,
            accumulator_v: (self.pod.accumulator_v >> 8) as u8,
            camera_carry: self.pod.camera_carry,
            tow_spin: self.tow.spin,
        }
    }

    /// A press of the gun with Select held and a special weapon fitted (economy.md 6.4,
    /// player.md 10): the weapon fires when the slot it takes (as for the plain shot) is one of
    /// 3-6, else nothing fires and nothing is charged, and its cost comes off the gauge after the
    /// shot (a gauge at or below the cost goes to 0). The smart bomb needs no slot. Returns
    /// whether the press was used up that way: false for a plain bullet (no weapon, an empty
    /// gauge, Select not held, or a device).
    fn fire_special_weapon(&mut self, data: &objdata::ObjectData) -> bool {
        let weapon = self.pod.equipment.weapon;
        if weapon == 0
            || !self.held.has(Buttons::SELECT)
            || self.pod.weapon_energy | self.pod.weapon_low == 0
            || !matches!(self.pod.state, pod::STATE_POD | 2 | 3)
        {
            return false;
        }
        let Some(cost) = weapons::cost(weapon) else {
            // The anti-gravity switch: no shot and no cost (player.md 10.1).
            return self.pod.press_special();
        };
        self.pod.press_special();
        let made = if weapon == weapons::SMART_BOMB {
            self.objects.smart_bomb(data, &mut self.score);
            true
        } else {
            let strength = self.pod.equipment.bullet_upgrade + 1;
            self.objects.fire_weapon(data, &self.pod, weapon, strength)
        };
        if made {
            // The gauge's high byte loses the cost; at or below 0 both bytes are cleared.
            match self.pod.weapon_energy.checked_sub(cost) {
                Some(left) if left > 0 => self.pod.weapon_energy = left,
                _ => (self.pod.weapon_energy, self.pod.weapon_low) = (0, 0),
            }
        }
        true
    }

    /// Validation: sets the bytes of parallax star `index` left by the session before.
    pub fn seed_star(&mut self, index: usize, at: (u8, u8), fractions: (u8, u8), dead: bool) {
        self.stars.set_leftover(index, at, fractions, dead);
    }

    /// Validation: sets the parallax-star job's mode left by the session before.
    pub fn seed_star_mode(&mut self, mode: u8) {
        self.stars.mode = mode;
    }

    /// Whether the game is paused (Start was seen; the picture stays as it was).
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// One frame of the pause (game-flow.md 4): the logic only watches the buttons, but the
    /// job counter, the random generator and the frame counter advance as in any frame. Returns
    /// true when a newly pressed Start ends the pause.
    pub fn pause_frame(&mut self, buttons: Buttons) -> bool {
        self.job = (self.job + 1) & 7;
        let pressed = buttons.pressed_since(self.pause_held);
        self.pause_held = buttons;
        let counter = self.counter.to_le_bytes();
        self.rng.step([
            buttons.0, pressed.0, 0, 0, counter[0], counter[1], counter[2], self.job,
        ]);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        if pressed.has(Buttons::START) {
            self.paused = false;
            // The controller bytes the next pass compares against.
            self.held = buttons;
            return true;
        }
        false
    }

    /// Whether the camera follows the final level's route: level 26 with the player in state 6.
    fn routed(&self) -> bool {
        self.level == usize::from(final_level::FINAL_LEVEL) && self.pod.state == pod::STATE_FINAL
    }

    /// The A pass of the route camera (levels.md 5.2, player.md 9.1 item 10): the camera moves
    /// by the previous move, the route code sets the next one, and the pod is carried by it.
    fn route_a_pass(&mut self, held: Buttons) -> (i32, i32) {
        let moved = self.route.move_camera(&mut self.camera.position);
        let cores = (23..=27)
            .filter(|&s| self.objects.slots[s].state >= 6)
            .count() as u8;
        // The terrain's colours (graphics.md 7.5): the route's first job, or after it the
        // core-hit script.
        let hit = std::mem::take(&mut self.objects.core_hit);
        self.terrain
            .step(true, self.route.done, self.counter, hit, cores);
        self.route
            .run(self.camera.position, cores, self.rng.out, self.counter);
        self.pod.carry(held, self.route.mx, self.route.my);
        moved
    }

    pub fn level(&self) -> usize {
        self.level
    }

    /// What the demo's pilot sees (`crate::game::pilot`).
    pub fn observe(&self) -> crate::game::pilot::Observation {
        let body = &self.pod.body;
        let signed = |magnitude: u32, negative: bool| {
            let speed = (f64::from(magnitude) / f64::from(1u32 << 24)) as f32;
            if negative { -speed } else { speed }
        };
        let towed = self.tow.holding().then(|| {
            let o = &self.objects.slots[self.tow.slot];
            (
                f32::from(o.x) + f32::from(o.x_fraction) / 256.0,
                f32::from(o.y) + f32::from(o.y_fraction) / 256.0,
            )
        });
        let mut seen = crate::game::pilot::Observation {
            level: self.level,
            time: self.clock.time() as f64 / f64::from(1u32 << 31) / 2.0,
            state: self.pod.state,
            x: f32::from(body.x) + f32::from(body.x_sub) / 65536.0,
            y: f32::from(body.y) + f32::from(body.y_sub) / 65536.0,
            vx: signed(body.h_mag, body.h_left),
            vy: signed(body.v_mag, !body.v_down),
            facing: self.pod.facing,
            fuel: self.pod.fuel,
            shield: self.pod.shield,
            has_shields: self.pod.equipment.has_shields(),
            has_boosters: self.pod.equipment.has_boosters(),
            special: self.pod.equipment.weapon != 0
                && (self.pod.weapon_energy | self.pod.weapon_low) != 0,
            towed,
            taken_in: self.tow.taken_in,
            camera: (
                f32::from(self.camera.position.x),
                f32::from(self.camera.position.y),
            ),
            ..Default::default()
        };
        seen.read_objects(
            &self.objects.slots,
            self.tow.slot,
            self.objects
                .items
                .iter()
                .map(|item| (item.kind, item.x, item.y)),
        );
        seen
    }

    /// The frame counter RAM $5D-$5F.
    pub fn counter(&self) -> u32 {
        self.counter
    }

    /// Takes the frame counter from the original after a scene change, where the loading
    /// frames the remake does not have advanced it (`docs/deviations.md`).
    pub fn set_counter(&mut self, counter: u32) {
        self.counter = counter & 0xFF_FFFF;
    }

    /// Takes the sound requests made since the last call, in the order the original made them
    /// (sound.md 8).
    pub fn take_sound(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.sfx)
    }

    /// Moves the requests the objects and the pod made into the gameplay's list.
    fn collect_sound(&mut self) {
        let objects = std::mem::take(&mut self.objects.sfx);
        let pod = std::mem::take(&mut self.pod.sfx);
        self.sfx.extend(objects);
        self.sfx.extend(pod);
    }

    /// Whether the next pass is an A pass.
    pub fn next_is_a_pass(&self) -> bool {
        self.counter & 1 == 0
    }

    /// Takes over the Original's state at the start of a logic step (the replay comparisons).
    pub fn from_original(game: &crate::game::gameplay::Gameplay) -> Scaled {
        // Everything but the pod's body and accumulators and the rope has the same shape in
        // both engines; those are put in the Scaled engine's formats.
        let text = ron::ser::to_string(game).expect("the state serialises");
        let mut value: ron::Value = ron::de::from_str(&text).expect("the state reads back");
        let original = &game.pod;
        let body = original.body;
        let scaled_body = pod::Body {
            x: body.x,
            x_sub: u16::from(body.x_fraction) << 8,
            y: body.y,
            y_sub: u16::from(body.y_fraction) << 8,
            h_mag: u32::from(body.h_speed) << 16,
            h_left: body.h_left,
            v_mag: u32::from(body.v_speed) << 16,
            v_down: body.v_down,
        };
        let mut tow = Tow {
            slot: game.tow.slot,
            mass: game.tow.mass,
            sense: game.tow.sense,
            spin: game.tow.spin,
            taken_in: game.tow.taken_in,
            ..Tow::default()
        };
        tow.set_rope(game.tow.angle, game.tow.speed);
        let field = |value: &mut ron::Value, name: &str, new: ron::Value| {
            if let ron::Value::Map(map) = value {
                map.insert(ron::Value::String(name.to_owned()), new);
            }
        };
        if let ron::Value::Map(map) = &mut value {
            if let Some(pod) = map.get_mut(&ron::Value::String("pod".to_owned())) {
                field(pod, "body", as_value(&scaled_body));
                let accumulator = |byte: u8| ron::Value::Number((u32::from(byte) << 8).into());
                field(pod, "accumulator_h", accumulator(original.accumulator_h));
                field(pod, "accumulator_v", accumulator(original.accumulator_v));
            }
            map.insert(ron::Value::String("tow".to_owned()), as_value(&tow));
        }
        let mut scaled: Scaled = value
            .into_rust()
            .expect("both engines' states have one shape");
        scaled.clock = clock::Clock::default();
        scaled
    }

    pub fn clock(&self) -> clock::Clock {
        self.clock
    }

    /// The pod's position in Q16.16 pixels.
    pub fn pod_q16(&self) -> (u32, u32) {
        (self.pod.body.x_q16(), self.pod.body.y_q16())
    }

    /// One tick at `rate` Hz (held between 30 and 200). `seen` is the controller with
    /// everything pressed since the last tick, `held` as it is now. The player moves for the
    /// tick's game time; the logic of the frames the tick crosses runs in order; the camera and
    /// the stars follow (`docs/scaled.md`).
    pub fn tick(&mut self, world: &World, seen: Buttons, held: Buttons, rate: u32) {
        if self.transition.is_some() || self.paused {
            // A scene change or a pause: the scene layer acts on it.
            return;
        }
        self.apply_cheats();
        let level = &world.levels[self.level];
        // No shooting on warp levels: A is cleared from the input (player.md 4.1).
        let strip = |buttons: Buttons| {
            if level.is_warp_level() {
                Buttons(buttons.0 & !Buttons::A)
            } else {
                buttons
            }
        };
        if std::mem::take(&mut self.first_read) {
            self.controls.settle(strip(held));
        } else {
            self.controls.read(strip(seen), strip(held));
        }
        let tick = self.clock.advance(rate);
        let f = tick.fraction();
        let within = |time: u64| ((time & 0xFFFF_FFFF) >> 16) as u32;
        let moment = Moment {
            f,
            tau: within(tick.start),
            rest: match within(tick.end) {
                0 => 0,
                done => motion::ONE - done,
            },
        };
        let before = (self.pod.body.x, self.pod.body.y);
        let camera_before = self.camera.position;
        let turn = self.controls.rotation(f);
        let period = match self.pod.equipment.owned & 3 {
            0 => 6,
            2 => 2,
            _ => 4,
        };
        let input = pod::TickInput {
            held: self.controls.held,
            pressed: self.controls.pressed,
            turn: if self.pod.state == pod::STATE_POD {
                turn
            } else {
                0
            },
            fire: self.controls.fire(f, period),
        };
        let ctx = Context {
            world,
            level,
            frame: self.counter as u8,
            now: tick.end,
            moment,
        };
        self.pod.route_camera = self.camera_at_time(tick.end.saturating_sub(clock::STEP));
        self.pod.route_margins = self.route_margins(level);
        self.pod.tick(&ctx, &input, &mut self.tow);
        self.collect_sound();
        self.count_spin(self.tow.held_angle(moment));
        if std::mem::take(&mut self.pod.fire) {
            self.fire(&world.objects);
        }
        for frame in tick.frames() {
            self.apply_cheats();
            self.pass(world, frame);
            if self.routed() {
                // The camera as the frame leaves it, at the frame's end.
                let time = (frame + 1) * (clock::STEP / 2);
                self.camera_trail.push((time, self.camera.position));
                if self.camera_trail.len() > 6 {
                    self.camera_trail.remove(0);
                }
            }
            if self.transition.is_some() || self.paused {
                break;
            }
        }
        if self.transition.is_none() && !self.paused {
            self.touch(world, tick.end);
        }
        let frozen = matches!(self.pod.state, STATE_POD_EXPLODING | STATE_ASSEMBLING);
        if !self.routed() && !frozen {
            self.camera
                .tick(level, before, (self.pod.body.x, self.pod.body.y), f);
        }
        if !level.is_warp_level() {
            let moved = (
                i32::from(self.camera.position.x.wrapping_sub(camera_before.x) as i16),
                i32::from(self.camera.position.y.wrapping_sub(camera_before.y) as i16),
            );
            self.stars.drift(moved, self.pod.in_sky);
        }
        self.attach_to_pod();
        self.apply_cheats();
        self.show(level.is_warp_level());
    }

    /// The pod's gun (player.md 10): the fitted weapon's shot with Select, else the plain
    /// bullet, which sounds on pulse 2.
    fn fire(&mut self, data: &objdata::ObjectData) {
        let before = places(&self.objects);
        if self.objects.nose_shot(&self.pod).is_some() {
            // Player.md 10: the gun's sound, started at once on pulse 2, whatever the
            // weapon. With no slot for the shot there is none (Mesen: eight shots in the
            // air, the press made neither shot nor sound).
            self.sfx.push(Request::Direct {
                channel: Channel::Pulse2,
                id: 0x06,
            });
        }
        if !self.fire_special_weapon(data) {
            let strength = self.pod.equipment.bullet_upgrade + 1;
            self.objects.fire_pod_shot(data, &self.pod, strength);
        }
        // A new shot sits at the nose until the next step moves it.
        for (slot, place) in places(&self.objects).into_iter().enumerate() {
            if place != before[slot]
                && let Some(from) = self.step_from.get_mut(slot)
            {
                *from = place;
            }
        }
    }

    /// Where the pod is drawn: where it is, or on its way over the step while the step's logic
    /// moves it.
    fn pod_shown(&self) -> (f32, f32) {
        let now = pod_position(&self.pod);
        let Some(from) = self.pod_from.filter(|_| self.pod.state == pod::STATE_GATE) else {
            return now;
        };
        let elapsed = self.clock.time().saturating_sub(self.step_at);
        let phase = (elapsed.min(clock::STEP) >> 16) as f32 / 65536.0;
        let from = (from.0 as f32 / 65536.0, from.1 as f32 / 65536.0);
        (
            from.0 + (now.0 - from.0) * phase,
            from.1 + (now.1 - from.1) * phase,
        )
    }

    /// The camera at game time `time`, between the frames that moved it.
    fn camera_at_time(&self, time: u64) -> Point {
        let trail = &self.camera_trail;
        let Some(i) = trail.iter().position(|&(t, _)| t > time) else {
            return trail.last().map_or(self.camera.position, |&(_, at)| at);
        };
        if i == 0 {
            return trail[0].1;
        }
        let ((t0, a), (t1, b)) = (trail[i - 1], trail[i]);
        let along = |a: u16, b: u16| {
            let d = i64::from(b.wrapping_sub(a) as i16);
            let part = d * (time - t0) as i64 / (t1 - t0).max(1) as i64;
            a.wrapping_add_signed(part as i16)
        };
        Point {
            x: along(a.x, b.x),
            y: along(a.y, b.y),
        }
    }

    /// Whether `slot` hangs on the pod, which `attach_to_pod` keeps it at after every tick: the
    /// flame, the shield ring or the beam, and the towed item.
    fn on_pod(&self, slot: usize) -> bool {
        let o = &self.objects.slots[slot];
        match slot {
            1 => {
                self.pod.state == pod::STATE_POD
                    && o.state == objects::state::MOTHERSHIP
                    && self.pod.flame_picture().is_some()
            }
            2 => {
                o.state == objects::state::BEAM
                    || (o.state == objects::state::SHIELD_RING && self.pod.state == pod::STATE_POD)
            }
            _ => self.tow.holding() && slot == self.tow.slot && o.state != 0 && o.work6 == 0,
        }
    }

    /// The objects' places at game time `now`, on their way over the step; what hangs on the
    /// pod is where it is, with the pod.
    fn places_at(&self, now: u64) -> Vec<Option<(u32, u32)>> {
        let elapsed = now.saturating_sub(self.step_at);
        let phase = (elapsed.min(clock::STEP) >> 16) as u32;
        places(&self.objects)
            .into_iter()
            .enumerate()
            .map(|(slot, to)| {
                let to = to?;
                if self.on_pod(slot) {
                    return Some((to.x, to.y));
                }
                let from = self.step_from.get(slot).copied().flatten();
                Some(
                    from.and_then(|from| between(from, to, phase))
                        .unwrap_or((to.x, to.y)),
                )
            })
            .collect()
    }

    /// The pod and its shots against the objects at the end of a tick (behaviour.md 3), with
    /// the objects where they are shown then. A push moves an object's step place along.
    fn touch(&mut self, world: &World, now: u64) {
        let shown = self.places_at(now);
        let ahead = places(&self.objects);
        for (o, at) in self.objects.slots.iter_mut().zip(&shown) {
            if let Some((x, y)) = *at {
                (o.x, o.x_fraction) = ((x >> 8) as u16, x as u8);
                (o.y, o.y_fraction) = ((y >> 8) as u16, y as u8);
            }
        }
        collide::collide(
            &mut self.objects,
            &mut self.pod,
            &world.objects,
            &self.rng,
            &mut self.score,
            &mut self.bonus_tag,
            &mut self.touch_ready,
            now,
        );
        self.collect_sound();
        for (slot, o) in self.objects.slots.iter_mut().enumerate() {
            let (Some((x, y)), Some(before)) = (shown[slot], ahead[slot]) else {
                continue;
            };
            if (o.state, o.index) != (before.state, before.index) {
                // Destroyed: its explosion stays where it was hit.
                continue;
            }
            let moved = |now: u16, fraction: u8, shown: u32| {
                (u32::from(now) << 8 | u32::from(fraction)).wrapping_sub(shown)
            };
            let x = before.x.wrapping_add(moved(o.x, o.x_fraction, x)) & 0xFF_FFFF;
            let y = before.y.wrapping_add(moved(o.y, o.y_fraction, y)) & 0xFF_FFFF;
            (o.x, o.x_fraction) = ((x >> 8) as u16, x as u8);
            (o.y, o.y_fraction) = ((y >> 8) as u16, y as u8);
        }
    }

    /// Records what the last tick shows, for drawing between ticks.
    fn show(&mut self, warp_level: bool) {
        self.shown_before = self.shown_now;
        self.shown_now = View {
            camera: (
                f32::from(self.camera.position.x),
                f32::from(self.camera.position.y),
            ),
            pod: self.pod_shown(),
            objects: {
                let mut views = object_views(&self.objects);
                for (view, at) in views.iter_mut().zip(self.places_at(self.clock.time())) {
                    if let (Some(view), Some((x, y))) = (view.as_mut(), at) {
                        (view.0, view.1) = (x as f32 / 256.0, y as f32 / 256.0);
                    }
                }
                views
            },
        };
        // The stars flash on the frame clock (graphics.md 5.7.3, "Drawing").
        let phase = (self.counter | 1) as u8;
        self.star_sprites = if warp_level {
            Vec::new()
        } else {
            self.stars
                .drawn()
                .map(|(i, star)| {
                    let flash = matches!((32 * i as u8).wrapping_add(phase), 1 | 5);
                    (star.x, star.y, if flash { 1 } else { 3 })
                })
                .collect()
        };
    }

    /// The port menu's pod rest (decision 8): an upright pod landing on a flat floor slower
    /// than `below` (Q8.24 px per step) comes to rest; `None` is off, the Original's hop.
    pub fn set_pod_rest(&mut self, below: Option<u32>) {
        self.pod.rest_below = below;
    }

    /// Sets the view's width (`docs/widescreen.md`; the original's 256 by default; replays
    /// and comparisons never change it). The rules that measure against the screen follow it.
    pub fn set_view_width(&mut self, width: u16) {
        self.view_width = crate::game::view::clamp_width(width);
    }

    /// How far the view reaches beyond the camera's screen, left and right.
    fn route_margins(&self, level: &crate::game::level::Level) -> (i32, i32) {
        let view = self.viewport(level);
        let left = view.shift(self.camera.position);
        (left, view.w() - 256 - left)
    }

    /// The view around the camera, for the rules that measure against the screen.
    fn viewport(&self, level: &crate::game::level::Level) -> Viewport {
        Viewport::around(level, self.camera.position, self.view_width)
    }

    /// Sets the cheats (all off by default; replays and comparisons never set them).
    pub fn set_cheats(&mut self, cheats: Cheats) {
        self.cheats = cheats;
        self.apply_cheats();
    }

    /// Tops up what the cheats keep full, and tells the pod whether damage takes fuel.
    fn apply_cheats(&mut self) {
        let cheats = self.cheats;
        self.pod.free_fuel = cheats.infinite_fuel;
        if cheats.infinite_fuel {
            self.pod.fuel = pod::FULL_FUEL;
        }
        if cheats.full_gauge {
            (self.pod.weapon_energy, self.pod.weapon_low) = (0xE0, 0);
        }
        if cheats.infinite_lives && self.lives == 0 {
            self.lives = 1;
        }
    }

    /// One frame of the NES's clock: the logic of an A pass (the player's step, the objects)
    /// where the frame counter is even, of a B pass where it is odd. The player's motion, the
    /// camera and the stars are the ticks'.
    fn pass(&mut self, world: &World, frame: u64) {
        let level = &world.levels[self.level];
        let a_pass = self.next_is_a_pass();

        if a_pass {
            self.beam_caught = false;
        }
        // The rotating job counter advances at the start of the frame's handler, before the
        // pass; the first pass of a level sees 1 (measured), and the pod's step may set it (to 9
        // when Jetman emerges, player.md 7.1) for the split that follows.
        self.job = (self.job + 1) & 7;
        // The original uploads the palette on job 0 only (game-flow.md 1.2); the remake shows
        // the terrain colours and the gate's fades at once (docs/deviations.md, "Display
        // refresh").
        self.terrain.shown = self.terrain.shadow;
        if let Some(gate) = self.gate.as_mut() {
            gate.fades_shown = gate.fades;
        }
        self.stars.spawn(self.counter);
        if a_pass {
            self.held = self.controls.held;
            self.pressed = self.controls.take_step_presses();
            let ctx = Context {
                world,
                level,
                frame: self.counter as u8,
                now: self.clock.time(),
                moment: Moment::STEP,
            };
            let held = self.held;
            if self.tow.holding() {
                let item = &mut self.objects.slots[self.tow.slot];
                self.tow.taken_in = item.work6 != 0;
                if item.work6 == 0 {
                    let ((speed_x, speed_y), (left, down)) = self.tow.item_motion();
                    item.speed_x = speed_x;
                    item.speed_y = speed_y;
                    item.flags_x = u8::from(left);
                    item.flags_y = u8::from(down);
                }
                // Sound.md 8.3: the rope's hum, every fourth frame while the item is not being
                // taken in; its pitch follows the rope's angle.
                if self.counter & 3 == 0 && !self.tow.taken_in {
                    let angle = self.tow.angle8();
                    let p = if angle < 0x40 { angle } else { 0x80 - angle };
                    self.sfx.push(Request::RamPitch(p.wrapping_mul(2)));
                    self.sfx.push(Request::a(Channel::Triangle, 0x1A));
                }
            }
            self.pod.route_camera = self.camera.position;
            self.pod.route_margins = self.route_margins(level);
            self.pod.route_end = self.route.done;
            self.pod.route_my = self.route.my;
            self.pod.step(&ctx, held, &mut self.tow);
            self.collect_sound();
            if self.pod.state == pod::STATE_GATE {
                self.gate_step(level);
                // The drawing stage comes after the state's flash (game-flow.md 9.2, "Bits 0-1").
                self.pod.hit_pause_countdown();
            }
            self.route.my = self.pod.route_my;
            self.after_player();
            // The drawing stage eases the F gauge in every player state, the gate's too
            // (economy.md 3.1).
            self.pod.ease_fuel_display();
            let data = &world.objects;
            // Impact sparks (player.md 5.5) and the pod's shot (10) are objects in slots 3-10.
            let requests = std::mem::take(&mut self.pod.spark_requests);
            self.objects.make_sparks(data, &mut self.rng, &requests);
            self.step_from = places(&self.objects);
            self.step_at = frame * (clock::STEP / 2);
            self.pod_from = (self.pod.state == pod::STATE_GATE)
                .then(|| (self.pod.body.x_q16(), self.pod.body.y_q16()));
            let ahead = {
                let left = (self.step_at + clock::STEP).saturating_sub(self.clock.time());
                let f = (left >> 16) as u32;
                let (h, v) = (
                    self.pod.body.h_axis().signed(),
                    self.pod.body.v_axis().signed(),
                );
                (motion::displacement(h, h, f), motion::displacement(v, v, f))
            };
            // Slot 2's behaviour, the second of the sweep (slot 1, the flame, does nothing).
            self.place_beam();
            // Every object runs its behaviour (behaviour.md 1).
            let view = self.viewport(level);
            let mut env = Env {
                data,
                level,
                camera: self.camera.position,
                view,
                pod: &mut self.pod,
                rng: &mut self.rng,
                counter: self.counter,
                score: &mut self.score,
                bonus_tag: self.bonus_tag,
                beam_spin: self.tow.holding().then_some(self.tow.spin),
                beam_slot: self.tow.slot,
            };
            // The step's logic moves the objects to their places at the step's end, against
            // the pod a step ahead (`docs/scaled.md`); drawing moves them there.
            env.pod.body.shift(ahead.0, ahead.1, false, 0);
            self.objects.sweep(&mut env);
            self.pod.body.shift(-ahead.0, -ahead.1, false, 0);
            self.collect_sound();
            if std::mem::take(&mut self.objects.countdown_done) {
                self.request(Transition::WarpEnd);
            }
            let arrived = std::mem::take(&mut self.objects.arrivals);
            if arrived > 0 {
                // Respawn particles reached the pod (player.md 7.3).
                self.pod.timer = self.pod.timer.wrapping_sub(arrived);
                self.pod.facing = 0;
                self.pod.materialised = true;
            }
            if self.routed() {
                self.route_a_pass(held);
            }
        } else if self.routed() {
            self.route.move_camera(&mut self.camera.position);
        }
        if !a_pass {
            self.pod.update_level_gravity(level.number);
            self.level_routine(world, level);
            // B pass step 2: objects that left the window are freed (not in the gate state:
            // its particles fly out of the window and stay, player.md 9.2.3).
            if self.pod.state != pod::STATE_GATE {
                self.objects.despawn(level, self.viewport(level));
            }
        }
        // The status-bar split: the generator steps, then the frame counter (game-flow.md 5).
        let counter = self.counter.to_le_bytes();
        let pressed = self.controls.take_frame_presses();
        self.rng.step([
            self.controls.held.0,
            pressed.0,
            0,
            0,
            counter[0],
            counter[1],
            counter[2],
            self.job,
        ]);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        // The warp level's routine runs on every frame, after the counter's increment
        // (levels.md 6.7).
        self.warp_routine(world, level);
        // A pass item 8: Start pauses, if the player state allows it. The rest of the pass is
        // not done, then or later (game-flow.md 4).
        let pausing =
            a_pass && self.pressed.has(Buttons::START) && matches!(self.pod.state, 1..=3 | 6);
        if pausing {
            self.paused = true;
            self.pause_held = self.held;
        }
        if a_pass && !pausing {
            // After the A pass's split: the green-orb spawner, the items scan (levels.md 3.3)
            // and the collision checks (behaviour.md 3).
            let data = &world.objects;
            let pod_state = self.pod.state;
            let view = self.viewport(level);
            let mut env = Env {
                data,
                level,
                camera: self.camera.position,
                view,
                pod: &mut self.pod,
                rng: &mut self.rng,
                counter: self.counter,
                score: &mut self.score,
                bonus_tag: self.bonus_tag,
                beam_spin: self.tow.holding().then_some(self.tow.spin),
                beam_slot: self.tow.slot,
            };
            // The per-level routine's colours (levels.md 6.6): the pass began with $5D a
            // multiple of 32.
            if pod_state != 9 && self.counter & 0x1F == 1 {
                self.terrain
                    .by_depth(level.number, self.camera.position.y, false);
            }
            if pod_state != 9 {
                self.objects.spawn_orb(&mut env, self.counter);
                self.objects.item_scan(data, view);
            }
            // Item 9: the transition guard counts down.
            self.guard = self.guard.saturating_sub(1);
            // Touching an object has an impact effect too (behaviour.md 3.1); the touches are
            // the ticks' (`touch`).
            let requests = std::mem::take(&mut self.pod.spark_requests);
            self.objects.make_sparks(data, &mut self.rng, &requests);
            // Item 11: the tractor beam and the player against the items.
            self.beam(world, self.held);
            self.collect_sound();
            // The bonus-message routine: a pickup's tag gets bit 5 the step it is set, and
            // bits 5 and 6 are gone the step after (economy.md 2.5).
            // A new tag (bit 6) has the sign's picture written for its nibble (graphics.md
            // 3.1); the marker stays invisible meanwhile, so it is shown at once.
            if self.bonus_tag & 0x40 != 0 {
                self.sign_picture = Some(self.bonus_tag & 0x0F);
            }
            if self.bonus_tag & 0x20 != 0 {
                self.bonus_tag &= !0x60;
            } else if self.bonus_tag & 0x40 != 0 {
                self.bonus_tag |= 0x20;
            }
        } else if self.pod.state != 9 {
            // B pass step 5: the placement scan.
            self.objects
                .step_scan(&world.objects, level, self.viewport(level));
        }
    }

    /// The camera and the pod's position (the top-left of the playfield and the object point)
    /// at `blend` between the last two passes (0 the earlier, 1 the later), in whole pixels: the
    /// whole part of a position, as the NES draws it. The pod is placed by its blended offset
    /// from the blended camera, so that between passes its place on the screen moves one way
    /// only (each rounded on its own, the two would put it a pixel back and forth).
    pub fn view_at(&self, blend: f32) -> (Point, (i32, i32)) {
        let (camera, mixed) = self.camera_at(blend);
        let blend = blend.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * blend;
        let (before, now) = (&self.shown_before, &self.shown_now);
        let pod = (
            i32::from(camera.x) + (mix(before.pod.0, now.pod.0) - mixed.0).floor() as i32,
            i32::from(camera.y) + (mix(before.pod.1, now.pod.1) - mixed.1).floor() as i32,
        );
        (camera, pod)
    }

    /// The camera at `blend` in whole pixels, and unrounded.
    fn camera_at(&self, blend: f32) -> (Point, (f32, f32)) {
        let blend = blend.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * blend;
        let (before, now) = (&self.shown_before, &self.shown_now);
        let mixed = (
            mix(before.camera.0, now.camera.0).max(0.0),
            mix(before.camera.1, now.camera.1).max(0.0),
        );
        let camera = Point {
            x: mixed.0.floor() as u16,
            y: mixed.1.floor() as u16,
        };
        (camera, mixed)
    }

    /// Draws the playfield, the objects and the pod at `blend` between the last two passes.
    /// Objects are placed relative to the camera; one that was in its slot in both passes is
    /// blended between its two positions, whole pixels as the NES draws them.
    pub fn draw(
        &self,
        frame: &mut Frame,
        world: &World,
        graphics: &LevelGraphics,
        sprites: &Sprites,
        layouts: &SpriteFile,
        blend: f32,
    ) {
        let (camera, pod) = self.view_at(blend);
        // The warp level's mesh tile drifts and the gate state darkens the background palettes
        // 2 and 3 (graphics.md 5.7.2, player.md 9.2.3): drawn from a changed copy.
        let changed;
        let graphics = if self.level >= crate::game::level::FIRST_WARP_LEVEL
            || self.gate.is_some()
            || self.terrain.shown.is_some()
        {
            changed = self.changed_graphics(graphics);
            &changed
        } else {
            graphics
        };
        let blend = blend.clamp(0.0, 1.0);
        let screen_camera = (i32::from(camera.x), i32::from(camera.y));
        let (_, mixed_camera) = self.camera_at(blend);
        // Jetman is drawn with his own figure in the pod's first tiles, in a wormhole too.
        let jetman = matches!(self.pod.state, 2 | 3 | 5)
            || (self.pod.state == pod::STATE_GATE
                && self
                    .gate
                    .as_ref()
                    .is_some_and(|gate| matches!(gate.previous(), 2 | 3)));
        let sign = self.sign_picture.and_then(|n| {
            let n = usize::from(n);
            graphics.sign_tiles.get(32 * n..32 * n + 32)
        });
        let sprites =
            crate::game::picture::dress_sprites(sprites, jetman, self.beam_image(graphics), sign);
        // Slot 1 is in front of slot 30: draw from the back.
        let level = &world.levels[self.level];
        let logic_view = self.viewport(level);
        let mut drawn = Vec::new();
        for slot in (1..objects::SLOTS).rev() {
            let object = &self.objects.slots[slot];
            if !ObjectTable::is_drawn(object, logic_view) {
                continue;
            }
            let mut object = *object;
            if slot <= 2 && object.state == objects::state::BEAM && self.beam_caught {
                object.frame = 0x38;
            }
            // A moving object is placed by its blended offset from the blended camera, as the
            // pod is (`view_at`); a still one by its own position, so it stays on the ground.
            // Positions are 16-bit and wrap round (a star above the level's top edge has y
            // $FFxx, player.md 9.2.3): the offset is taken as a signed 16-bit number, and a
            // move across the wrap is not blended.
            let wrap = |offset: i32| i32::from(offset as u16 as i16);
            let axis = |before: f32, now: f32, camera: f32, screen: i32| {
                if before == now || (now - before).abs() >= 32768.0 {
                    wrap(now.floor() as i32 - screen)
                } else {
                    wrap((before + (now - before) * blend - camera).floor() as i32)
                }
            };
            let at = match (
                self.shown_before.objects[slot],
                self.shown_now.objects[slot],
            ) {
                (Some(before), Some(now)) if before.2 == now.2 && before.3 == now.3 => (
                    axis(before.0, now.0, mixed_camera.0, screen_camera.0),
                    axis(before.1, now.1, mixed_camera.1, screen_camera.1),
                ),
                (_, Some(now)) => (
                    wrap(now.0.floor() as i32 - screen_camera.0),
                    wrap(now.1.floor() as i32 - screen_camera.1),
                ),
                _ => continue,
            };
            drawn.push((slot, object, at));
        }
        crate::game::picture::Picture {
            level: self.level,
            graphics,
            sprites: &sprites,
            camera,
            view: Viewport::around(level, camera, frame.width as u16),
            objects: drawn,
            behind: self.pod.bay,
            stars: &self.star_sprites,
            hud: self.hud(),
            panel_icon: self.panel_icon,
            pod: crate::game::picture::PodPicture {
                facing: self.pod.facing,
                // Frames $20 and up are mirrored by the facing bit: RAM $0336 bit 1 when bit 2
                // is set, else whether it is not 0 (objects.md 6.4).
                mirrored: {
                    let fx = self.pod.direction_flags();
                    if fx & 4 != 0 { fx & 2 != 0 } else { fx != 0 }
                },
                hit_pause: self.pod.hit_pause,
                palette_bits: self.pod.palette_bits,
                at: (pod.0 - screen_camera.0, pod.1 - screen_camera.1),
            },
        }
        .draw(frame, world, layouts);
    }

    /// What the status panel shows.
    pub fn hud(&self) -> crate::game::hud::Hud {
        crate::game::hud::Hud {
            fuel: self.pod.fuel_shown,
            weapon: self.pod.weapon_energy,
            jetpods: self.jetpods,
            lives: self.lives,
            score: self.score.0,
            negative: false,
        }
    }

    /// The object slots 3-30 as the replay comparison checks them: for every slot in use,
    /// (field letter, slot, value) with the letters s state, x, y, f frame, h hit points and
    /// i list index (`tools/mesen/replay-capture.lua`). Slots 1-2 (the pod's flame) are not
    /// simulated; hit points of the item slots 11-14 are not defined (objects.md 10.1).
    pub fn object_fields(&self) -> Vec<(char, usize, u32)> {
        let mut fields = Vec::new();
        for slot in 3..objects::SLOTS {
            let o = &self.objects.slots[slot];
            if o.state == 0 {
                continue;
            }
            fields.push(('s', slot, o.state.into()));
            fields.push(('x', slot, o.x.into()));
            fields.push(('y', slot, o.y.into()));
            fields.push(('f', slot, o.frame.into()));
            if !objects::ITEM_SLOTS.contains(&slot) {
                fields.push(('h', slot, o.hit_points.into()));
            }
            fields.push(('i', slot, o.index.into()));
        }
        fields
    }

    /// The slots attached to the pod, 1 (the flame) and 2 (the beam or the shield ring), as
    /// the replay comparison checks them apart: (field letter, slot, value) with the letters s
    /// state, x, y, f frame and d the direction flags X and Y as one 16-bit value.
    pub fn attached_fields(&self) -> Vec<(char, usize, u32)> {
        let mut fields = Vec::new();
        for slot in 1..=2 {
            let o = &self.objects.slots[slot];
            if o.state == 0 {
                continue;
            }
            fields.push(('s', slot, o.state.into()));
            fields.push(('x', slot, o.x.into()));
            fields.push(('y', slot, o.y.into()));
            fields.push(('f', slot, o.frame.into()));
            fields.push(('d', slot, u32::from(o.flags_x) << 8 | u32::from(o.flags_y)));
        }
        fields
    }
}
