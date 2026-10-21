//! Gameplay: the frame, pass and logic-step schedule, the controller, the random generator, the
//! pod, the terrain and the camera (`docs/spec/game-flow.md` 1-5, `docs/spec/player.md`,
//! `docs/spec/levels.md` 5-6).
//!
//! The simulation runs on the NES's frame clock: [`Gameplay::pass`] runs one frame's pass, an
//! A pass (input, pod, first half of the camera move) on frames where the frame counter is
//! even and a B pass (second half) where it is odd. [`Gameplay::step`] runs both, one logic
//! step. Drawing is separate ([`Gameplay::draw`]) and blends the last two passes.
//!
//! The objects (`docs/spec/objects.md`, `docs/spec/behaviour.md`) are in [`objects`] (the table,
//! placement, despawning, explosions, the items scan), [`behave`] (the sweep and the shared
//! helpers, turrets, generators, mines, orbs), [`enemies`] (the placed objects of levels 1-7),
//! [`minions`] (what they make: aimed shots, missiles, flames, exhaust, drones, hoppers, heads)
//! and [`collide`] (the pod and its shots against them). The blue sphere, the ground-gun pairs
//! of level 8, the level 26 objects and the companion in the pod's service are not
//! implemented (spec gaps in `behaviour.md` 4.15, 4.16, 4.18). The player is the [`pod::Pod`] (the pod, [`jetman`] outside it); [`tow`]
//! is the tractor beam and [`economy`] the deliveries, docking, lives and the scene changes
//! gameplay asks for; [`pod::Body`] is the shared position and speed type.

pub mod autopilot;
pub mod behave;
pub mod camera;
pub mod collide;
pub mod economy;
pub mod enemies;
pub mod final_level;
pub mod gate;
pub mod input;
pub mod inventory;
pub mod jetman;
pub mod minions;
pub mod objdata;
pub mod objects;
pub mod pod;
pub mod rng;
pub mod sprites;
pub mod stars;
pub mod tables;
pub mod terrain;
pub mod tow;
pub mod warp;
pub mod weapons;

#[cfg(test)]
mod tests;

use crate::game::graphics::LevelGraphics;
use crate::game::level::{Point, World};
use crate::game::playfield::Frame;
use crate::game::sound::{Channel, Request};
use camera::Camera;
use economy::Transition;
use input::Buttons;
use objects::{Env, LevelMemory, ObjectTable, Score};
use pod::{Context, Pod, STATE_ASSEMBLING, STATE_POD_EXPLODING};
use rng::Rng;
use sprites::{SpriteFile, Sprites};
use stars::Stars;
use std::sync::Arc;
use tow::Tow;

/// State a level start inherits from before it: the random generator and the frame counter
/// ($5D-$5F count loading frames, which the remake does not have, `docs/deviations.md`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Seed {
    pub rng: Rng,
    pub counter: u32,
    pub score: Score,
    /// The memory of destroyed objects of the levels left before (objects.md 3.6).
    pub memory: LevelMemory,
    /// The parallax-star job's mode RAM $D5, which survives level changes (graphics.md 5.7.3).
    pub star_mode: u8,
    /// The parallax stars as the level before left them (their mode, dead mask, x and
    /// fractions survive a level load; graphics.md 5.7.3); `None` at the start of a game, when
    /// every star is dead and [`Seed::star_mode`] gives the mode.
    #[serde(default)]
    pub stars: Option<Stars>,
    /// Jetman lives RAM $3E and jetpods RAM $3D (economy.md 4).
    pub lives: u8,
    pub jetpods: u8,
    /// Crystals stored RAM $E0 (economy.md 2.4).
    pub crystals: u8,
    /// The pod's equipment RAM $DC (game-flow.md 9).
    pub equipment: u8,
    /// The fitted weapon RAM $D1 (the attract demos fit one, game-flow.md 7.1).
    pub weapon: u8,
    /// The fitted pod device RAM $D3 and the bullet upgrade RAM $050E (economy.md 6.4).
    pub device: u8,
    pub bullet_upgrade: u8,
    /// The weapon gauge RAM $C9:$C8 at the start; `None` is full.
    pub weapon_gauge: Option<u16>,
    /// The mapping device RAM $060C (economy.md 2.3) and the planets whose warpship piece was
    /// delivered, one bit each (economy.md 5.1).
    pub mapping_device: bool,
    /// The super mapping device RAM $060D (economy.md 6.5), bought only; the game carries it.
    pub super_mapping_device: bool,
    pub planet_flags: u16,
    /// What the object table's flags Y and frame bytes hold before the level fills it, per slot
    /// (the previous scene's leftovers, which the lift of a towed load reads from free slots,
    /// player.md 8.3 b); empty: all 0.
    pub leftovers: Vec<(u8, u8)>,
    /// What the pod's memory held from an earlier session, which a new level start does not
    /// clear (the attract demos start from the previous demo's).
    pub pod_leftovers: PodLeftovers,
    /// Every object slot as the level before left it, state aside: a level entered by a
    /// level change (code 3) finds the bytes of the slots' last occupants (empty: all 0).
    pub stale: Vec<objects::Object>,
    /// The level is entered through a wormhole (code 3, game-flow.md 7.4): what the player
    /// brings along.
    pub arrival: Option<Arrival>,
    /// The mothership's fuel count RAM $0636 saved on leaving a planet by a wormhole.
    pub saved_fuel_count: u8,
    /// The final level entered from the last warpship piece (game-flow.md 7.4): the pod flies
    /// up at 1.50 from the home point with a full tank, under the header's camera.
    pub final_entry: bool,
}

/// What a level entered by a level change (code 3) keeps of the player (game-flow.md 7.4,
/// "Where the player appears").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Arrival {
    /// The player state (Jetman alone arrives as Jetman), the fuel and the two direction bits
    /// and the movement angle the previous level left.
    pub state: u8,
    pub fuel: u16,
    /// The fuel gauge's display copy RAM $05CE: only a launch from the mothership empties it
    /// (economy.md 3.1).
    pub fuel_shown: u8,
    pub h_left: bool,
    pub v_down: bool,
    pub move_angle: u8,
    /// The player's work bytes RAM $0393 and $03B2, which a new level does not clear.
    pub timer: u8,
    pub counter: u8,
    /// The facing, and the beam's rope bytes RAM $68-$6D, which stay as they were.
    pub facing: u8,
    pub rope: (u16, u32, bool),
    /// RAM $97: 64 after a gate that ended directly in the change, else 0.
    pub invulnerable: u8,
    /// A planet reached from a second or warp level: the player and the camera are 128 px
    /// above the mothership.
    pub lowered: bool,
    /// The mothership's fuel count to put back on a planet.
    pub fuel_count: Option<u8>,
}

/// The pod's bytes that survive a level start: the movement angle RAM $9C, the invulnerability
/// timer RAM $97, the two speed accumulators and the thrust carry-in byte RAM $0D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PodLeftovers {
    pub move_angle: u8,
    pub invulnerable: u8,
    pub accumulator_h: u8,
    pub accumulator_v: u8,
    pub camera_carry: u8,
    /// The tow torque's spin byte RAM $6E, which the tow's other bytes do not outlast.
    pub tow_spin: u8,
}

impl Default for Seed {
    fn default() -> Seed {
        Seed {
            rng: Rng::default(),
            counter: 0,
            score: Score::NEW_GAME,
            memory: LevelMemory::default(),
            star_mode: 0,
            stars: None,
            lives: 4,
            jetpods: 4,
            crystals: 0,
            equipment: 0,
            weapon: 0,
            device: 0,
            bullet_upgrade: 0,
            weapon_gauge: None,
            mapping_device: false,
            super_mapping_device: false,
            planet_flags: 0,
            leftovers: cold_boot_leftovers(),
            pod_leftovers: PodLeftovers::default(),
            stale: Vec::new(),
            arrival: None,
            saved_fuel_count: 0,
            final_entry: false,
        }
    }
}

/// The object table's flags Y and frame bytes as a cold-booted game leaves them at the start of
/// level 0, for the slots that are still free after the first steps (measured in the original).
fn cold_boot_leftovers() -> Vec<(u8, u8)> {
    let mut slots = vec![(0, 0); objects::SLOTS];
    slots[1].0 = 1;
    slots[4].1 = 0x11;
    slots[12].1 = 0xCA;
    slots[13].1 = 0xC5;
    slots[14].1 = 0xC1;
    slots
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

/// The port menu's cheats: all off by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cheats {
    /// Losing Jetman costs no life.
    pub infinite_lives: bool,
    /// The tank stays full: hits and crashes take no fuel, though they still push, spark and
    /// sound.
    pub infinite_fuel: bool,
    /// The weapon gauge stays full.
    pub full_gauge: bool,
}

/// Gameplay on one level.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Gameplay {
    level: usize,
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
    /// Whether this B pass's star job wrote the thrust carry-in byte (player.md 4.6).
    star_wrote_carry: bool,
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
    /// Validation mode, see [`Gameplay::set_original_rules`].
    original_rules: bool,
    /// Bytes only the validation mode reads, read from the user's ROM at run time.
    #[serde(skip)]
    validation: Option<ValidationInput>,
    /// Bytes of the warp mesh's variant table (graphics.md 5.7.2) that a replay's test setup
    /// zeroes in the original, beyond the hole the validation mode keeps.
    #[serde(skip)]
    pub mesh_holes: Vec<usize>,
    /// The port menu's cheats, set by the program (not part of a saved state).
    #[serde(skip)]
    cheats: Cheats,
    /// The attract demo's autopilot, when this is a demo (`autopilot`).
    demo: Option<autopilot::Demo>,
    /// Paused: Start was seen in an A pass (game-flow.md 4); `pause_frame` runs the frames until
    /// the next Start.
    paused: bool,
    /// The sound requests made since the caller took them ([`Gameplay::take_sound`]).
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

    /// Where the pod was drawn after the previous pass and after the last one.
    shown_before: View,
    shown_now: View,
    /// Where the pod stood before this step's A pass and after it (for the half-way point).
    step_start: (f32, f32),
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

/// Puts each object in `now` half-way along its move from `before` (where the last B pass left
/// it): one that is the same object in both and moved less than 16 pixels on each axis. A jump
/// (a spawn point, a teleport, a move across the 16-bit wrap) is shown where it landed.
fn halfway_objects(before: &[Option<(f32, f32, u8, u8)>], now: &mut [Option<(f32, f32, u8, u8)>]) {
    for (before, now) in before.iter().zip(now.iter_mut()) {
        if let (Some(b), Some(n)) = (before, now.as_mut())
            && (b.2, b.3) == (n.2, n.3)
            && (n.0 - b.0).abs() < 16.0
            && (n.1 - b.1).abs() < 16.0
        {
            n.0 = (b.0 + n.0) / 2.0;
            n.1 = (b.1 + n.1) / 2.0;
        }
    }
}

fn pod_position(pod: &Pod) -> (f32, f32) {
    (
        f32::from(pod.body.x) + f32::from(pod.body.x_fraction) / 256.0,
        f32::from(pod.body.y) + f32::from(pod.body.y_fraction) / 256.0,
    )
}

impl Gameplay {
    /// Starts gameplay on `level`: the pod at home, the camera placed from it.
    pub fn new(world: &World, level: usize, seed: Seed) -> Gameplay {
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
        pod.accumulator_h = left.accumulator_h;
        pod.accumulator_v = left.accumulator_v;
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
            (pod.body.h_speed, pod.body.v_speed) = (0, 0x0040);
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
            (pod.body.h_speed, pod.body.v_speed) = (0, 0x0180);
            (pod.body.h_left, pod.body.v_down) = (false, false);
            (pod.fuel, pod.invulnerable, pod.shield) = (pod::FULL_FUEL, 0, false);
        }
        // The pod starts inside the mothership's bay area (economy.md 2.2).
        pod.bay = usize::from(info.number) < crate::game::level::PLANETS
            && economy::in_bay_area(&pod, info);
        let camera = if seed.final_entry {
            Camera {
                position: info.camera_start,
                delta: (0, 0),
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
        seed.memory.restore(level, &mut objects);
        if let Some(count) = seed.arrival.and_then(|a| a.fuel_count)
            && let Some(entry) = objects.items.first_mut()
        {
            entry.kind = count;
        }
        objects.start_scan(&world.objects, info, camera.position);
        // One call of the items scan is part of the level start: the first item is in its slot
        // before the first step (levels.md 3.3; measured). Not on a warp level, where the first
        // crystal comes with the first step.
        if !info.is_warp_level() {
            objects.item_scan(&world.objects, camera.position);
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
        let mut game = Gameplay {
            level,
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
                angle: seed.arrival.map_or(0, |a| a.rope.0),
                speed: seed.arrival.map_or(0, |a| a.rope.1),
                sense: seed.arrival.is_some_and(|a| a.rope.2),
                ..Tow::default()
            },
            original_rules: false,
            mesh_holes: Vec::new(),
            validation: None,
            cheats: Cheats::default(),
            demo: None,
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
            memory: seed.memory,
            stars: seed.stars.clone().map_or_else(
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
            star_wrote_carry: false,
            shown_before: view,
            shown_now: view,
            step_start: view.pod,
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

    /// Validation mode, for replay comparisons only (not a port-menu switch). On, every
    /// deviation that changes the game state (`docs/deviations.md`) is switched off and the
    /// original's rule runs instead: the tow lift's sign, the push on a protected pod, the
    /// waypoint test of patrolling creatures. Off (the default) is the game as played.
    pub fn set_original_rules(&mut self, on: bool) {
        self.original_rules = on;
    }

    /// Gives the validation mode the bytes it needs from outside the committed data (never set
    /// in play; `ValidationInput`).
    pub fn set_validation_input(&mut self, input: Option<ValidationInput>) {
        self.validation = input;
    }

    /// Makes this the original's attract demo, as `replay-compare --demos` replays it (the
    /// remake's own demos are flown by `crate::game::pilot`): the autopilot supplies the
    /// input, scene changes are dropped, and a newly pressed real button or Jetman flying on
    /// his own (state 2) ends it.
    /// `timer` is the shield timer the previous demo left; `held` the real controller now.
    /// With `sound` the demo makes its sound requests (an addition); the original's is silent.
    pub fn start_demo(&mut self, world: &World, timer: u8, held: Buttons, sound: bool) {
        self.demo = Some(autopilot::Demo {
            pilot: autopilot::Autopilot::new(usize::from(world.levels[self.level].number), timer),
            waypoints: world.objects.demo_waypoints.clone(),
            real_held: held,
            over: false,
            sound,
        });
    }

    /// Whether sound requests are dropped: in a demo, unless it was given its sound.
    fn silent(&self) -> bool {
        self.demo.as_ref().is_some_and(|demo| !demo.sound)
    }

    /// A test setup for replays: the pod at rest at (`x`, `y`) and the camera at (`cx`, `cy`).
    pub fn warp(&mut self, x: u16, y: u16, cx: u16, cy: u16) {
        let body = &mut self.pod.body;
        (body.x, body.y, body.x_fraction, body.y_fraction) = (x, y, 0, 0);
        (body.h_speed, body.v_speed) = (0, 0);
        self.camera.position = Point { x: cx, y: cy };
        self.camera.delta = (0, 0);
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

    /// The object table's flags Y and frame bytes, per slot, which a later level start finds
    /// in its free slots ([`Seed::leftovers`]).
    pub fn table_leftovers(&self) -> Vec<(u8, u8)> {
        self.objects
            .slots
            .iter()
            .map(|o| (o.flags_y, o.frame))
            .collect()
    }

    /// The pod's bytes a later level start inherits.
    pub fn pod_leftovers(&self) -> PodLeftovers {
        PodLeftovers {
            move_angle: self.pod.move_angle,
            invulnerable: self.pod.invulnerable,
            accumulator_h: self.pod.accumulator_h,
            accumulator_v: self.pod.accumulator_v,
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
            self.objects
                .smart_bomb(data, &mut self.score, self.original_rules);
            true
        } else {
            let strength = self.pod.equipment.bullet_upgrade + 1;
            self.objects
                .fire_weapon(data, &self.pod, weapon, strength, self.original_rules)
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

    /// Whether this is a demo that has ended.
    /// This is an attract demo.
    pub fn is_demo(&self) -> bool {
        self.demo.is_some()
    }

    pub fn demo_over(&self) -> bool {
        self.demo.as_ref().is_some_and(|demo| demo.over)
    }

    /// The autopilot's waypoint index and shield timer (RAM $0510, $0511).
    pub fn demo_state(&self) -> Option<(usize, u8)> {
        self.demo
            .as_ref()
            .map(|demo| (demo.pilot.waypoint, demo.pilot.timer))
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
        self.pod
            .carry(held, self.route.mx, self.route.my, self.original_rules);
        moved
    }

    pub fn level(&self) -> usize {
        self.level
    }

    /// What the demo's pilot sees (`crate::game::pilot`).
    pub fn observe(&self) -> crate::game::pilot::Observation {
        let body = &self.pod.body;
        let signed = |speed: u16, negative: bool| {
            let speed = f32::from(speed) / 256.0;
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
            time: f64::from(self.counter) / 2.0,
            state: self.pod.state,
            x: f32::from(body.x) + f32::from(body.x_fraction) / 256.0,
            y: f32::from(body.y) + f32::from(body.y_fraction) / 256.0,
            vx: signed(body.h_speed, body.h_left),
            vy: signed(body.v_speed, !body.v_down),
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
    /// (sound.md 8). The demo makes none: its requests are dropped (sound.md 8.2), unless it
    /// was given its sound (`Gameplay::start_demo`).
    pub fn take_sound(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.sfx)
    }

    /// Moves the requests the objects and the pod made into the gameplay's list.
    fn collect_sound(&mut self) {
        let objects = std::mem::take(&mut self.objects.sfx);
        let pod = std::mem::take(&mut self.pod.sfx);
        if !self.silent() {
            self.sfx.extend(objects);
            self.sfx.extend(pod);
        }
    }

    /// Whether the next pass is an A pass.
    pub fn next_is_a_pass(&self) -> bool {
        self.counter & 1 == 0
    }

    /// Runs both passes of one logic step with `buttons` held throughout (the controller is
    /// read before the A pass only, game-flow.md 1.2).
    pub fn step(&mut self, world: &World, buttons: Buttons) {
        debug_assert!(self.next_is_a_pass(), "a step starts with an A pass");
        self.pass(world, buttons);
        if self.transition.is_none() {
            self.pass(world, buttons);
        }
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

    /// Runs one frame's pass. `buttons` is read only by an A pass.
    pub fn pass(&mut self, world: &World, buttons: Buttons) {
        self.apply_cheats();
        self.pass_once(world, buttons);
        self.apply_cheats();
    }

    fn pass_once(&mut self, world: &World, buttons: Buttons) {
        if self.transition.is_some() || self.demo_over() || self.paused {
            // A scene change is pending: the scene layer acts on it (`resume`).
            return;
        }
        let level = &world.levels[self.level];
        let a_pass = self.next_is_a_pass();
        if a_pass {
            self.beam_caught = false;
        }
        // The rotating job counter advances at the start of the frame's handler, before the
        // pass; the first pass of a level sees 1 (measured), and the pod's step may set it (to 9
        // when Jetman emerges, player.md 7.1) for the split that follows.
        self.job = (self.job + 1) & 7;
        // Job 0 uploads the palette (game-flow.md 1.2): the terrain colours and the gate's fades
        // as the last pass left them reach the screen then; the remake shows them at once
        // (docs/deviations.md, "Display refresh").
        if self.job == 0 || !self.original_rules {
            self.terrain.shown = self.terrain.shadow;
            if let Some(gate) = self.gate.as_mut() {
                gate.fades_shown = gate.fades;
            }
        }
        self.shown_before = self.shown_now;
        if a_pass {
            let buttons = if level.is_warp_level() {
                // No shooting on warp levels: A is cleared from the input (player.md 4.1).
                Buttons(buttons.0 & !Buttons::A)
            } else {
                buttons
            };
            if let Some(demo) = self.demo.as_mut() {
                // A newly pressed real button ends the demo before the autopilot writes its bytes.
                let real = buttons;
                let newly = real.pressed_since(demo.real_held);
                demo.real_held = real;
                if newly.0 != 0 {
                    demo.over = true;
                    return;
                }
                let shootable = std::array::from_fn(|i| self.objects.slots[23 + i].state >= 6);
                let seen = autopilot::Observed {
                    level: usize::from(level.number),
                    towed: self.tow.holding(),
                    x: self.pod.body.x,
                    y: self.pod.body.y,
                    h_speed: self.pod.body.h_speed,
                    v_speed: self.pod.body.v_speed,
                    moving_left: self.pod.body.h_left,
                    moving_down: self.pod.body.v_down,
                    facing: self.pod.facing,
                    counter: self.counter as u8,
                    shootable,
                };
                let out = demo.pilot.decide(&demo.waypoints, &seen);
                // The autopilot leaves the waypoint's x high byte in the thrust's horizontal
                // carry-in (game-flow.md 7.1, "$0D"): the star job's value never reaches it.
                if let Some(&(wx, ..)) = demo.waypoints.get(demo.pilot.waypoint) {
                    self.pod.camera_carry = (wx >> 8) as u8;
                }
                self.held = out.held;
                self.pressed = out.pressed;
                self.first_read = false;
            } else {
                self.pressed = if std::mem::take(&mut self.first_read) {
                    Buttons(0)
                } else {
                    buttons.pressed_since(self.held)
                };
                self.held = buttons;
            }
            let ctx = Context {
                world,
                level,
                frame: self.counter as u8,
                original_rules: self.original_rules,
            };
            self.step_start = pod_position(&self.pod);
            let (held, pressed) = (self.held, self.pressed);
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
                self.tow.sign_bits = self.lift_sign_bits();
                // Sound.md 8.3: the rope's hum, every fourth frame while the item is not being
                // taken in; its pitch follows the rope's angle.
                if self.counter & 3 == 0 && !self.tow.taken_in && !self.silent() {
                    let angle = self.tow.angle8();
                    let p = if angle < 0x40 { angle } else { 0x80 - angle };
                    self.sfx.push(Request::RamPitch(p.wrapping_mul(2)));
                    self.sfx.push(Request::a(Channel::Triangle, 0x1A));
                }
            }
            self.pod.route_camera = self.camera.position;
            self.pod.route_end = self.route.done;
            self.pod.route_my = self.route.my;
            self.pod.step(&ctx, held, pressed, &mut self.tow);
            self.collect_sound();
            if self.pod.state == pod::STATE_GATE {
                self.gate_step(level);
                // The drawing stage comes after the state's flash (game-flow.md 9.2, "Bits 0-1").
                self.pod.hit_pause_countdown();
            }
            self.route.my = self.pod.route_my;
            if let Some(demo) = self.demo.as_mut() {
                // The pod's step starts the shield timer (game-flow.md 7.1), with the pod where it moved to: while the level's
                // palette table is still settling, or when an enemy is near.
                if demo.pilot.timer == 0 {
                    let slot = 15 + ((self.counter >> 1) & 15) as usize;
                    let object = &self.objects.slots[slot];
                    let near = object.state != 0
                        && autopilot::near(
                            i32::from(object.x.wrapping_sub(self.pod.body.x) as i16),
                            i32::from(object.y.wrapping_sub(self.pod.body.y) as i16),
                        );
                    if self.pod.index_table_busy || near {
                        demo.pilot.timer = 32;
                    }
                }
            }
            self.after_player(world);
            // The drawing stage eases the F gauge in every player state, the gate's too
            // (economy.md 3.1).
            self.pod.ease_fuel_display();
            if self.pod.autopress {
                self.pressed.0 |= Buttons::A;
            }
            let data = &world.objects;
            // Impact sparks (player.md 5.5) and the pod's shot (10) are objects in slots 3-10.
            let requests = std::mem::take(&mut self.pod.spark_requests);
            self.objects
                .make_sparks(data, &mut self.rng, &requests, self.original_rules);
            if self.pod.fire && !self.silent() && self.objects.nose_shot(&self.pod).is_some() {
                // Player.md 10: the gun's sound, started at once on pulse 2, whatever the
                // weapon. With no slot for the shot there is none (Mesen: eight shots in the
                // air, the press made neither shot nor sound).
                self.sfx.push(Request::Direct {
                    channel: Channel::Pulse2,
                    id: 0x06,
                });
            }
            if self.pod.fire && self.fire_special_weapon(data) {
                // The fitted weapon's shot replaced the plain bullet (or blocked it).
            } else if self.pod.fire {
                let strength = self.pod.equipment.bullet_upgrade + 1;
                self.objects
                    .fire_pod_shot(data, &self.pod, strength, self.original_rules);
            }
            // Slot 2's behaviour, the second of the sweep (slot 1, the flame, does nothing).
            self.place_beam();
            // Every object runs its behaviour (behaviour.md 1).
            let mut env = Env {
                data,
                level,
                camera: self.camera.position,
                pod: &mut self.pod,
                rng: &mut self.rng,
                counter: self.counter,
                score: &mut self.score,
                original_rules: self.original_rules,
                bonus_tag: self.bonus_tag,
                beam_spin: self.tow.holding().then_some(self.tow.spin),
                beam_slot: self.tow.slot,
            };
            self.objects.sweep(&mut env);
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
            let (x, y) = (self.pod.body.x, self.pod.body.y);
            let frozen = matches!(self.pod.state, STATE_POD_EXPLODING | STATE_ASSEMBLING);
            let moved = if self.routed() {
                self.route_a_pass(held)
            } else if frozen {
                (0, 0)
            } else {
                self.camera.set_delta(self.pod.start_low, x, y);
                self.camera.half_step(level, false, (x, y))
            };
            // The star job also runs in the A pass when the terrain lists are short
            // (graphics.md 5.7.3).
            let towing = self.tow.holding();
            let size = self.pod.list_size;
            if !level.is_warp_level() && (size == 0x10 || (size < 0x20 && !towing)) {
                self.stars
                    .call(moved, self.pod.in_sky, self.counter, !self.original_rules);
            }
        } else {
            let moved = if self.routed() {
                self.route.move_camera(&mut self.camera.position)
            } else if matches!(self.pod.state, STATE_POD_EXPLODING | STATE_ASSEMBLING) {
                (0, 0)
            } else {
                self.camera
                    .half_step(level, true, (self.pod.body.x, self.pod.body.y))
            };
            self.star_sprites.clear();
            if !level.is_warp_level() {
                // Drawn once per logic step, at the start of the B pass ($5D is odd), so before
                // this pass's call moves them (graphics.md 5.7.3, "Drawing").
                let phase = self.counter as u8;
                self.star_sprites = self
                    .stars
                    .drawn()
                    .map(|(i, star)| {
                        let flash = matches!((32 * i as u8).wrapping_add(phase), 1 | 5);
                        (star.x, star.y, if flash { 1 } else { 3 })
                    })
                    .collect();
                // The B pass's call comes after its counter increment: the value is even.
                let even = self.counter.wrapping_add(1);
                let carry = self
                    .stars
                    .call(moved, self.pod.in_sky, even, !self.original_rules);
                // The sound driver clears the byte every frame, ahead of the star job; a call
                // that does not run leaves it so (player.md 4.6, rule 3).
                self.star_wrote_carry = carry.is_some();
                if let Some(carry) = carry {
                    self.pod.camera_carry = carry;
                }
            }
        }
        if !a_pass {
            self.pod.update_level_gravity(level.number);
            self.level_routine(world, level);
            // B pass step 2: objects that left the window are freed (not in the gate state:
            // its particles fly out of the window and stay, player.md 9.2.3).
            if self.pod.state != pod::STATE_GATE {
                self.objects.despawn(level, self.camera.position);
            }
        }
        // The status-bar split: the generator steps, then the frame counter (game-flow.md 5).
        let counter = self.counter.to_le_bytes();
        self.rng.step([
            self.held.0,
            self.pressed.0,
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
        let pausing = a_pass
            && self.demo.is_none()
            && self.pressed.has(Buttons::START)
            && matches!(self.pod.state, 1..=3 | 6);
        if pausing {
            self.paused = true;
            self.pause_held = self.held;
        }
        if a_pass && !pausing {
            // After the A pass's split: the green-orb spawner, the items scan (levels.md 3.3)
            // and the collision checks (behaviour.md 3).
            let data = &world.objects;
            let pod_state = self.pod.state;
            let mut env = Env {
                data,
                level,
                camera: self.camera.position,
                pod: &mut self.pod,
                rng: &mut self.rng,
                counter: self.counter,
                score: &mut self.score,
                original_rules: self.original_rules,
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
                self.objects.item_scan(data, self.camera.position);
            }
            // Item 9: the transition guard counts down.
            self.guard = self.guard.saturating_sub(1);
            collide::collide(
                &mut self.objects,
                &mut self.pod,
                data,
                &self.rng,
                &mut self.score,
                &mut self.bonus_tag,
                self.original_rules,
            );
            self.collect_sound();
            // Touching an object has an impact effect too (behaviour.md 3.1).
            let requests = std::mem::take(&mut self.pod.spark_requests);
            self.objects
                .make_sparks(data, &mut self.rng, &requests, self.original_rules);
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
            // Item 13: the demo ends when the player is Jetman on his own.
            if self.pod.state == 2
                && let Some(demo) = self.demo.as_mut()
            {
                demo.over = true;
            }
        } else if self.pod.state != 9 {
            // B pass step 5: the placement scan.
            let scanned = self.objects.step_scan(
                &world.objects,
                level,
                self.camera.position,
                self.original_rules,
            );
            if let Some(carry) = scanned
                && !self.star_wrote_carry
            {
                self.pod.camera_carry = carry;
            }
        }

        // What is shown: the camera as it is, and the pod and the objects half-way through
        // their step after the A pass and at their new positions after the B pass. They all
        // move once a step and the camera in halves, so this keeps them together and makes
        // the motion even across both passes.
        let now = pod_position(&self.pod);
        let mut objects = object_views(&self.objects);
        let pod = if a_pass {
            halfway_objects(&self.shown_before.objects, &mut objects);
            (
                (self.step_start.0 + now.0) / 2.0,
                (self.step_start.1 + now.1) / 2.0,
            )
        } else {
            now
        };
        self.shown_now = View {
            camera: (
                f32::from(self.camera.position.x),
                f32::from(self.camera.position.y),
            ),
            pod,
            objects,
        };
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
        let mut drawn = Vec::new();
        for slot in (1..objects::SLOTS).rev() {
            let object = &self.objects.slots[slot];
            if !ObjectTable::is_drawn(object, self.camera.position) {
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

    /// The state the replay comparison checks, as (name, value) pairs with the names of the
    /// capture script's log (`tools/mesen/replay-capture.lua`).
    /// Bit 0 of the 64 bytes from RAM $0355 as the object table stands now (player.md 8.3 b):
    /// the arrays are contiguous, so these are the flags Y of slots 0-30 ($0355-$0373), the
    /// frames of slots 0-30 ($0374-$0392) and the list index of slots 0 and 1 ($0393-$0394).
    /// Slot 0 is the pod.
    fn lift_sign_bits(&self) -> u64 {
        let slots = &self.objects.slots;
        let bit = |x: usize| -> bool {
            if x >= 62 {
                return x != 62 && slots[x - 62].index & 1 != 0;
            }
            let (array, i) = (x / 31, x % 31);
            if i == 0 && array == 0 {
                return self.pod.body.v_down;
            }
            if i == 0 {
                return self.pod.facing & 1 != 0;
            }
            (if array == 0 {
                slots[i].flags_y
            } else {
                slots[i].frame
            }) & 1
                != 0
        };
        (0..64).fold(0u64, |bits, x| bits | u64::from(bit(x)) << x)
    }

    pub fn snapshot(&self) -> Vec<(&'static str, u32)> {
        // The capture shows the rope's bytes only while something hangs on the beam.
        let p = &self.pod;
        let b = &p.body;
        let rng =
            u32::from(self.rng.out) << 16 | u32::from(self.rng.high) << 8 | u32::from(self.rng.low);
        let counter = self.counter.to_le_bytes();
        let mut fields = vec![
            ("x", b.x.into()),
            ("xf", b.x_fraction.into()),
            ("y", b.y.into()),
            ("yf", b.y_fraction.into()),
            ("hw", (b.h_speed >> 8).into()),
            ("hf", (b.h_speed & 0xFF).into()),
            ("hl", b.h_left.into()),
            ("vw", (b.v_speed >> 8).into()),
            ("vf", (b.v_speed & 0xFF).into()),
            ("vd", b.v_down.into()),
            ("face", p.facing.into()),
            ("st", p.state.into()),
            ("fuel", p.fuel.into()),
            ("cx", self.camera.position.x.into()),
            ("cy", self.camera.position.y.into()),
            ("ang", p.move_angle.into()),
            ("hp", p.hit_pause.into()),
            ("iv", p.invulnerable.into()),
            ("sh", if p.shield { 0x80 } else { 0 }),
            ("ach", p.accumulator_h.into()),
            ("acv", p.accumulator_v.into()),
            ("dd", p.surface.into()),
            ("f1", p.width.into()),
            ("f2", p.half_height.into()),
            ("t66", self.tow.slot as u32),
            (
                "t67",
                // The crystals counted on a warp level share the byte with the beam's load.
                if self.level >= crate::game::level::FIRST_WARP_LEVEL {
                    self.warp_crystals
                } else {
                    self.tow.mass
                }
                .into(),
            ),
            ("t68", (self.tow.angle & 0xFF).into()),
            ("t69", (self.tow.angle >> 8).into()),
            ("t6a", self.tow.speed & 0xFF),
            ("t6b", (self.tow.speed >> 8) & 0xFF),
            ("t6c", self.tow.speed >> 16),
            ("t6d", self.tow.sense.into()),
            ("t6e", self.tow.spin.into()),
            ("ptm", p.timer.into()),
            ("pcn", p.counter.into()),
            ("fl", p.direction_flags().into()),
            ("lives", self.lives.into()),
            ("pods", self.jetpods.into()),
            (
                "e0",
                if self.level == 26 {
                    self.route.step
                } else {
                    self.crystals
                }
                .into(),
            ),
            ("c9", p.weapon_energy.into()),
            ("c8", p.weapon_low.into()),
            ("ag", p.antigravity.into()),
            ("d2", self.bonus_tag.into()),
            ("bay", if p.bay { 0x20 } else { 0 }),
            // The F gauge's display copy RAM $05CE (economy.md 3.1).
            ("fs", p.fuel_shown.into()),
            ("grd", self.guard.into()),
            (
                "fc",
                self.objects.items.first().map_or(0, |e| e.kind).into(),
            ),
            ("score", self.score.as_digits()),
            ("rng", rng),
            (
                "ctr",
                u32::from(counter[0]) << 16 | u32::from(counter[1]) << 8 | u32::from(counter[2]),
            ),
        ];
        if self.level >= crate::game::level::FIRST_WARP_LEVEL {
            // The mesh helpers' position bytes (objects.md 10.6).
            let [wx, wy, vx, vy] = self.helper_bytes();
            fields.push(("wx", wx.into()));
            fields.push(("wy", wy.into()));
            fields.push(("vx", vx.into()));
            fields.push(("vy", vy.into()));
        }
        if let Some(demo) = &self.demo {
            // The autopilot's memory and the bytes it wrote (RAM $0510, $0511, $18, $19).
            fields.push(("wp", (demo.pilot.waypoint - demo.pilot.base) as u32));
            fields.push(("tm", demo.pilot.timer.into()));
            fields.push(("ih", self.held.0.into()));
            fields.push(("ip", self.pressed.0.into()));
            fields.push(("cc", self.pod.camera_carry.into()));
        }
        fields
    }
}

/// Data the original's rules read that the remake does not ship: the caller reads it from the
/// player's ROM and passes it to [`Gameplay::set_validation_input`]. Without it the validation
/// mode falls back to the remake's rule where it would need these bytes.
#[derive(Clone, Debug, Default)]
pub struct ValidationInput {
    /// The ROM bytes the warp mesh lookup of levels 37 and 39 reads when its index is a whole
    /// byte (objects.md 10.6, Q38): from the first source table's start, `SHIMMER_BYTES`.
    pub warp_shimmer: Arc<[u8]>,
}
