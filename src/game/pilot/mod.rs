//! The attract demo's pilot: a rules-based player that flies the pod from what the engine
//! shows it (the level's terrain, the objects, the pod's own state), in either engine. It plays
//! the game's loop: find an item, catch it with the tractor beam, tow it home to the mothership,
//! and shoot or shield against what comes near on the way. It reads no recorded route, so a
//! change to the levels or to the physics changes its flight, not whether it can fly.
//!
//! Units are pixels and logic steps (1/30 s): the speeds are pixels per step, as the engines
//! keep them. Floats are used only with operations IEEE 754 rounds exactly, so a run is the same
//! on every platform.

pub mod nav;

use nav::{Field, NavMap, UNREACHED};

use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::objects::Object;
use crate::game::level::World;

/// The thrust table, a cosine × 64 by facing (player.md 4.4).
const THRUST: [u8; 16] = [
    0x40, 0x3E, 0x3B, 0x35, 0x2D, 0x23, 0x18, 0x0C, 0x00, 0x0C, 0x18, 0x23, 0x2D, 0x35, 0x3B, 0x3E,
];
/// The acceleration of the plain engine at full thrust, px per step² (the boosters give four
/// times as much).
const ENGINE: f32 = 16.0 / 256.0;
/// The item kinds worth towing home, and how heavy each is (economy.md 2.1, player.md 8.1).
const WANTED: [(u8, u8); 10] = [
    (15, 0),
    (16, 0),
    (5, 1),
    (12, 1),
    (9, 1),
    (3, 1),
    (4, 1),
    (10, 0),
    (6, 2),
    (19, 2),
];
/// Steps to turn about before braking, on average (player.md 4.3: a turn step per step).
const TURN_ABOUT: f32 = 10.0;
/// Steps between one turn and the next: a pilot that settles on a facing looks calmer than
/// one that fiddles with it.
const TURN_GAP: f64 = 6.0;
/// Steps ahead the pull is allowed for when judging a burn.
const LOOK_AHEAD: f32 = 15.0;
/// Fuel delivered without the delivery screen (economy.md 2.2): fetched only when nothing else
/// is reachable.
const FUEL: u8 = 10;
/// Where the pod hovers to deliver: above the home position, the load hanging under it in the
/// mothership's snap column (economy.md 2.2).
const DELIVERY_HEIGHT: i32 = 96;
/// The playfield's height on screen, in pixels (the status panel is below it).
const PLAYFIELD_HEIGHT: f32 = crate::game::playfield::PLAYFIELD_LINES as f32;
/// Fuel units of a full tank's high byte.
const FUEL_FULL: u16 = 0xE000;

/// What the pilot sees of the game in one call: the engines build it (`observe`).
#[derive(Debug, Clone, Default)]
pub struct Observation {
    /// The level index into the world's levels.
    pub level: usize,
    /// Game time in logic steps.
    pub time: f64,
    /// The player state (1: the pod).
    pub state: u8,
    /// The pod's position in pixels and its speed in pixels per step (+x right, +y down).
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Facing 0-31, a 1/32 turn clockwise from up.
    pub facing: u8,
    pub fuel: u16,
    pub shield: bool,
    pub has_shields: bool,
    pub has_boosters: bool,
    /// A special weapon is fitted and its gauge is not empty.
    pub special: bool,
    /// The towed object's position, and whether the mothership is taking it in.
    pub towed: Option<(f32, f32)>,
    pub taken_in: bool,
    /// The camera: the top left corner of the playfield on screen.
    pub camera: (f32, f32),
    /// The towed item's list index, when it is from the items list.
    pub towed_index: Option<usize>,
    /// The level's items list entries still in play: (list index, kind, x, y), the position
    /// from the item's slot while it has one.
    pub items: Vec<(usize, u8, f32, f32)>,
    /// Objects that hurt the pod: position, and whether shooting can destroy it.
    pub threats: Vec<Threat>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threat {
    pub x: f32,
    pub y: f32,
    /// A shot or a small mover: it closes in, and the shield is the answer.
    pub missile: bool,
    /// The object's state (objects.md 2.4).
    pub state: u8,
    /// A mover's speed in px per step (placed objects: 0, their speed fields hold other things).
    pub vx: f32,
    pub vy: f32,
}

impl Observation {
    /// Whether (`x`, `y`) is on the playfield on screen, with `margin` pixels to spare.
    pub fn on_screen(&self, x: f32, y: f32, margin: f32) -> bool {
        let (sx, sy) = (x - self.camera.0, y - self.camera.1);
        (margin..256.0 - margin).contains(&sx) && (margin..PLAYFIELD_HEIGHT - margin).contains(&sy)
    }

    /// The parts both engines share: the object slots and the items list (kind, x, y per
    /// entry). Slots 11-14 hold the live items, 15-30 what hurts (objects.md 2.2).
    pub fn read_objects(
        &mut self,
        slots: &[Object],
        towed: usize,
        items: impl Iterator<Item = (u8, u16, u16)>,
    ) {
        self.towed_index = slots
            .get(towed)
            .filter(|o| towed != 0 && o.index & 0x80 == 0)
            .map(|o| usize::from(o.index));
        self.items = items
            .enumerate()
            .filter(|&(_, (kind, _, _))| kind != 0)
            .map(|(index, (kind, x, y))| (index, kind, f32::from(x), f32::from(y)))
            .collect();
        for slot in slots.iter().take(15).skip(11) {
            if slot.state == 0 || slot.index & 0x80 != 0 {
                continue;
            }
            let index = usize::from(slot.index);
            if let Some(item) = self.items.iter_mut().find(|item| item.0 == index) {
                (item.2, item.3) = position(slot);
            }
        }
        self.threats = slots
            .iter()
            .skip(15)
            .filter(|o| hurts(o.state))
            .map(|o| {
                let (x, y) = position(o);
                let missile = o.state < 0x80;
                let speed = |speed: u16, negative: bool| {
                    let speed = f32::from(speed) / 256.0;
                    if !missile {
                        0.0
                    } else if negative {
                        -speed
                    } else {
                        speed
                    }
                };
                Threat {
                    x,
                    y,
                    missile,
                    state: o.state,
                    vx: speed(o.speed_x, o.left()),
                    vy: speed(o.speed_y, !o.down()),
                }
            })
            .collect();
    }
}

fn position(o: &Object) -> (f32, f32) {
    (
        f32::from(o.x) + f32::from(o.x_fraction) / 256.0,
        f32::from(o.y) + f32::from(o.y_fraction) / 256.0,
    )
}

/// Whether an object in slots 15-30 in `state` can hurt the pod: not free, and not a spark,
/// exhaust, explosion, marker, item or bonus (objects.md 2.4).
fn hurts(state: u8) -> bool {
    !matches!(state, 0..=7 | 0x0A | 0x0B | 0x0E | 0x21 | 0x24 | 0x25)
}

/// The thrust direction of a facing as a unit-ish vector (x right, y down), and its length
/// as a fraction of full thrust (1 at the axes, about 1 diagonally).
fn thrust_vector(facing: u8) -> (f32, f32) {
    let f = facing & 31;
    let vertical = f32::from(THRUST[usize::from(f & 15)]) / 64.0;
    let horizontal = f32::from(THRUST[usize::from((f + 8) & 15)]) / 64.0;
    let sx = if (17..=31).contains(&f) { -1.0 } else { 1.0 };
    let sy = if (9..=23).contains(&f) { 1.0 } else { -1.0 };
    (sx * horizontal, sy * vertical)
}

/// The facing whose thrust points most nearly along (`x`, `y`).
fn facing_toward(x: f32, y: f32) -> u8 {
    let length = (x * x + y * y).sqrt().max(1e-6);
    (0..32u8)
        .max_by(|&a, &b| {
            let dot = |f| {
                let (tx, ty) = thrust_vector(f);
                (tx * x + ty * y) / length
            };
            dot(a).total_cmp(&dot(b))
        })
        .unwrap_or(0)
}

/// The signed turn from facing `from` to `to`, -16 to 15.
fn turn(from: u8, to: u8) -> i32 {
    (i32::from(to) - i32::from(from) + 16).rem_euclid(32) - 16
}

/// What the pilot is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Task {
    /// Flying to the item with this list index, to catch it.
    Fetch(usize),
    /// Towing a load home.
    Deliver,
    /// Nothing to fetch: flying about.
    Roam,
}

/// The pilot's memory between calls. The maps are made again after a saved state is loaded.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Pilot {
    pub task: Task,
    /// The input the last call gave.
    held: Buttons,
    /// What moves the pod besides its thrust (gravity, the load), px per step², estimated from
    /// how the speed changes.
    push: (f32, f32),
    /// The speed at the start of the stretch being learnt from, and whether it had thrust.
    sample: Option<(f64, f32, f32)>,
    thrusted: bool,
    /// When a turn button may next be pressed, and when a tap of Down or Up ends.
    turn_ready: f64,
    tap_until: f64,
    /// Items given up on (list index).
    skipped: Vec<usize>,
    /// The closest the pod came to the task's goal, and when.
    best_cost: u32,
    best_at: f64,
    /// When a threat last came close, and when the special weapon last fired.
    danger_at: f64,
    special_at: f64,
    /// The roaming goal.
    roam: Option<(i32, i32)>,
    /// The item last on the beam: after letting go of it the pilot comes back for it.
    load: Option<usize>,
    /// A placed object being shot at, until when, and when the next fight may start.
    fight: Option<((f32, f32), f64)>,
    fight_ready: f64,
    /// The burn (thrust held), whether it was on last call and since when, and the boosters.
    burning: bool,
    burned: bool,
    burn_since: f64,
    boosting: bool,
    boost_since: f64,
    #[serde(skip)]
    maps: Option<Maps>,
}

#[derive(Debug, Clone)]
struct Maps {
    level: usize,
    nav: NavMap,
    home: Field,
    /// The field of the current goal (the item, or a place to roam to).
    goal: Option<Field>,
}

impl Default for Pilot {
    fn default() -> Pilot {
        Pilot::new()
    }
}

impl Pilot {
    pub fn new() -> Pilot {
        Pilot {
            task: Task::Roam,
            held: Buttons(0),
            push: (0.0, 0.0),
            sample: None,
            thrusted: false,
            turn_ready: 0.0,
            tap_until: -1.0e9,
            skipped: Vec::new(),
            best_cost: UNREACHED,
            best_at: 0.0,
            danger_at: -1.0e9,
            special_at: -1.0e9,
            roam: None,
            load: None,
            fight: None,
            fight_ready: 0.0,
            burning: false,
            burned: false,
            burn_since: 0.0,
            boosting: false,
            boost_since: 0.0,
            maps: None,
        }
    }

    /// The buttons to hold now.
    pub fn decide(&mut self, world: &World, seen: &Observation) -> Buttons {
        if seen.state != 1 {
            self.held = Buttons(0);
            self.sample = None;
            return self.held;
        }
        self.learn(seen);
        let level = &world.levels[seen.level];
        let home = (i32::from(level.home.x), i32::from(level.home.y));
        if self.maps.as_ref().is_none_or(|m| m.level != seen.level) {
            let nav = NavMap::new(world, level, (seen.x as u16, seen.y as u16), bay(home));
            let delivery = (home.0, home.1 - DELIVERY_HEIGHT);
            let field = nav
                .field(delivery, 3)
                .or_else(|| nav.field(delivery, 2))
                .or_else(|| nav.field((seen.x as i32, seen.y as i32), 1));
            let Some(home_field) = field else {
                return Buttons(0);
            };
            self.maps = Some(Maps {
                level: seen.level,
                nav,
                home: home_field,
                goal: None,
            });
            self.task = Task::Roam;
            self.push = (0.0, f32::from(level.gravity) / 1024.0);
        }
        self.plan(seen, home);
        let mut held = 0u8;
        let (target, careful) = self.steer_target(seen, home);
        // In a fight the pilot holds the line of fire, burning only when it must.
        let aim = self.aim(seen);
        let (mut want_facing, thrust, boost) = self.fly(seen, target, careful, aim.is_some());
        if !thrust && let Some(aim) = aim {
            let sinking = seen.vy > 0.5
                || self
                    .maps
                    .as_ref()
                    .is_some_and(|m| m.nav.clearance_at(seen.x, seen.y) <= 2);
            if !sinking {
                // A turn step either side is near enough: the gun fires on a near line too.
                want_facing = if turn(seen.facing, aim).abs() <= 1 {
                    seen.facing
                } else {
                    aim
                };
            }
        }
        // Turning: one press at a time, so a pilot that wavers turns no faster than a hold.
        let error = turn(seen.facing, want_facing);
        let turning = self.held.0 & (Buttons::LEFT | Buttons::RIGHT);
        if error != 0 && (turning != 0 || seen.time >= self.turn_ready) {
            held |= if error < 0 {
                Buttons::LEFT
            } else {
                Buttons::RIGHT
            };
        } else if turning != 0 {
            self.turn_ready = seen.time + TURN_GAP;
        }
        if thrust {
            held |= Buttons::B;
            if boost {
                held |= Buttons::SELECT;
            }
        }
        held |= self.combat(seen, held);
        self.held = Buttons(held);
        self.thrusted |= thrust;
        self.held
    }

    /// Estimates the push from a stretch of at least a step with no thrust: the change of
    /// speed is then all gravity, the load and whatever else pulls. (Learning from thrust too
    /// would blame a pod pinned against something on a push, and thrust harder against it.)
    fn learn(&mut self, seen: &Observation) {
        let Some((t0, vx0, vy0)) = self.sample else {
            self.sample = Some((seen.time, seen.vx, seen.vy));
            self.thrusted = false;
            return;
        };
        if self.thrusted {
            self.sample = Some((seen.time, seen.vx, seen.vy));
            self.thrusted = false;
            return;
        }
        let span = (seen.time - t0) as f32;
        if span < 1.0 {
            return;
        }
        let ax = (seen.vx - vx0) / span;
        let ay = (seen.vy - vy0) / span;
        // A bounce or a hit is not a push.
        if ax.abs() < 0.12 && ay.abs() < 0.12 {
            self.push.0 += (ax - self.push.0) * 0.25;
            self.push.1 += (ay - self.push.1) * 0.25;
        }
        self.sample = Some((seen.time, seen.vx, seen.vy));
    }

    /// The part of the engine's thrust that moves the pod: all of it, or with a load on the
    /// beam the share its class and the facing leave (player.md 8.3 c).
    fn thrust_share(&self, seen: &Observation) -> f32 {
        if seen.towed.is_none() || seen.taken_in {
            return 1.0;
        }
        let kind = seen
            .towed_index
            .and_then(|index| seen.items.iter().find(|item| item.0 == index))
            .map(|item| item.1);
        let class = kind
            .and_then(|kind| WANTED.iter().find(|w| w.0 == kind))
            .map_or(1, |w| w.1);
        let f = 0x38 + 32 * (2 - i32::from(class)) + ((4 * i32::from(seen.facing)) & 63);
        f as f32 / 256.0
    }

    /// Chooses the task and keeps the goal's field up to date.
    fn plan(&mut self, seen: &Observation, home: (i32, i32)) {
        if seen.towed.is_some() {
            self.load = seen.towed_index.or(self.load);
            if self.task != Task::Deliver {
                self.set_task(Task::Deliver, seen);
            }
        } else if self.task == Task::Deliver {
            self.set_task(Task::Roam, seen);
        }
        if let Task::Fetch(index) = self.task {
            let gone = !seen.items.iter().any(|item| item.0 == index);
            let stuck = seen.time - self.best_at > 600.0;
            if gone || stuck {
                if stuck {
                    self.skipped.push(index);
                }
                self.set_task(Task::Roam, seen);
            }
        }
        if self.task == Task::Roam
            && let Some(index) = self.choose_item(seen)
        {
            self.set_task(Task::Fetch(index), seen);
        }
        let Some(maps) = self.maps.as_mut() else {
            return;
        };
        let goal = match self.task {
            Task::Deliver => None,
            Task::Fetch(index) => seen
                .items
                .iter()
                .find(|item| item.0 == index)
                .map(|item| (item.2 as i32, item.3 as i32)),
            Task::Roam => {
                if self.roam.is_none_or(|(x, y)| {
                    (x as f32 - seen.x).abs() + (y as f32 - seen.y).abs() < 48.0
                }) {
                    self.roam = Some(far_point(maps, seen, home));
                }
                self.roam
            }
        };
        match goal {
            None => maps.goal = None,
            Some(goal) => {
                let moved = maps.goal.as_ref().is_none_or(|field| {
                    (field.goal.0 - goal.0).abs() + (field.goal.1 - goal.1).abs() > 24
                });
                if moved {
                    // Room to spare if there is some within the beam's reach of the goal, else
                    // the closest the pod fits.
                    let cell = NavMap::cell(goal.0, goal.1);
                    let near = |min| {
                        maps.nav
                            .nearest(cell, min)
                            .is_some_and(|(x, y)| (x - cell.0).abs().max((y - cell.1).abs()) <= 2)
                    };
                    let min = if near(2) { 2 } else { 1 };
                    maps.goal = maps.nav.field(goal, min);
                }
            }
        }
        let field = maps.goal.as_ref().unwrap_or(&maps.home);
        let cost = field.cost_at(&maps.nav, seen.x, seen.y);
        if cost < self.best_cost.saturating_sub(40) {
            self.best_cost = cost;
            self.best_at = seen.time;
        }
    }

    fn set_task(&mut self, task: Task, seen: &Observation) {
        self.task = task;
        self.best_cost = UNREACHED;
        self.best_at = seen.time;
        if let Some(maps) = self.maps.as_mut() {
            maps.goal = None;
        }
    }

    /// The item to fetch: the cheapest round trip, light loads first, fuel last.
    fn choose_item(&self, seen: &Observation) -> Option<usize> {
        let maps = self.maps.as_ref()?;
        if let Some(load) = self.load
            && !self.skipped.contains(&load)
            && seen.items.iter().any(|item| item.0 == load)
        {
            return Some(load);
        }
        let mut best: Option<(u64, usize)> = None;
        for &(index, kind, x, y) in &seen.items {
            let Some(&(_, mass)) = WANTED.iter().find(|w| w.0 == kind) else {
                continue;
            };
            if self.skipped.contains(&index) {
                continue;
            }
            let cost = maps.home.cost_at(&maps.nav, x, y);
            if cost == UNREACHED {
                continue;
            }
            let mut score = u64::from(cost) * (4 + u64::from(mass)) / 4;
            if kind == FUEL {
                score += 1 << 24;
            }
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, index));
            }
        }
        best.map(|(_, index)| index)
    }

    /// The point to fly at now, and whether to fly carefully (near the goal or towing).
    fn steer_target(&self, seen: &Observation, home: (i32, i32)) -> ((f32, f32, f32), bool) {
        let Some(maps) = self.maps.as_ref() else {
            return ((seen.x, seen.y, 0.0), true);
        };
        let towing = seen.towed.is_some();
        let field = maps.goal.as_ref().unwrap_or(&maps.home);
        let goal = match self.task {
            Task::Deliver => (home.0 as f32, (home.1 - DELIVERY_HEIGHT) as f32),
            _ => (field.goal.0 as f32, field.goal.1 as f32),
        };
        let direct = ((goal.0 - seen.x).powi(2) + (goal.1 - seen.y).powi(2)).sqrt();
        let (bx0, by0, bx1, by1) = bay(home);
        let (px, py) = (seen.x as i32, seen.y as i32);
        if !towing && (bx0..=bx1).contains(&px) && (by0..=by1).contains(&py) {
            // Out of the mothership's bay first, straight up: back in it the pod docks.
            return ((home.0 as f32, (by0 - 24) as f32, 48.0), true);
        }
        if direct < 64.0 && maps.nav.line_clear((seen.x, seen.y), goal, 1) {
            return ((goal.0, goal.1, direct), true);
        }
        let path = field.path(&maps.nav, (seen.x, seen.y), 14);
        let remaining = f32::from(
            u16::try_from(field.cost_at(&maps.nav, seen.x, seen.y) / 10).unwrap_or(u16::MAX),
        ) * nav::CELL as f32;
        // The farthest cell of the path in plain sight, with room to spare if there is any.
        let in_sight = |min: u8| {
            path.iter()
                .rev()
                .map(|&cell| NavMap::centre(cell))
                .find(|&point| maps.nav.line_clear((seen.x, seen.y), point, min))
        };
        let ahead = in_sight(3)
            .or_else(|| in_sight(2))
            .or_else(|| path.get(1).map(|&cell| NavMap::centre(cell)))
            .unwrap_or(goal);
        ((ahead.0, ahead.1, remaining.max(direct.min(64.0))), towing)
    }

    /// The facing wanted, and whether to thrust (and with the boosters), to fly at `target`
    /// (x, y, distance left along the way). It flies as a player does: a burn is held until the
    /// speed is right, then the pod coasts, and when the goal comes up faster than the pod can
    /// stop it turns about and brakes, with the boosters if it has them.
    fn fly(
        &mut self,
        seen: &Observation,
        target: (f32, f32, f32),
        careful: bool,
        fighting: bool,
    ) -> (u8, bool, bool) {
        let (tx, ty, left) = target;
        let (dx, dy) = (tx - seen.x, ty - seen.y);
        let distance = (dx * dx + dy * dy).sqrt();
        let clearance = self
            .maps
            .as_ref()
            .map_or(2, |m| m.nav.clearance_at(seen.x, seen.y));
        let mut top: f32 = match clearance {
            0..=2 => 1.0,
            3 => 1.6,
            4 => 2.2,
            5 | 6 => 2.9,
            _ => 3.6,
        };
        if careful {
            top *= 0.75;
        }
        let engine = ENGINE * self.thrust_share(seen);
        let boosters = seen.has_boosters && seen.fuel > FUEL_FULL / 16;
        // The speed it can still stop from in `d` pixels: half a turn about, then the brake.
        let (ux, uy) = if distance > 0.5 {
            (dx / distance, dy / distance)
        } else {
            (0.0, 0.0)
        };
        // The pull along the way (gravity, going down) leaves less to brake with.
        let pull = (self.push.0 * ux + self.push.1 * uy).max(0.0);
        let brake = (if boosters { 4.0 * engine } else { engine } * 0.6 - pull).max(engine * 0.2);
        let stoppable =
            |d: f32| brake * (((TURN_ABOUT * TURN_ABOUT) + 2.0 * d / brake).sqrt() - TURN_ABOUT);
        let speed = top
            .min(stoppable(left.max(0.0)))
            .min(stoppable(distance + 32.0))
            .min(distance / 8.0);
        let (wx, wy) = (ux * speed, uy * speed);
        // The change of speed wanted, allowing for what the pull will do over the next third
        // of a second: burns then run long and the coasts between them too, as a player's do.
        let (ex, ey) = (
            wx - seen.vx - self.push.0 * LOOK_AHEAD,
            wy - seen.vy - self.push.1 * LOOK_AHEAD,
        );
        let need = (ex * ex + ey * ey).sqrt();
        let (fx, fy) = thrust_vector(seen.facing);
        let along = fx * ex + fy * ey;
        let wanted = facing_toward(ex, ey);
        let off = turn(seen.facing, wanted).abs();
        // Hold the facing through a burn, and through small errors between burns.
        // Lazy in the open, tighter by the walls.
        let open = clearance >= 5;
        let (slack, settle) = if open { (6, 8.0) } else { (3, 4.0) };
        let keep = if self.burning {
            off <= slack
        } else {
            off <= 2 || need < 0.25
        };
        let facing = if keep { seen.facing } else { wanted };
        // Precise near the goal, in long burns on the way.
        let start = if fighting {
            0.8
        } else if speed < 0.6 {
            0.25
        } else {
            0.45
        };
        let held = seen.time - self.burn_since;
        self.burning = if self.burning {
            off <= slack && (along > 0.05 || held < settle)
        } else {
            off <= 2 && along > start && held >= settle
        };
        if self.burning != self.burned {
            self.burned = self.burning;
            self.burn_since = seen.time;
        }
        // The boosters for a big change, as long pushes too: a quarter second at least.
        let boosting = boosters
            && self.burning
            && if self.boosting {
                along > 0.4 || seen.time - self.boost_since < 8.0
            } else {
                along > 1.0 && seen.time - self.boost_since > 8.0
            };
        if boosting != self.boosting {
            self.boosting = boosting;
            self.boost_since = seen.time;
        }
        (facing, self.burning, self.boosting)
    }

    /// The facing that aims at a placed object worth stopping for (a turret or gun near the
    /// way), if any, for up to 40 steps at a time.
    fn aim(&mut self, seen: &Observation) -> Option<u8> {
        let speed = (seen.vx * seen.vx + seen.vy * seen.vy).sqrt();
        let maps = self.maps.as_ref()?;
        let visible = |t: &Threat| {
            let (dx, dy) = (t.x - seen.x, t.y - seen.y);
            !t.missile
                && seen.on_screen(t.x, t.y, 16.0)
                && dx * dx + dy * dy < 120.0 * 120.0
                && maps.nav.line_clear((seen.x, seen.y), (t.x, t.y), 1)
        };
        if let Some((at, until)) = self.fight {
            let target = seen
                .threats
                .iter()
                .filter(|t| visible(t))
                .min_by(|a, b| {
                    let d = |t: &Threat| (t.x - at.0).abs() + (t.y - at.1).abs();
                    d(a).total_cmp(&d(b))
                })
                .filter(|t| (t.x - at.0).abs() + (t.y - at.1).abs() < 24.0);
            match target {
                Some(t) if seen.time < until => {
                    self.fight = Some(((t.x, t.y), until));
                    return Some(facing_toward(t.x - seen.x, t.y - seen.y));
                }
                _ => {
                    self.fight = None;
                    self.fight_ready = seen.time + 30.0;
                    return None;
                }
            }
        }
        if speed > 1.3 || seen.time < self.fight_ready {
            return None;
        }
        // A fight is picked only where the pod can hold its height while aiming: not close
        // by, and not below it (the gun would point away from the thrust that keeps it up).
        let t = seen.threats.iter().find(|t| {
            let (dx, dy) = (t.x - seen.x, t.y - seen.y);
            visible(t)
                && dx * dx + dy * dy > 40.0 * 40.0
                && turn(0, facing_toward(dx, dy)).abs() <= 8
        })?;
        self.fight = Some(((t.x, t.y), seen.time + 40.0));
        Some(facing_toward(t.x - seen.x, t.y - seen.y))
    }

    /// The shield and the gun, added to the flight's buttons.
    fn combat(&mut self, seen: &Observation, held: u8) -> u8 {
        let mut out = 0u8;
        // The beam catches nothing with the shield on, and Down lets go of a load.
        let fetching_close = match self.task {
            Task::Fetch(index) => seen
                .items
                .iter()
                .find(|item| item.0 == index)
                .is_some_and(|item| (item.2 - seen.x).abs() + (item.3 - seen.y).abs() < 96.0),
            _ => false,
        };
        // With a load on the beam the shield costs the load, so only a sure hit counts.
        let (range, within) = if seen.towed.is_some() {
            (16.0, 10.0)
        } else {
            (24.0, 20.0)
        };
        // Within the beam's reach of the item a wide berth would keep the shield on and the beam
        // off: then only near contact with a placed object counts (behaviour.md 3.1).
        let catching = match self.task {
            Task::Fetch(index) => {
                seen.items
                    .iter()
                    .find(|item| item.0 == index)
                    .is_some_and(|item| {
                        (item.2 - seen.x).abs() < 32.0 && (item.3 - seen.y).abs() < 40.0
                    })
            }
            _ => false,
        };
        let danger = seen.threats.iter().any(|t| {
            let range = if catching && !t.missile { 8.0 } else { range };
            imminent(seen, t, range, within)
        });
        if danger {
            self.danger_at = seen.time;
        }
        let tapping = seen.time < self.tap_until;
        if tapping {
            out |= self.held.0 & (Buttons::UP | Buttons::DOWN);
        } else if seen.has_shields && seen.time >= self.tap_until + 1.0 {
            let calm = seen.time - self.danger_at > if fetching_close { 6.0 } else { 30.0 };
            if !seen.shield && danger && seen.fuel > FUEL_FULL / 16 {
                // With a load on the beam this lets it go too; it is caught again after.
                out |= Buttons::DOWN;
                self.tap_until = seen.time + 1.5;
            } else if seen.shield && calm {
                out |= Buttons::UP;
                self.tap_until = seen.time + 1.5;
            }
        }
        // The gun points along the facing: fire at what lies within a turn step of it.
        let (fx, fy) = thrust_vector(seen.facing);
        let line = self.maps.as_ref();
        // Only at what the player can see: shots into the dark look like a fault.
        let in_sight = |t: &Threat| {
            seen.on_screen(t.x, t.y, 8.0)
                && line.is_none_or(|m| m.nav.line_clear((seen.x, seen.y), (t.x, t.y), 1))
        };
        let ahead = seen.threats.iter().any(|t| {
            let (dx, dy) = (t.x - seen.x, t.y - seen.y);
            let d = (dx * dx + dy * dy).sqrt();
            d < 150.0 && (dx * fx + dy * fy) > d * 0.95 && in_sight(t)
        });
        if ahead && held & Buttons::SELECT == 0 {
            out |= Buttons::A;
        }
        // The fitted weapon finds its own way: a shot now and then at anything in range.
        let in_range = seen
            .threats
            .iter()
            .any(|t| (t.x - seen.x).abs() < 140.0 && (t.y - seen.y).abs() < 120.0 && in_sight(t));
        // Not in a burn: B with Select would be the boosters.
        let burning = held & Buttons::B != 0;
        if seen.special && in_range && !burning && seen.time - self.special_at > 40.0 {
            self.special_at = seen.time;
            out |= Buttons::A | Buttons::SELECT;
        } else if seen.time - self.special_at < 1.5 {
            out |= self.held.0 & (Buttons::A | Buttons::SELECT);
        }
        out
    }
}

/// The mothership's bay around the home position, closed to the pilot's paths: a pod with
/// nothing on the beam that flies into it docks (economy.md 2.2, "The bay area").
pub(crate) fn bay(home: (i32, i32)) -> (i32, i32, i32, i32) {
    (home.0 - 16, home.1 - 8, home.0 + 16, home.1 + 16)
}

/// Whether `threat` will pass within `range` pixels of the pod soon: the closest approach of
/// the two on their present courses, over the next `within` steps.
fn imminent(seen: &Observation, threat: &Threat, range: f32, within: f32) -> bool {
    let (rx, ry) = (threat.x - seen.x, threat.y - seen.y);
    let (vx, vy) = (threat.vx - seen.vx, threat.vy - seen.vy);
    let speed2 = vx * vx + vy * vy;
    let t = if speed2 > 1e-4 {
        (-(rx * vx + ry * vy) / speed2).clamp(0.0, within)
    } else {
        0.0
    };
    let (cx, cy) = (rx + vx * t, ry + vy * t);
    let reach = if threat.missile { range } else { range + 8.0 };
    cx * cx + cy * cy < reach * reach
}

/// A far open point to roam to: the reachable cell with the greatest cost from home, among a
/// fixed sample, changing as the pod moves.
fn far_point(maps: &Maps, seen: &Observation, home: (i32, i32)) -> (i32, i32) {
    let mut best = (home.0, home.1 - DELIVERY_HEIGHT);
    let mut best_cost = 0;
    let step = 7;
    let offset = ((seen.x as i32 / 64) + (seen.y as i32 / 64)) % step;
    for cy in (offset..maps.nav.height).step_by(step as usize * 3) {
        for cx in (offset..maps.nav.width).step_by(step as usize * 3) {
            if maps.nav.clearance((cx, cy)) < 4 {
                continue;
            }
            let cost = maps.home.cost(&maps.nav, (cx, cy));
            if cost != UNREACHED && cost > best_cost {
                best_cost = cost;
                let (x, y) = NavMap::centre((cx, cy));
                best = (x as i32, y as i32);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests;
