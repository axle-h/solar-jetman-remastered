//! The jetpod's logic step (`docs/spec/player.md` 4 and 5, `docs/spec/levels.md` 6.4-6.5).
//!
//! Types are kept general where Jetman and the objects will share them: [`Body`] is a position
//! with sub-pixels and a speed on each axis (player.md 4.9).

use super::clock::STEP;
use super::input::Buttons;
use super::motion::{self, Axis, ONE, PX, UNIT};
use super::tables::{ATAN, SIN, THRUST, TRIANGLE};
use super::terrain::{Surface, Window};
use super::tow::{Moment, Tow};
use crate::game::level::{Level, PLANETS, World};
use crate::game::sound::{Channel, Request};

/// Player states (player.md 3). The final level's 6 and the gate and interlude states 9 and 10
/// are not simulated.
pub const STATE_POD: u8 = 1;
pub const STATE_JETMAN_FLYING: u8 = 2;
pub const STATE_JETMAN_WALKING: u8 = 3;
pub const STATE_POD_EXPLODING: u8 = 4;
pub const STATE_JETMAN_DYING: u8 = 5;
pub const STATE_FINAL: u8 = 6;
pub const STATE_RESPAWN: u8 = 7;
pub const STATE_ASSEMBLING: u8 = 8;
pub const STATE_GATE: u8 = 9;

/// The pod rest's speed (`docs/scaled.md`, "Pod rest"): an upright pod landing on a flat
/// floor slower than this comes to rest, Jetman's soft landing.
pub const REST_BELOW: u32 = 2 << 24;

/// Fuel of a full tank (player.md 1).
pub const FULL_FUEL: u16 = 0xE000;
/// What anti-gravity takes from the 16-bit weapon gauge each step (player.md 4.1).
const ANTIGRAVITY_DRAIN: u16 = 0x10;
/// RAM $D1 for the fitted anti-gravity (economy.md 6.4).
pub const ANTIGRAVITY_DEVICE: u8 = 2;

/// Speed cap of the pod after gravity (player.md 4.2) and of thrust (4.6).
const CAP_POD: u32 = 4 * PX;
const CAP_THRUST: u32 = 3 * PX;

/// A position and a speed (player.md 4.9) in the Scaled engine's formats: whole pixels with
/// a 16-bit fraction (Q16.16), and speeds as magnitudes in Q8.24 px per step with a direction
/// each (`docs/scaled.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Body {
    pub x: u16,
    pub x_sub: u16,
    pub y: u16,
    pub y_sub: u16,
    /// Horizontal speed, and whether it points left.
    pub h_mag: u32,
    pub h_left: bool,
    /// Vertical speed, and whether it points down.
    pub v_mag: u32,
    pub v_down: bool,
}

impl Body {
    /// The speeds in the Original's 8.8 form.
    pub fn h_speed(&self) -> u16 {
        (self.h_mag >> 16).min(0xFFFF) as u16
    }

    pub fn v_speed(&self) -> u16 {
        (self.v_mag >> 16).min(0xFFFF) as u16
    }

    pub fn set_h_speed(&mut self, speed: u16) {
        self.h_mag = u32::from(speed) << 16;
    }

    pub fn set_v_speed(&mut self, speed: u16) {
        self.v_mag = u32::from(speed) << 16;
    }

    /// The speeds as axes (left and up negative).
    pub fn h_axis(&self) -> Axis {
        Axis {
            magnitude: self.h_mag,
            negative: self.h_left,
        }
    }

    pub fn v_axis(&self) -> Axis {
        Axis {
            magnitude: self.v_mag,
            negative: !self.v_down,
        }
    }

    pub fn set_h_axis(&mut self, axis: Axis) {
        (self.h_mag, self.h_left) = (axis.magnitude, axis.negative);
    }

    pub fn set_v_axis(&mut self, axis: Axis) {
        (self.v_mag, self.v_down) = (axis.magnitude, !axis.negative);
    }

    /// The Original's 8-bit sub-pixel fractions.
    pub fn x_fraction(&self) -> u8 {
        (self.x_sub >> 8) as u8
    }

    pub fn y_fraction(&self) -> u8 {
        (self.y_sub >> 8) as u8
    }

    /// The position in Q16.16 px.
    pub fn x_q16(&self) -> u32 {
        u32::from(self.x) << 16 | u32::from(self.x_sub)
    }

    pub fn y_q16(&self) -> u32 {
        u32::from(self.y) << 16 | u32::from(self.y_sub)
    }

    /// A whole step's move by the speeds, x first, then y. For the player the y clamps of
    /// player.md 4.9 and the map bottom (4.2) apply; `bottom_page` is RAM $3A, the highest y
    /// page.
    pub fn advance(&mut self, player: bool, bottom_page: u8) {
        let (dx, dy) = (self.h_axis().signed() >> 8, self.v_axis().signed() >> 8);
        self.shift(dx, dy, player, bottom_page);
    }

    /// Moves by (`dx`, `dy`) in Q16.16 px, with the player's y clamps when `player`.
    pub fn shift(&mut self, dx: i64, dy: i64, player: bool, bottom_page: u8) {
        let x = i64::from(self.x_q16()) + dx;
        self.x = ((x >> 16) & 0xFFFF) as u16;
        self.x_sub = x as u16;
        let y = i64::from(self.y_q16()) + dy;
        self.y_sub = y as u16;
        if !player {
            self.y = ((y >> 16) & 0xFFFF) as u16;
            return;
        }
        let page = y >> 24;
        if y < 0 {
            // The high byte went negative.
            self.y = 0;
            self.v_mag = 0;
        } else if page == 0 && ((y >> 16) as u16) < 16 {
            self.y = 16;
            self.v_mag = 0;
        } else {
            self.y = (y >> 16) as u16;
        }
        if (self.y >> 8) > u16::from(bottom_page) {
            self.y = u16::from(bottom_page) << 8 | 0xFF;
            self.v_mag = 0;
            self.y_sub = 0;
        }
    }

    fn move_by(&mut self, dx: i32, dy: i32) {
        self.x = (i32::from(self.x) + dx) as u16;
        self.y = (i32::from(self.y) + dy) as u16;
    }
}

/// What the pod carries that changes the physics (player.md 4.4, 4.5, 4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Equipment {
    /// RAM $DC: bits 0-1 the pod type, bit 6 boosters owned, bit 7 shields owned.
    pub owned: u8,
    /// RAM $D3: the fitted engine or shield item (8 super shields, 9 momentum killer,
    /// 10 efficient engines, 11 double strength thrusters), 0 none.
    pub device: u8,
    /// RAM $050E: the bullet upgrade, 0-2 (a shot has this + 1 hit points of strength).
    pub bullet_upgrade: u8,
    /// RAM $D1: the fitted weapon or device, the shop number + 1 (1 homing missiles, 5 time
    /// bombs, 6 multi warhead, 8 star bullets), 0 none (economy.md 6).
    pub weapon: u8,
}

impl Equipment {
    pub fn has_shields(self) -> bool {
        self.owned & 0x80 != 0
    }
    pub fn has_boosters(self) -> bool {
        self.owned & 0x40 != 0
    }
}

/// What the pod's step needs to know about the level and the moment.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub world: &'a World,
    pub level: &'a Level,
    /// RAM $5D, the frame counter (low byte is enough).
    pub frame: u8,
    /// Game time at the end of the tick, which the contact cooldowns count from.
    pub now: u64,
    /// The tick within its logic step ([`Moment::STEP`] for the step logic).
    pub moment: Moment,
}

/// The controller as one tick of the player sees it (`super::controls`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickInput {
    pub held: Buttons,
    /// Newly pressed in this tick.
    pub pressed: Buttons,
    /// The facing change: -1, 0 or 1.
    pub turn: i8,
    /// The gun fires.
    pub fire: bool,
}

/// Terrain contact kinds, for their cooldowns (`docs/scaled.md`).
const FLAT_FLOOR: usize = 0;
const FLOOR_SLOPE: usize = 1;
const WALL: usize = 2;
const FLAT_CEILING: usize = 3;
const CEILING_SLOPE: usize = 4;

/// Something the player's step asks the rest of the game to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    /// The pod blew up (player.md 7.1): jetpods - 1, the explosion in slot 1.
    PodDestroyed,
    /// Jetman was killed (player.md 7.2): lives 1 become 0, the explosion in slot 1.
    JetmanKilled,
    /// The death timer ran out: transition 4 (player.md 7.2).
    JetmanLost,
    /// Jetman touched a spare pod (player.md 6.3) and began assembling it.
    SparePodFound,
    /// The new pod is ready after assembling: jetpods + 1 (player.md 6.3).
    PodAssembled,
}

/// The jetpod (player slot 0, state 1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pod {
    /// The infinite-fuel cheat (`Cheats`): hits and crashes take no fuel, but otherwise happen
    /// as ever. Set by the gameplay each pass; not part of a saved state.
    #[serde(skip)]
    pub free_fuel: bool,
    /// The port menu's pod rest (`docs/scaled.md`): an upright pod landing
    /// on a flat floor slower than this (Q8.24 px per step) comes to rest; `None`, off. Set by
    /// the program each tick; not part of a saved state.
    #[serde(skip)]
    pub rest_below: Option<u32>,
    /// Resting on a flat floor.
    #[serde(default)]
    pub resting: bool,
    /// The level routine's gravity override RAM $0595 (levels.md 6.6); 0 uses the header.
    pub gravity_override: u8,
    pub body: Body,
    /// The player state, RAM $0317.
    pub state: u8,
    /// Facing, the sprite frame RAM $0374, 0-31.
    pub facing: u8,
    /// Fuel, RAM $C7:$C6.
    pub fuel: u16,
    pub equipment: Equipment,
    /// Movement angle at the start of the step, RAM $9C.
    pub move_angle: u8,
    /// Hit-pause counter RAM $99, and invulnerability timer RAM $97.
    pub hit_pause: u8,
    /// The palette bits RAM $98 bits 0-1, ORed into the pod's and Jetman's sprite attributes:
    /// the momentum killer's, anti-gravity's and the gate's flashes and the hit pause's palette
    /// 1 (game-flow.md 9.2, "Bits 0-1").
    pub palette_bits: u8,
    pub invulnerable: u8,
    /// Shield on (RAM $65 bit 7).
    pub shield: bool,
    /// Thrust sub-fraction accumulators, RAM $C4 (horizontal) and $C5 (vertical), in Q.24
    /// below one unit of speed (the Original's byte is the top 8 bits).
    pub accumulator_h: u32,
    pub accumulator_v: u32,
    /// Fuel used by thrust and not yet taken from the tank, Q16 of a unit.
    #[serde(default)]
    fuel_owed: u32,
    /// Per contact kind: when the next touch that is not closing on the surface gets a full
    /// response, and when the last impact effect was (game time; `docs/scaled.md`).
    #[serde(default)]
    next_response: [u64; 5],
    #[serde(default)]
    last_effect: [Option<u64>; 5],
    /// The fractions of the shield window's growth (Q16 px).
    #[serde(default)]
    width_grown: u16,
    #[serde(default)]
    height_grown: u16,
    /// Jetman on foot keeps braking after his speed turned round until this game time
    /// (`docs/scaled.md`).
    #[serde(default)]
    pub(super) brake_until: u64,
    /// Surfaces touched by the last terrain test, RAM $DD.
    pub surface: u8,
    /// Width and half-height used by the terrain test, RAM $F1 and $F2.
    pub width: u8,
    pub half_height: u8,
    /// Impact effects of the last step that call for sparks (player.md 5.5), each as the carry
    /// that enters the first spark's rotation of the random byte, and the sparks' life. The
    /// sparks are objects and not made yet; only their slots and their use of the random
    /// generator are (`Scaled::pass`).
    /// The camera of the last step on the final level, which limits the pod's flight.
    pub route_camera: crate::game::level::Point,
    /// The route has ended (the camera sways); the camera's vertical move RAM, which the pod's
    /// step clears and the screen limits may set (player.md 9.1, "After the route's end").
    pub route_end: bool,
    pub route_my: i32,
    /// The final level's horizontal direction byte: 0 right, 5 left, 1 or 4 after a bounce.
    pub h_byte: u8,
    pub spark_requests: Vec<(u8, u8, (u16, u16))>,
    /// The sound requests the pod made since the pass took them (sound.md 8).
    pub sfx: Vec<Request>,
    /// Where the pod was just before the terrain test's last position correction.
    contact: Option<(u16, u16)>,
    /// The pod fires a shot this step (player.md 10).
    pub fire: bool,
    /// The shot comes from an autofire repeat: A counts as newly pressed this step, also in
    /// the controller byte the random generator reads (player.md 4.1).
    pub autopress: bool,
    /// The autofire countdown RAM $EF (player.md 4.1).
    autofire: u8,
    /// Whether the last terrain window lay in chunk 0, RAM $D4 = 0 (see `window_in_sky`).
    pub in_sky: bool,
    /// The size of the last terrain collision lists, RAM $B8 (graphics.md 5.7.3).
    pub list_size: u16,
    /// The index table RAM $A8-$B7 is out of its resting order 0-15: some list of directions 0-14
    /// holds a segment (a segment in direction 15 alone leaves the table as it is;
    /// game-flow.md 7.1).
    pub index_table_busy: bool,
    /// The byte the star job left for the next horizontal thrust (player.md 4.6, case 2).
    pub camera_carry: u8,
    /// The fuel gauge's display copy RAM $05CE (economy.md 3.1), 0 at the launch.
    pub fuel_shown: u8,
    /// The weapon energy RAM $C9 (high byte), full at the launch.
    pub weapon_energy: u8,
    /// The weapon gauge's low byte RAM $C8 (the gauge is 16 bits wide; costs take the high byte).
    pub weapon_low: u8,
    /// Anti-gravity switched on (RAM $D2 bit 7, player.md 4.7).
    pub antigravity: bool,
    /// The exhaust flame (player.md 4.8): the stored offset `d` while slot 1 is in use.
    pub flame: Option<u8>,
    /// This step's call of the flame routine: B held, and the frame counter.
    #[serde(skip)]
    pub flame_call: Option<(bool, u8)>,
    /// The way Jetman looks, RAM $0336 bits 1-2: Some(true) facing left (player.md 6.1, 6.2).
    pub look: Option<bool>,
    /// RAM $0393 of slot 0: the shooting-pose timer, the death timer, the respawn particle
    /// counter.
    pub timer: u8,
    /// RAM $03B2 of slot 0: the explosion and assembly counter.
    pub counter: u8,
    /// A new value for the rotating job counter RAM $60 (player.md 7.1).
    pub job_request: Option<u8>,
    /// What this step asks the rest of the game to do.
    pub events: Vec<Event>,
    /// The exhaust flame was freed in this step (its picture is then the base alone, player.md
    /// 8.3 b, X = 32).
    pub flame_freed: bool,
    /// The pod is drawn behind the mothership: RAM $98 bit 5 (economy.md 2.2).
    pub bay: bool,
    /// A respawn particle has reached the pod: it is visible (and flickers) from then on.
    pub materialised: bool,
    /// Slope responses of the last terrain test: the fraction byte of the new horizontal speed
    /// before halving, and the whole part of it before the clamp (player.md 4.6).
    pub(super) slope: SlopeEffect,
}

/// What the slope responses of one step leave behind for the thrust carry-ins (player.md 4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct SlopeEffect {
    floor: bool,
    ceiling: bool,
    /// Fraction byte of the later response's new horizontal speed.
    fraction: u8,
    /// Whole part (0-15) of the ceiling response's new horizontal speed.
    whole: u8,
}

/// A speed of `magnitude` (Q8.24 px per step) with direction `positive` changed by the signed
/// `amount`: in the same direction it adds; against it, it subtracts, and what would go below 0
/// leaves 0 with the direction turned (the rest is discarded). Returns the new magnitude and
/// direction.
fn pull_axis(magnitude: u32, positive: bool, amount: i64) -> (u32, bool) {
    if amount == 0 {
        return (magnitude, positive);
    }
    let by = amount.unsigned_abs() as u32;
    if (amount > 0) == positive {
        (magnitude + by, positive)
    } else if magnitude >= by {
        (magnitude - by, positive)
    } else {
        (0, !positive)
    }
}

impl Pod {
    /// The pod as gameplay starts on `level`: at the home position 3 px higher, leaving the
    /// mothership at 1.80 up (observed in the original; the specs do not give the launch
    /// speed), full tank, facing up.
    pub fn at_level_start(level: &Level) -> Pod {
        Pod {
            gravity_override: 0,
            body: Body {
                x: level.home.x,
                y: level.home.y - 3,
                // Observed: a level entered by a transition (second levels, the final level)
                // starts at 0.40 up, a planet's launch at 1.80 up.
                v_mag: if usize::from(level.number) < crate::game::level::PLANETS {
                    0x0180 << 16
                } else {
                    0x0040 << 16
                },
                ..Body::default()
            },
            state: STATE_POD,
            facing: 0,
            fuel: FULL_FUEL,
            equipment: Equipment::default(),
            move_angle: 0,
            hit_pause: 0,
            palette_bits: 0,
            invulnerable: 0,
            shield: false,
            accumulator_h: 0,
            accumulator_v: 0,
            fuel_owed: 0,
            next_response: [0; 5],
            last_effect: [None; 5],
            width_grown: 0,
            height_grown: 0,
            brake_until: 0,
            surface: 0,
            width: 6,
            half_height: 6,
            route_camera: crate::game::level::Point::default(),
            route_end: false,
            route_my: 0,
            h_byte: 0,
            spark_requests: Vec::new(),
            sfx: Vec::new(),
            contact: None,
            fire: false,
            autopress: false,
            autofire: 0,
            in_sky: false,
            list_size: 16,
            index_table_busy: false,
            camera_carry: 0,
            flame: None,
            flame_call: None,
            look: None,
            timer: 0,
            counter: 0,
            job_request: None,
            events: Vec::new(),
            flame_freed: false,
            bay: false,
            materialised: false,
            fuel_shown: 0,
            weapon_energy: 0xE0,
            weapon_low: 0,
            free_fuel: false,
            rest_below: None,
            resting: false,
            antigravity: false,
            slope: SlopeEffect::default(),
        }
    }

    /// One tick of the player's motion (`docs/scaled.md`): the pod, or
    /// Jetman flying or walking. The rest of the player's logic is [`Pod::step`]'s.
    pub fn tick(&mut self, ctx: &Context, input: &TickInput, tow: &mut Tow) {
        let taken_in = self.state == STATE_POD && tow.holding() && tow.taken_in;
        if self.fuel == 0 && !taken_in {
            // The step logic destroys the pod or kills Jetman.
            return;
        }
        match self.state {
            STATE_POD => self.tick_pod(ctx, input, tow),
            STATE_JETMAN_FLYING => self.tick_flying(ctx, input),
            STATE_JETMAN_WALKING => self.tick_walking(ctx, input),
            STATE_FINAL => self.tick_final(ctx, input),
            _ => {}
        }
    }

    /// The player's logic of one step, all but the motion of [`Pod::tick`] (player.md 4.1,
    /// 4.2, 6, 7): timers, the flame, the shield's upkeep, the state changes. `held` is the
    /// controller as the last tick saw it.
    pub fn step(&mut self, ctx: &Context, held: Buttons, tow: &mut Tow) {
        self.events.clear();
        self.flame_freed = false;
        self.flame_call = None;
        self.job_request = None;
        if !matches!(self.state, 1..=9) {
            self.flame = None;
            return;
        }
        // 4.1 item 5: anti-gravity drains the weapon gauge by $10 a step and switches off when
        // it runs out.
        if self.antigravity {
            self.drain_antigravity();
        }
        // 4.1 item 6: invulnerability keeps the hit pause up.
        if self.invulnerable > 0 {
            self.invulnerable -= 1;
            self.hit_pause = (ctx.frame & 7) + 1;
        }
        if self.state == STATE_POD {
            // The momentum killer's flash, then anti-gravity's (game-flow.md 9.2, "Bits 0-1",
            // 2): palette 2 on two steps in four unless B is held; all four palettes in turn.
            if self.equipment.device == 9 {
                self.palette_bits = if held.has(Buttons::B) {
                    0
                } else {
                    (ctx.frame >> 2 & 1) << 1
                };
            }
            if self.antigravity_active() {
                self.palette_bits = ctx.frame >> 1 & 3;
            }
        }
        match self.state {
            STATE_POD => self.step_pod(ctx, held, tow),
            STATE_JETMAN_FLYING => self.step_flying(ctx, held),
            STATE_JETMAN_WALKING => self.step_walking(ctx, held),
            STATE_POD_EXPLODING | STATE_ASSEMBLING => self.step_explosion(ctx),
            STATE_JETMAN_DYING => self.step_dying(),
            STATE_FINAL => self.step_final(ctx, held),
            // The gate state's own step is the game's (`gate`).
            STATE_GATE => {}
            _ => self.step_respawn(),
        }
        // The gate state's drawing stage follows the game's gate step (`gate`).
        if !matches!(self.state, STATE_POD | STATE_GATE) {
            self.hit_pause_countdown();
        }
    }

    /// The anti-gravity drain (player.md 4.1 item 5): the 16-bit gauge loses $10; one that
    /// would go below 0 stays at 0 and the switch goes off.
    fn drain_antigravity(&mut self) {
        let gauge = u16::from(self.weapon_energy) << 8 | u16::from(self.weapon_low);
        match gauge.checked_sub(ANTIGRAVITY_DRAIN) {
            Some(left) => {
                self.weapon_energy = (left >> 8) as u8;
                self.weapon_low = left as u8;
            }
            None => {
                self.weapon_energy = 0;
                self.weapon_low = 0;
                self.antigravity = false;
                // The flash stops (game-flow.md 9.2, "Bits 0-1", 1).
                self.palette_bits = 0;
            }
        }
    }

    /// Whether the fitted anti-gravity is switched on, which also stops the magnets' force
    /// (behaviour.md 4.9).
    pub fn antigravity_active(&self) -> bool {
        self.antigravity && self.equipment.weapon == ANTIGRAVITY_DEVICE
    }

    /// A special-weapon press (player.md 10.1): the anti-gravity switch toggles with the
    /// device fitted (2; 3 and 7 are sold nowhere and behave the same), and is cleared with
    /// anything else. Returns whether the press was the switch (no shot, no cost).
    pub fn press_special(&mut self) -> bool {
        let switch = matches!(self.equipment.weapon, 2 | 3 | 7);
        self.antigravity = switch && !self.antigravity;
        if switch {
            // The press clears the flash; it starts on the next step (game-flow.md 9.2).
            self.palette_bits = 0;
        }
        switch
    }

    /// 5.4: the hit-pause countdown comes at the end of the step, in the drawing stage, after
    /// the terrain test and the thrust.
    pub(super) fn hit_pause_countdown(&mut self) {
        // Also during the protection that resets it every step (measured: Jetman emerging).
        // While it runs it decides the palette bits: palette 1 from the new value $1F up
        // (game-flow.md 9.2, "Bits 0-1", 3).
        if self.hit_pause > 0 {
            self.hit_pause -= 1;
            self.palette_bits = u8::from(self.hit_pause >= 0x1F);
        }
    }

    /// The pod's step logic (player.md 4.2): an empty tank destroys the pod, unless the
    /// mothership is taking the item in; the flame; the shield's upkeep; the hit pause.
    fn step_pod(&mut self, ctx: &Context, held: Buttons, tow: &mut Tow) {
        if self.fuel == 0 && !(tow.holding() && tow.taken_in) {
            self.destroy_pod(tow);
            return;
        }
        // 4.2 step 11: the exhaust flame (4.8).
        self.update_flame(held, ctx.frame);
        // 4.2 step 12: shield upkeep.
        if self.shield {
            self.fuel = self.fuel.saturating_sub(1);
        }
        self.hit_pause_countdown();
    }

    /// One tick of the pod (player.md 4.2, `docs/scaled.md`): the shield's switches, rotation,
    /// the rope's pull, thrust (acting in the tick that sees B), the momentum killer, gravity
    /// with the load's lift, the caps, the move, the terrain and the rope's swing.
    fn tick_pod(&mut self, ctx: &Context, input: &TickInput, tow: &mut Tow) {
        let f = ctx.moment.f;
        self.switch_shield(input.pressed, ctx);
        if self.shield || input.pressed.has(Buttons::DOWN) {
            // 8.1: the shield, or a new press of Down, lets the item go.
            tow.drop_item(3);
        }
        // 4.3: rotation.
        self.facing = self.facing.wrapping_add(input.turn as u8) & 31;
        if self.resting {
            // Thrust, turning away from upright, a push or the rope end the rest.
            let pushed = self.body.v_mag != 0;
            if input.held.has(Buttons::B) || self.facing != 0 || pushed || tow.active() {
                self.resting = false;
            } else {
                self.tick_resting(ctx, input);
                return;
            }
        }
        // 4.1 item 4: above y = 16 with nothing touched, the pod is sent down.
        if self.body.y < 16 && self.surface == 0 {
            self.body.v_down = true;
            self.body.v_mag = 0x20 * UNIT;
        }
        // 4.1: the movement angle, from the speeds before anything moves.
        self.move_angle = self.movement_angle();
        let before = (self.body.h_axis().signed(), self.body.v_axis().signed());
        let angle = tow.held_angle(ctx.moment);
        let swing = tow.signed_speed();
        // 8.3 gap (measured): the load's pull comes before everything else.
        if tow.active() {
            let (to_right, to_down) = tow.pull_on_pod(angle, f);
            self.nudge(to_right, to_down);
        }
        let (down, a, gravity_byte) = self.gravity_value(ctx.level);
        if input.held.has(Buttons::B) {
            self.thrust(input.held, (down, a, gravity_byte), angle, ctx.moment, tow);
        }
        if self.equipment.device == 9 && !input.held.has(Buttons::B) {
            // Momentum killer (4.7): both speeds shrink by 0.20 a step towards 0.
            let amount = motion::units(0x20, f);
            self.body.h_mag = self.body.h_mag.saturating_sub(amount);
            self.body.v_mag = self.body.v_mag.saturating_sub(amount);
        }
        if !self.antigravity {
            // 4.7: anti-gravity skips the pull, the towing pull with it.
            self.gravity(down, a, angle, ctx.moment, tow);
        }
        self.body.h_mag = self.body.h_mag.min(CAP_POD);
        self.body.v_mag = self.body.v_mag.min(CAP_POD);
        let after = (self.body.h_axis().signed(), self.body.v_axis().signed());
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body.shift(
            motion::displacement(before.0, after.0, f),
            motion::displacement(before.1, after.1, f),
            true,
            bottom_page,
        );
        self.slope = SlopeEffect::default();
        self.terrain_test(ctx);
        // 8.2: the rope swings (the item is placed by the caller, which owns the objects).
        if tow.holding() {
            tow.swing(swing, f);
        }
        if input.fire {
            self.fire = true;
        }
    }

    /// A tick of the pod at rest on a flat floor (decision 8): no gravity and no hop; the
    /// horizontal speed brakes as Jetman's walk does; leaving the floor ends the rest.
    fn tick_resting(&mut self, ctx: &Context, input: &TickInput) {
        let f = ctx.moment.f;
        let before = self.body.h_axis().signed();
        self.body.h_mag = self.body.h_mag.saturating_sub(motion::units(0x20, f));
        let after = self.body.h_axis().signed();
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body
            .shift(motion::displacement(before, after, f), 0, true, bottom_page);
        // Held on the floor as Jetman stands: a pixel lower, caught by the test.
        (self.body.v_mag, self.body.v_down) = (0, true);
        self.body.y = self.body.y.wrapping_add(1);
        self.slope = SlopeEffect::default();
        self.terrain_test(ctx);
        if self.surface & 1 == 0 {
            self.resting = false;
        }
        if input.fire {
            self.fire = true;
        }
    }

    /// The vertical carry-in after the slope responses of the last terrain test (player.md
    /// 4.6): a floor response gives 255 (it wins over a ceiling's), a ceiling alone the whole
    /// part of its new horizontal speed.
    fn slope_carry(&self, carry_v: u8) -> u8 {
        if self.slope.floor {
            0xFF
        } else if self.slope.ceiling {
            self.slope.whole
        } else if self.surface & 1 != 0 {
            // A flat floor's bounce leaves nothing in it (measured in the attract demos).
            0
        } else {
            carry_v
        }
    }

    /// RAM $0336 as the capture reads it: bit 0 moving left, bits 1-2 the way Jetman looks.
    pub fn direction_flags(&self) -> u8 {
        if self.state == STATE_FINAL {
            return self.h_byte & !1 | u8::from(self.body.h_left);
        }
        let look = match self.look {
            Some(left) => 4 | (u8::from(left) << 1),
            None => 0,
        };
        u8::from(self.body.h_left) | look
    }

    /// The fuel gauge's display copy follows the fuel's high byte once per logic step
    /// (economy.md 3.1): up by max(1, gap / 64) (at most 3), down by 1 within a gap of 16, else 3.
    pub fn ease_fuel_display(&mut self) {
        let target = (self.fuel >> 8) as u8;
        let shown = self.fuel_shown;
        self.fuel_shown = if shown < target {
            shown + ((target - shown) / 64).clamp(1, 3)
        } else if shown > target {
            shown - if shown - target <= 16 { 1 } else { 3 }
        } else {
            shown
        };
    }

    /// The exhaust flame's offset `d` (player.md 4.8): set while B is held, then growing every
    /// 8th frame until the flame is gone. `c` is the frame counter.
    pub(super) fn update_flame(&mut self, held: Buttons, c: u8) {
        // Jetman's flame acts on slot 1 itself (`Scaled::jetman_flame`).
        self.flame_call = Some((held.has(Buttons::B), c));
        if held.has(Buttons::B) {
            let boosted = self.equipment.owned & 0x40 != 0 && held.has(Buttons::SELECT);
            // Player.md 4.8: the engine sound, through the second request pair.
            if boosted || c & 2 == 0 {
                self.sfx.push(Request::b(Channel::Noise, 0x10));
            }
            self.flame = Some(if boosted {
                [0, 1, 2, 1, 0, 1, 2, 1][usize::from((c & 0x0E) / 2)]
            } else if c & 2 == 0 {
                2
            } else {
                1
            });
        } else if let Some(d) = self.flame
            && c & 7 == 0
        {
            self.flame = (d < 2).then_some(d + 1);
            self.flame_freed = self.flame.is_none();
        }
    }

    /// The flame's frame before its offset is added, from the facing.
    pub fn flame_base(&self) -> u8 {
        const BASE: [u8; 16] = [
            0x18, 0x18, 0x1B, 0x1B, 0x1E, 0x1E, 0x21, 0x21, 0x24, 0x21, 0x21, 0x1E, 0x1E, 0x1B,
            0x1B, 0x18,
        ];
        BASE[usize::from(self.facing & 31) & 15]
    }

    /// The flame's picture: layout frame (table B), offset from the pod, mirrored, flipped
    /// (player.md 4.8).
    pub fn flame_picture(&self) -> Option<(u8, (i32, i32), bool, bool)> {
        let d = self.flame?;
        let f = usize::from(self.facing & 31);
        let offset = |i: usize| i32::from(super::objects::NOSE[i] as i8);
        Some((
            self.flame_base() + d,
            (offset(f + 8), offset(f)),
            f >= 16,
            (f + 24) & 31 < 16,
        ))
    }

    /// The shield switches (player.md 4.7): Down newly pressed on, Up newly pressed off.
    fn switch_shield(&mut self, pressed: Buttons, ctx: &Context) {
        if ctx.level.is_warp_level() {
            return;
        }
        if pressed.has(Buttons::DOWN) && self.equipment.has_shields() {
            self.shield = true;
        }
        if pressed.has(Buttons::UP) {
            self.shield = false;
            self.width = 6;
            self.half_height = 6;
        }
    }

    /// The pull direction, magnitude per step and thrust carry-in byte of the gravity of this
    /// step (levels.md 6.5).
    fn gravity_value(&self, level: &Level) -> (bool, u16, u8) {
        let y = self.body.y;
        if usize::from(level.number) < PLANETS && y < 64 {
            let value = ((65 - y) * 2) as u8;
            return (true, u16::from(value) / 4, value);
        }
        let byte = if self.gravity_override != 0 {
            self.gravity_override
        } else {
            level.gravity as u8
        };
        let g = i16::from(byte as i8);
        (g >= 0, g.unsigned_abs() / 4, byte)
    }

    /// The per-level routine of the B pass that changes gravity by the pod's position
    /// (levels.md 6.6); it takes effect with the next step's gravity.
    pub fn update_level_gravity(&mut self, level: u8) {
        let (x, y) = (self.body.x, self.body.y);
        self.gravity_override = match level {
            3 if y >= 1280 => 0xF8,
            5 if (2876..=2883).contains(&x)
                && (3328..=3499).contains(&y)
                && !(3436..=3451).contains(&y) =>
            {
                0x7F
            }
            6 if y >= 2560 => 0xFF,
            11 if (2048..=3583).contains(&y) => 0xE8,
            11 if y >= 3584 => 0x20,
            _ => 0,
        };
    }

    /// Gravity of a tick (`down`, `a` per step) and a towed load's effects on the pod and the
    /// swing (player.md 8.3).
    fn gravity(&mut self, down: bool, a: u16, angle: u8, moment: Moment, tow: &mut Tow) {
        let f = moment.f;
        let mut speed = self.body.v_axis();
        motion::gravity(&mut speed, a, !down, f);
        self.body.set_v_axis(speed);
        if tow.active() {
            // 8.3 a and b: the lift on the pod, and gravity's pull on the swing.
            let amount = tow.lift(angle, a, f);
            let adds = Tow::lift_adds(angle, self.body.v_down);
            tow.pull(angle, a, !down, moment);
            self.lighten(amount, adds);
        }
    }

    /// The swinging load changes the vertical speed's magnitude by `amount` (Q8.24, player.md
    /// 8.3 b): it adds with no cap, or subtracts, and a speed that would go below 0 becomes 0
    /// with the direction flipped (the leftover is discarded).
    fn lighten(&mut self, amount: u32, adds: bool) {
        let body = &mut self.body;
        body.v_mag = if adds {
            body.v_mag + amount
        } else if body.v_mag >= amount {
            body.v_mag - amount
        } else {
            body.v_down = !body.v_down;
            0
        };
    }

    /// The load's pull on the speeds, as signed amounts (right and down positive, Q8.24). Spec
    /// gap, measured: like the lift (player.md 8.3 b), a pull against the motion that is larger
    /// than the speed leaves 0 and turns the direction, discarding the rest.
    fn nudge(&mut self, to_right: i64, to_down: i64) {
        let body = &mut self.body;
        let h = pull_axis(body.h_mag, !body.h_left, to_right);
        let v = pull_axis(body.v_mag, body.v_down, to_down);
        (body.h_mag, body.h_left) = (h.0, !h.1);
        (body.v_mag, body.v_down) = v;
    }

    /// The movement angle RAM $9C from the speeds (player.md 5.1), 1/256 turn, 0 = up.
    pub fn movement_angle(&self) -> u8 {
        let (mut h, mut v) = (self.body.h_speed(), self.body.v_speed());
        for shifts in 1..=8 {
            h <<= 1;
            v <<= 1;
            if (h | v) & 0x0800 != 0 || shifts == 8 {
                break;
            }
        }
        let (h, v) = (usize::from(h >> 8), usize::from(v >> 8));
        let (right, down) = (!self.body.h_left, self.body.v_down);
        if v == 0 {
            return if right { 0x40 } else { 0xC0 };
        }
        if h == 0 {
            return if down { 0x80 } else { 0 };
        }
        let e = if h == v {
            0x20
        } else if h > v {
            let e = ATAN[usize::from(TRIANGLE[h]) + v] / 2;
            if down { 0x40 - e } else { e }
        } else {
            let e = ATAN[usize::from(TRIANGLE[v]) + h] / 2;
            if down { e } else { 0x40 - e }
        };
        match (right, down) {
            (true, false) => 0x40u8.wrapping_sub(e),
            (true, true) => 0x80u8.wrapping_sub(e),
            (false, false) => e.wrapping_add(0xC0),
            (false, true) => e.wrapping_add(0x80),
        }
    }

    /// Acceleration per step from a thrust table value (player.md 4.4).
    fn acceleration(&self, table: u8, boost: bool) -> u16 {
        let doubled = self.equipment.device == 11;
        let table = u16::from(table);
        match (boost, doubled) {
            (false, false) => (table / 2) / 2,
            (false, true) => (table / 2) * 2,
            (true, false) => table,
            (true, true) => table * 2,
        }
    }

    /// Fuel use and acceleration of a tick with B held (player.md 4.4-4.6, `docs/scaled.md`):
    /// the cost comes off the tank as game time passes, so a held step costs the same at any
    /// rate. `gravity` is the tick's (down, a, byte): the byte is the vertical carry-in, and a
    /// thrust against gravity is capped lower by the part of the step's gravity still to come.
    /// The horizontal carry-in is 0 (`docs/scaled.md`).
    fn thrust(
        &mut self,
        held: Buttons,
        (gravity_down, gravity_a, gravity_byte): (bool, u16, u8),
        angle: u8,
        moment: Moment,
        tow: &mut Tow,
    ) {
        let f = moment.f;
        let boost = self.equipment.has_boosters() && held.has(Buttons::SELECT);
        let efficient = self.equipment.device == 10;
        let cost: u16 = match (boost, efficient) {
            (false, false) => 2,
            (false, true) => 1,
            (true, false) => 16,
            (true, true) => 8,
        };
        if self.fuel == 0 || self.fuel < cost {
            self.fuel = 0;
            return;
        }
        self.fuel_owed += u32::from(cost) * f;
        let used = (self.fuel_owed >> 16) as u16;
        self.fuel_owed &= 0xFFFF;
        self.fuel = self.fuel.saturating_sub(used);

        let facing = usize::from(self.facing);
        let (tv, th) = (THRUST[facing & 15], THRUST[(facing + 8) & 15]);
        let (mut av, mut ah) = (self.acceleration(tv, boost), self.acceleration(th, boost));
        let (mut carry_v, mut carry_h) = (self.slope_carry(gravity_byte), 0);
        if tow.active() {
            // 8.3 (c): the acceleration of 4.4 is scaled by F, its product's high byte is the
            // acceleration applied and its low byte the carry-in.
            let scale = u16::from(tow.thrust_scale(self.facing));
            let (pv, ph) = (av * scale, ah * scale);
            (av, ah) = (pv >> 8, ph >> 8);
            (carry_v, carry_h) = (pv as u8, ph as u8);
        }
        let thrust_down = (9..=23).contains(&facing);
        let thrust_left = (17..=31).contains(&facing);
        let cap = if !self.antigravity && thrust_down != gravity_down {
            // The Original's speed at its cap is 3.00 less a step of gravity when it pulls
            // against the thrust (`docs/scaled.md`).
            let still = (u64::from(motion::units(gravity_a, ONE)) * u64::from(ONE - f)) >> 16;
            CAP_THRUST - still as u32
        } else {
            CAP_THRUST
        };
        let mut speed = self.body.v_axis();
        motion::thrust(
            &mut speed,
            &mut self.accumulator_v,
            !thrust_down,
            (av, carry_v),
            f,
            true,
            cap,
        );
        self.body.set_v_axis(speed);
        if ah != 0 {
            let mut speed = self.body.h_axis();
            motion::thrust(
                &mut speed,
                &mut self.accumulator_h,
                thrust_left,
                (ah, carry_h),
                f,
                false,
                CAP_THRUST,
            );
            self.body.set_h_axis(speed);
        }
        // 8.3 d: the torque on W, towing.
        tow.torque(angle, self.facing, moment);
    }

    /// The pod is destroyed (player.md 7.1): Jetman climbs out with a full tank.
    fn destroy_pod(&mut self, tow: &mut Tow) {
        tow.drop_item(5);
        self.shield = false;
        self.width = 6;
        self.half_height = 6;
        self.equipment.device = 0;
        self.equipment.weapon = 0;
        self.antigravity = false;
        self.state = STATE_POD_EXPLODING;
        self.fuel = FULL_FUEL;
        self.facing = 0xFF;
        (self.body.h_mag, self.body.v_mag) = (0, 0);
        self.counter = 0;
        self.flame = None;
        self.events.push(Event::PodDestroyed);
    }

    /// Moves the pod out of the terrain (a position correction); the impact effect that
    /// follows makes its sparks where the pod was before.
    fn correct(&mut self, dx: i32, dy: i32) {
        self.contact = Some((self.body.x, self.body.y));
        self.body.move_by(dx, dy);
    }

    /// The impact effect (player.md 5.5): sparks unless the pod is protected.
    pub fn impact_effect(&mut self) {
        // Player.md 5.5: the sound, unless the pod is invulnerable.
        if self.invulnerable == 0 {
            self.sfx.push(Request::a(Channel::Pulse1, 0x02));
            self.sfx.push(Request::b(Channel::Pulse2, 0x04));
        }
        // player.md 5.5: after a wall hit or a flat ceiling the sparks start where the pod was
        // before the correction; after floors and sloped ceilings, where it is after it.
        let at = self.contact.take().unwrap_or((self.body.x, self.body.y));
        if self.invulnerable == 0 && self.hit_pause == 0 {
            let whole = ((self.body.h_speed() >> 8) + (self.body.v_speed() >> 8)) as u8;
            let age = 8u8.wrapping_sub(whole);
            self.spark_requests
                .push(((age >> 4) & 1, age.wrapping_mul(16), at));
        }
    }

    /// Damage from touching an object whose state has flag byte `flags` (behaviour.md 3.1):
    /// the fuel's high byte drops, then the impact effect runs (no x4, no hit pause).
    pub fn hurt_by_object(&mut self, flags: u8) {
        let harm = flags & 0x30;
        let units: Option<u16> = if self.shield {
            match harm {
                0 => Some(0),
                h if h & 0x10 != 0 => Some(0x20),
                _ => Some(0x08),
            }
        } else if harm == 0 {
            Some(0x40)
        } else {
            None
        };
        // behaviour.md 3.1: a shielded touch that costs nothing makes no sound and no sparks.
        if units == Some(0) {
            return;
        }
        if !self.free_fuel {
            self.fuel = match units {
                Some(units) => self.fuel.saturating_sub(units << 8),
                None => 0,
            };
        }
        self.impact_effect();
    }

    /// Fuel lost to an impact of `amount` (player.md 5.3): taken only with fuel left, off
    /// warp levels and outside a hit pause; unshielded it is x4 and starts a hit pause. Each
    /// contact kind has its effects (sound, sparks, damage) at most once per step of game time
    /// (`docs/scaled.md`).
    fn impact(&mut self, ctx: &Context, kind: usize, amount: u16) {
        if self.last_effect[kind].is_some_and(|last| ctx.now < last + STEP) {
            return;
        }
        self.last_effect[kind] = Some(ctx.now);
        self.impact_effect();
        // Jetman takes terrain damage like the pod (measured; economy.md 3.2 says he takes
        // none, which holds only for his soft landings and walking).
        if self.fuel == 0 || ctx.level.is_warp_level() || self.hit_pause != 0 {
            return;
        }
        let amount = u32::from(amount);
        let loss = if self.shield {
            if self.equipment.device == 8 {
                amount / 4
            } else {
                amount / 2
            }
        } else {
            self.hit_pause = 48;
            amount * 4
        };
        if !self.free_fuel {
            self.fuel = u32::from(self.fuel).saturating_sub(loss) as u16;
        }
    }

    /// Whether a touch of contact `kind` gets its full response (the reflection, the halving
    /// and the effects): always when the body is closing on the surface, otherwise once per
    /// step of game time (`docs/scaled.md`, the approach gate with its allowance). At a whole
    /// step a tick it always does, as in the Original.
    fn responds(&mut self, ctx: &Context, kind: usize, closing: bool) -> bool {
        let next = self.next_response[kind];
        let respond = closing || ctx.now >= next;
        if respond {
            self.next_response[kind] = if ctx.now.saturating_sub(next) >= STEP {
                ctx.now + STEP
            } else {
                next + STEP
            };
        }
        respond
    }

    /// The slope response for contact angle `a` (player.md 5.2, levels.md 6.4).
    fn slope_response(&mut self, ctx: &Context, a: u8, ceiling: bool) {
        let kind = if ceiling { CEILING_SLOPE } else { FLOOR_SLOPE };
        let s = self.move_angle;
        let signed = |value: u8| i32::from(value as i8);
        let clamp = |value: i32| value.clamp(-0x38, 0x38) as u8;
        let s_new = if a < 0x80 && s >= 0x80 {
            a.wrapping_add(clamp(signed(a.wrapping_sub(s).wrapping_sub(0x81))))
        } else if s >= 0x80 && s <= a {
            a.wrapping_add(clamp(signed(a.wrapping_sub(s).wrapping_sub(0x80))))
        } else {
            a.wrapping_sub(clamp(signed(s.wrapping_add(0x80).wrapping_sub(a))))
        };
        self.move_angle = s_new;
        let sum = self.body.h_speed().wrapping_add(self.body.v_speed());
        self.impact(ctx, kind, sum.wrapping_mul(2));

        let k = u32::from(((u32::from(sum) * 16) >> 8) as u8);
        let rebuild = |sine: u8| {
            let value = ((u32::from(sine) * k) >> 3) as u16;
            if value >> 8 >= 8 {
                0x0700 | (value & 0xFF)
            } else {
                value
            }
        };
        let raw_h = ((u32::from(SIN[usize::from(s_new & 0x7F)]) * k) >> 3) as u16;
        // What the thrust carry-ins see afterwards (player.md 4.6).
        self.slope.fraction = raw_h as u8;
        if ceiling {
            self.slope.ceiling = true;
            self.slope.whole = (raw_h >> 8) as u8;
        } else {
            self.slope.floor = true;
        }
        let h = rebuild(SIN[usize::from(s_new & 0x7F)]);
        let v = rebuild(SIN[usize::from(0x40u8.wrapping_sub(s_new) & 0x7F)]);
        // The new direction is written whole: the way Jetman looks is lost (measured).
        self.look = None;
        // Halved unless both whole parts are 0 and the fractions sum to $80 or less.
        let small = h >> 8 == 0 && v >> 8 == 0 && (h & 0xFF) + (v & 0xFF) <= 0x80;
        let (h, v) = if small { (h, v) } else { (h >> 1, v >> 1) };
        let body = &mut self.body;
        body.set_h_speed(h);
        body.set_v_speed(v);
        (body.h_left, body.v_down) = match s_new >> 6 {
            0 => (false, false),
            1 => (false, true),
            2 => (true, true),
            _ => (true, false),
        };
    }

    /// The terrain test and its responses (levels.md 6.4) for a tick (`docs/scaled.md`): the
    /// bands follow the tick's own vertical move, a touch gets its reflection and halving only
    /// when closing on the surface (or once per step of game time otherwise), and each kind's
    /// effects come at most once per step. With a whole step a tick it is the Original's test.
    pub(super) fn terrain_test(&mut self, ctx: &Context) {
        let f = ctx.moment.f;
        self.surface = 0;
        self.in_sky = super::terrain::window_in_sky(ctx.world, ctx.level, self.body.x, self.body.y);
        let window = Window::collect(ctx.world, ctx.level, self.body.x, self.body.y);
        self.list_size = window.list_size();
        self.index_table_busy = window.lists[..15].iter().any(|list| !list.is_empty());
        let ow = i32::from(self.width);
        let h = i32::from(self.half_height);
        let px = i32::from(self.body.x & 15) + 16 + ow / 2;
        let py = i32::from(self.body.y & 15) + 20;
        // This tick's vertical move, Q16 px: its whole part sets the floor band.
        let moved = ((u64::from(self.body.v_mag) * u64::from(f)) >> 24) as u32;
        let moved_whole = (moved >> 16) as i32;

        // Floors. At rest the band is the one for a speed of 1.00, as for Jetman standing.
        let moved_whole = if self.resting { 1 } else { moved_whole };
        let b = py + h;
        let t = b - moved_whole - 3;
        let mut bottom = b;
        let state = self.state;
        // A soft landing (levels.md 6.4 step 3): Jetman slower than 2 px per step, with the
        // speed he would have by the end of the step (`docs/scaled.md`).
        let mut landing = false;
        if let Some(Surface::Flat { y, .. }) = window.lists[0].iter().copied().find(
            |s| matches!(*s, Surface::Flat { x0, width, y } if x0 <= px && px <= x0 + width && t <= y && y < b),
        ) {
            self.body.move_by(0, y - bottom);
            bottom = y;
            self.surface |= 1;
            let pull = motion::units(12, ctx.moment.rest);
            let by_step_end = if self.body.v_down {
                self.body.v_mag + pull
            } else {
                self.body.v_mag.saturating_sub(pull)
            };
            let rests = state == STATE_POD
                && self.facing == 0
                && self.body.v_down
                && self.rest_below.is_some_and(|below| self.body.v_mag < below);
            if matches!(state, STATE_JETMAN_FLYING | STATE_JETMAN_WALKING)
                && by_step_end >> 24 < 2
            {
                landing = true;
            } else if self.resting {
                // At rest: held on the floor, nothing more.
            } else if rests {
                // The landing is an impact like a bounce's, but the pod stays.
                self.impact(ctx, FLAT_FLOOR, self.body.v_speed().wrapping_mul(4));
                self.body.v_mag = 0;
                self.resting = true;
            } else if self.responds(ctx, FLAT_FLOOR, self.body.v_down) {
                self.body.v_down = !self.body.v_down;
                self.impact(ctx, FLAT_FLOOR, self.body.v_speed().wrapping_mul(4));
                self.body.set_v_speed(self.body.v_speed() >> 1);
            }
        }
        // levels.md 6.4: the band is one deeper when the y fraction is not smaller than the
        // move's fraction, whichever way the pod moves.
        let fraction_bonus = if self.body.y_fraction() >= (moved >> 8) as u8 {
            2
        } else {
            1
        };
        let t_slope = t + fraction_bonus;
        'floors: for direction in 1..=6u8 {
            for surface in &window.lists[usize::from(direction)] {
                let Surface::Floor {
                    y_low, x0, width, ..
                } = *surface
                else {
                    continue;
                };
                if y_low < t_slope || px < x0 || px > x0 + width {
                    continue;
                }
                let line = surface.line_height(direction, px);
                if line > b {
                    continue;
                }
                self.body.move_by(0, line - bottom);
                self.surface |= match direction {
                    1 => 2,
                    2 => 4,
                    3 | 5 => 8,
                    _ => 16,
                };
                landing = false;
                if state == STATE_JETMAN_WALKING {
                    // Jetman standing: pushed down the slope at 1.xx or 2.xx (levels.md 6.4).
                    let whole = if direction <= 2 { 1 } else { 2 };
                    self.body
                        .set_h_speed(whole << 8 | (self.body.h_speed() & 0xFF));
                    self.body.h_left = direction % 2 == 0;
                    self.body.v_down = true;
                } else {
                    const CONTACT: [u8; 6] = [0x15, 0xEB, 0x20, 0xE0, 0x2A, 0xD6];
                    let contact = CONTACT[usize::from(direction) - 1];
                    if self.responds(ctx, FLOOR_SLOPE, self.closing(contact)) {
                        self.slope_response(ctx, contact, false);
                    }
                }
                break 'floors;
            }
        }
        if landing {
            // No slope hit: Jetman stands (the horizontal speed is kept, player.md 12).
            self.body.v_mag = 0;
            self.state = STATE_JETMAN_WALKING;
            self.body.v_down = true;
        }

        // Walls, with the original P.
        let (wall_top, wall_bottom) = (py - h, py + h - i32::from(state == STATE_JETMAN_WALKING));
        let overlaps = |y0: i32, height: i32| y0 < wall_bottom && y0 + height >= wall_top;
        let right_face = window.lists[7].iter().find_map(|s| match *s {
            Surface::Wall { x, y0, height } if overlaps(y0, height) && x <= px && px < x + 8 => {
                Some(x)
            }
            _ => None,
        });
        let left_face = window.lists[8].iter().find_map(|s| match *s {
            Surface::Wall { x, y0, height }
                if overlaps(y0, height) && x < px + ow / 2 && px + ow / 2 <= x + 8 =>
            {
                Some(x)
            }
            _ => None,
        });
        if right_face.is_some() || left_face.is_some() {
            let closing = if right_face.is_some() {
                self.body.h_left
            } else {
                !self.body.h_left
            };
            let respond = self.responds(ctx, WALL, closing);
            self.contact = Some((self.body.x, self.body.y));
            if let Some(x) = right_face {
                // The Original pushes 1 px out of the right face; a fine tick resolves
                // toward the face instead, so a deep hit is not repeated (`docs/scaled.md`).
                let depth = i64::from((x + 8 - px).max(0)) << 16;
                let w = {
                    let square = (u64::from(f) * u64::from(f)) >> 16;
                    ((square * square) >> 16) as i64
                };
                self.body.shift(
                    w + (i64::from(ONE) - w) * depth / i64::from(ONE),
                    0,
                    false,
                    0,
                );
                if respond {
                    self.body.h_left = false;
                }
            } else if let Some(x) = left_face {
                self.body.move_by(x - (px + ow / 2), 0);
                if respond {
                    self.body.h_left = true;
                }
            }
            self.surface |= 0x20;
            if respond {
                self.look = None;
                self.impact(ctx, WALL, self.body.h_speed().wrapping_mul(4));
                self.body.set_h_speed(self.body.h_speed() >> 1);
            }
        }

        // Ceilings, with the original P.
        let top = py - h;
        let reach = top + moved_whole + 6;
        let flat = window.lists[9].iter().find_map(|s| match *s {
            Surface::Flat { x0, width, y }
                if x0 <= px && px <= x0 + width && top <= y && y <= reach =>
            {
                Some(y)
            }
            _ => None,
        });
        if let Some(y) = flat {
            // The sparks start before the correction (player.md 5.5).
            self.correct(0, y + 1 - top);
            if self.responds(ctx, FLAT_CEILING, !self.body.v_down) {
                self.impact(ctx, FLAT_CEILING, self.body.v_speed().wrapping_mul(4));
                self.body.set_v_speed(self.body.v_speed() >> 1);
                self.body.v_down = true;
            }
        } else {
            'ceilings: for direction in 10..=15u8 {
                for surface in &window.lists[usize::from(direction)] {
                    let Surface::Ceiling {
                        y_top, x0, width, ..
                    } = *surface
                    else {
                        continue;
                    };
                    if y_top > reach || px < x0 || px > x0 + width {
                        continue;
                    }
                    let line = surface.line_height(direction, px);
                    if line < top {
                        continue;
                    }
                    const CONTACT: [u8; 6] = [0x60, 0xA0, 0x55, 0xAA, 0x6A, 0x95];
                    let contact = CONTACT[usize::from(direction) - 10];
                    let respond = self.responds(ctx, CEILING_SLOPE, self.closing(contact));
                    // The sideways push is a pixel a step: a whole pixel with a response,
                    // else the tick's share of it.
                    let push = if respond { ONE } else { f };
                    let push = if direction % 2 == 0 {
                        i64::from(push)
                    } else {
                        -i64::from(push)
                    };
                    self.body.shift(push, 0, false, 0);
                    self.body.move_by(0, line + 1 - top);
                    self.surface |= 0x20;
                    if respond {
                        self.slope_response(ctx, contact, true);
                    }
                    break 'ceilings;
                }
            }
        }

        // The window grows while the shield is on and nothing was touched (4.7), by a pixel a
        // step of game time.
        if self.shield && self.surface == 0 {
            if self.body.y >= 32 && self.width < 12 {
                Self::grow(&mut self.width, &mut self.width_grown, f);
            }
            if self.half_height < 12 && !window.has_flat() {
                Self::grow(&mut self.half_height, &mut self.height_grown, f);
            }
        }
    }

    /// A size that grows by `f` (Q16) of a pixel, keeping the fraction.
    fn grow(size: &mut u8, fraction: &mut u16, f: u32) {
        let total = u32::from(*fraction) + f;
        *size += (total >> 16) as u8;
        *fraction = total as u16;
    }

    /// Whether the speed points into a surface whose contact angle is `contact` (0 up, $40
    /// right): its component along the surface's outward normal is negative.
    fn closing(&self, contact: u8) -> bool {
        let sine = |a: u8| {
            let v = i64::from(SIN[usize::from(a & 0x7F)]);
            if a & 0x80 != 0 { -v } else { v }
        };
        let (h, v) = (
            self.body.h_axis().signed() >> 8,
            self.body.v_axis().signed() >> 8,
        );
        sine(contact) * h - sine(contact.wrapping_add(0x40)) * v < 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::gameplay;
    use crate::game::level::tests::synthetic_world;

    fn context(world: &World, f: u32, now: u64) -> Context<'_> {
        Context {
            world,
            level: &world.levels[0],
            frame: 0,
            now,
            moment: Moment { f, tau: 0, rest: 0 },
        }
    }

    /// The Original's pod and Scaled's, at the same place and speed.
    fn pods(world: &World) -> (gameplay::pod::Pod, Pod) {
        let mut original = gameplay::pod::Pod::at_level_start(&world.levels[0]);
        original.body.h_speed = 0x0123;
        original.body.v_speed = 0x0180;
        original.body.v_down = true;
        let mut scaled = Pod::at_level_start(&world.levels[0]);
        scaled.body.set_h_speed(0x0123);
        scaled.body.set_v_speed(0x0180);
        scaled.body.v_down = true;
        (original, scaled)
    }

    #[test]
    fn a_whole_step_a_tick_moves_as_the_original_step() {
        // No thrust: gravity, the caps, the move and the terrain test are the Original's at
        // f = 1, contacts included.
        let world = synthetic_world();
        let (mut original, mut scaled) = pods(&world);
        let original_ctx = gameplay::pod::Context {
            world: &world,
            level: &world.levels[0],
            frame: 0,
            original_rules: false,
        };
        let mut original_tow = gameplay::tow::Tow::default();
        let mut tow = Tow::default();
        for n in 1..=120 {
            original.step(&original_ctx, Buttons(0), Buttons(0), &mut original_tow);
            let ctx = context(&world, ONE, n * STEP);
            scaled.tick(&ctx, &TickInput::default(), &mut tow);
            scaled.step(&ctx, Buttons(0), &mut tow);
            let (a, b) = (&original.body, &scaled.body);
            assert_eq!(
                (a.x, a.x_fraction, a.y, a.y_fraction),
                (b.x, b.x_fraction(), b.y, b.y_fraction()),
                "step {n}"
            );
            assert_eq!(
                (a.h_speed, a.h_left, a.v_speed, a.v_down),
                (b.h_speed(), b.h_left, b.v_speed(), b.v_down),
                "step {n}"
            );
            assert_eq!(original.fuel, scaled.fuel, "step {n}");
        }
    }

    #[test]
    fn ticks_of_a_step_fly_the_path_of_the_step() {
        let world = synthetic_world();
        let (_, mut whole) = pods(&world);
        // Up and away from the floor: no contact.
        whole.body.v_down = false;
        let mut split = whole.clone();
        let mut tow = Tow::default();
        for n in 1..=10 {
            whole.tick(
                &context(&world, ONE, n * STEP),
                &TickInput::default(),
                &mut tow,
            );
            for i in 1..=4 {
                let ctx = context(&world, ONE / 4, (n - 1) * STEP + i * STEP / 4);
                split.tick(&ctx, &TickInput::default(), &mut tow);
            }
            let distance = |a: u32, b: u32| (i64::from(a) - i64::from(b)).abs();
            assert!(
                distance(whole.body.x_q16(), split.body.x_q16()) <= 8,
                "step {n}: {:?} {:?} {} {}",
                whole.body,
                split.body,
                whole.surface,
                split.surface
            );
            assert!(
                distance(whole.body.y_q16(), split.body.y_q16()) <= 8,
                "step {n}: {:?} {:?} {} {}",
                whole.body,
                split.body,
                whole.surface,
                split.surface
            );
            assert_eq!(whole.body.v_mag, split.body.v_mag);
        }
    }

    #[test]
    fn a_held_step_of_thrust_costs_the_same_at_any_rate() {
        let world = synthetic_world();
        for ticks in [1u32, 2, 3, 4, 5] {
            let (_, mut pod) = pods(&world);
            // No impact takes fuel.
            pod.free_fuel = true;
            let mut tow = Tow::default();
            let input = TickInput {
                held: Buttons(Buttons::B),
                ..TickInput::default()
            };
            let fuel = pod.fuel;
            let mut done = 0;
            for i in 1..=10 * ticks {
                let end = u64::from(i) * STEP / u64::from(ticks);
                let f = ((end >> 16) - (done >> 16)) as u32;
                done = end;
                pod.tick(&context(&world, f, end), &input, &mut tow);
            }
            assert_eq!(fuel - pod.fuel, 20, "{ticks} ticks a step");
        }
    }

    #[test]
    fn an_empty_tank_destroys_the_pod_at_the_step() {
        let world = synthetic_world();
        let (_, mut pod) = pods(&world);
        pod.fuel = 0;
        let mut tow = Tow::default();
        let ctx = context(&world, ONE, STEP);
        pod.tick(&ctx, &TickInput::default(), &mut tow);
        assert_eq!(pod.state, STATE_POD);
        pod.step(&ctx, Buttons(0), &mut tow);
        assert_eq!(pod.state, STATE_POD_EXPLODING);
        assert_eq!(pod.events, [Event::PodDestroyed]);
    }

    #[test]
    fn with_the_rest_switch_an_upright_landing_stays_down_until_thrust() {
        let world = synthetic_world();
        for rest in [None, Some(REST_BELOW)] {
            let (_, mut pod) = pods(&world);
            pod.body.set_h_speed(0);
            pod.rest_below = rest;
            let mut tow = Tow::default();
            let mut landed = false;
            for n in 1..=240 {
                let ctx = context(&world, ONE / 4, n * STEP / 4);
                pod.tick(&ctx, &TickInput::default(), &mut tow);
                landed |= pod.surface & 1 != 0;
            }
            assert!(landed);
            // Off, the Original's hop goes on; on, the pod lies still.
            assert_eq!(pod.resting, rest.is_some());
            if rest.is_none() {
                continue;
            }
            assert_eq!(pod.body.v_mag, 0);
            let y = pod.body.y;
            let thrust = TickInput {
                held: Buttons(Buttons::B),
                ..TickInput::default()
            };
            pod.tick(&context(&world, ONE / 4, 241 * STEP / 4), &thrust, &mut tow);
            assert!(!pod.resting);
            for n in 242..=260 {
                pod.tick(&context(&world, ONE / 4, n * STEP / 4), &thrust, &mut tow);
            }
            assert!(pod.body.y < y, "thrust lifts it off");
        }
    }

    #[test]
    fn a_touch_not_closing_on_a_wall_responds_once_a_step() {
        let world = synthetic_world();
        let (_, mut pod) = pods(&world);
        let ctx = |now| context(&world, ONE / 4, now);
        // Not closing: once, then not again until a step has passed; closing: always.
        assert!(pod.responds(&ctx(STEP / 4), WALL, false));
        assert!(!pod.responds(&ctx(STEP / 2), WALL, false));
        assert!(pod.responds(&ctx(3 * STEP / 4), WALL, true));
        assert!(!pod.responds(&ctx(STEP), WALL, false));
        assert!(pod.responds(&ctx(2 * STEP), WALL, false));
    }
}
