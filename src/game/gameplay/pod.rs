//! The jetpod's logic step (`docs/spec/player.md` 4 and 5, `docs/spec/levels.md` 6.4-6.5).
//!
//! Types are kept general where Jetman and the objects will share them: [`Body`] is a position
//! with sub-pixels and a speed on each axis (player.md 4.9).

use super::input::Buttons;
use super::tables::{ATAN, SIN, THRUST, TRIANGLE};
use super::terrain::{Surface, Window};
use super::tow::Tow;
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

/// Fuel of a full tank (player.md 1).
pub const FULL_FUEL: u16 = 0xE000;
/// What anti-gravity takes from the 16-bit weapon gauge each step (player.md 4.1).
const ANTIGRAVITY_DRAIN: u16 = 0x10;
/// RAM $D1 for the fitted anti-gravity (economy.md 6.4).
pub const ANTIGRAVITY_DEVICE: u8 = 2;

/// Speed cap of the pod after gravity (player.md 4.2) and of thrust (4.6), as 8.8 values.
const CAP_POD: u16 = 0x0400;
const CAP_THRUST: u16 = 0x0300;

/// A position with sub-pixels and speeds: unsigned 8.8 magnitudes with a direction each
/// (player.md 4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Body {
    pub x: u16,
    pub x_fraction: u8,
    pub y: u16,
    pub y_fraction: u8,
    /// Horizontal speed in 1/256 px per step, and whether it points left.
    pub h_speed: u16,
    pub h_left: bool,
    /// Vertical speed, and whether it points down.
    pub v_speed: u16,
    pub v_down: bool,
}

impl Body {
    /// The position update: x first, then y. For the player the y clamps of player.md 4.9 and
    /// the map bottom (4.2) apply; `bottom_page` is RAM $3A, the highest y page.
    pub fn advance(&mut self, player: bool, bottom_page: u8) {
        let x = (u32::from(self.x) << 8 | u32::from(self.x_fraction)) as i64;
        let dx = i64::from(self.h_speed);
        let x = if self.h_left { x - dx } else { x + dx };
        self.x = ((x >> 8) & 0xFFFF) as u16;
        self.x_fraction = x as u8;

        let y = (i64::from(self.y) << 8) | i64::from(self.y_fraction);
        let dy = i64::from(self.v_speed);
        let y = if self.v_down { y + dy } else { y - dy };
        self.y_fraction = y as u8;
        if !player {
            self.y = ((y >> 8) & 0xFFFF) as u16;
            return;
        }
        let page = y >> 16;
        if y < 0 {
            // The high byte went negative.
            self.y = 0;
            self.v_speed = 0;
        } else if page == 0 && ((y >> 8) as u16) < 16 {
            self.y = 16;
            self.v_speed = 0;
        } else {
            self.y = (y >> 8) as u16;
        }
        if (self.y >> 8) > u16::from(bottom_page) {
            self.y = u16::from(bottom_page) << 8 | 0xFF;
            self.v_speed = 0;
            self.y_fraction = 0;
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
    /// Validation mode: the original's rules where the remake deviates on purpose
    /// ([`super::Gameplay::set_original_rules`], `docs/deviations.md`).
    pub original_rules: bool,
}

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
    /// Thrust sub-fraction accumulators, RAM $C4 (horizontal) and $C5 (vertical).
    pub accumulator_h: u8,
    pub accumulator_v: u8,
    /// Surfaces touched by the last terrain test, RAM $DD.
    pub surface: u8,
    /// Width and half-height used by the terrain test, RAM $F1 and $F2.
    pub width: u8,
    pub half_height: u8,
    /// Position low bytes at the start of the step, for the camera (RAM $42, $43).
    pub start_low: (u8, u8),
    /// Impact effects of the last step that call for sparks (player.md 5.5), each as the carry
    /// that enters the first spark's rotation of the random byte, and the sparks' life. The
    /// sparks are objects and not made yet; only their slots and their use of the random
    /// generator are (`Gameplay::pass`).
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
    /// RAM $09 after the terrain test (player.md 8.3 d).
    torque_scratch: u8,
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

/// A speed of `magnitude` (1/65536 px per step) with direction `positive` changed by the signed
/// `amount`: in the same direction it adds; against it, it subtracts, and what would go below 0
/// leaves 0 with the direction turned (the rest is discarded). Returns the new magnitude and
/// direction.
fn pull_axis(magnitude: u32, positive: bool, amount: i32) -> (u32, bool) {
    if amount == 0 {
        return (magnitude, positive);
    }
    let by = amount.unsigned_abs();
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
                v_speed: if usize::from(level.number) < crate::game::level::PLANETS {
                    0x0180
                } else {
                    0x0040
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
            surface: 0,
            width: 6,
            half_height: 6,
            start_low: (0, 0),
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
            torque_scratch: 0,
            bay: false,
            materialised: false,
            fuel_shown: 0,
            weapon_energy: 0xE0,
            weapon_low: 0,
            free_fuel: false,
            antigravity: false,
            slope: SlopeEffect::default(),
        }
    }

    /// One logic step of the player: everything before the state's handler (player.md 4.1) and
    /// the handler (4.2, 6, 7), in order. `pressed` are the buttons newly pressed this step.
    pub fn step(&mut self, ctx: &Context, held: Buttons, pressed: Buttons, tow: &mut Tow) {
        self.spark_requests.clear();
        self.fire = false;
        self.autopress = false;
        self.events.clear();
        self.flame_freed = false;
        self.flame_call = None;
        self.job_request = None;
        if !matches!(self.state, 1..=9) {
            self.flame = None;
            return;
        }
        // 4.1 item 1: autofire. A held without Select repeats the press every few steps.
        let mut a_pressed = pressed.has(Buttons::A);
        if ctx.level.is_warp_level() {
            a_pressed = false;
        } else if held.has(Buttons::A) && !held.has(Buttons::SELECT) {
            if pressed.has(Buttons::A) {
                self.autofire = match self.equipment.owned & 3 {
                    0 => 6,
                    2 => 2,
                    _ => 4,
                };
            } else {
                self.autofire = self.autofire.saturating_sub(1);
                if self.autofire == 0 {
                    a_pressed = true;
                    self.autopress = true;
                    self.autofire = match self.equipment.owned & 3 {
                        0 => 6,
                        2 => 2,
                        _ => 4,
                    };
                }
            }
        }
        // 8.3 gap (measured): the load's horizontal pull comes before the movement angle is taken.
        if self.state == STATE_POD && tow.active() {
            let (to_right, to_down) = tow.pull_on_pod();
            self.nudge(to_right, to_down);
        }
        // 4.1: the movement angle, from the speeds before anything moves.
        self.move_angle = self.movement_angle();
        self.start_low = (self.body.x as u8, self.body.y as u8);
        // 4.1 item 4: above y = 16 with nothing touched, the pod is sent down.
        if self.body.y < 16 && self.surface == 0 {
            self.body.v_down = true;
            self.body.v_speed = 0x0020;
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
            STATE_POD => self.step_pod(ctx, held, pressed, a_pressed, tow),
            STATE_JETMAN_FLYING => self.step_flying(ctx, held, a_pressed),
            STATE_JETMAN_WALKING => self.step_walking(ctx, held, a_pressed),
            STATE_POD_EXPLODING | STATE_ASSEMBLING => self.step_explosion(ctx),
            STATE_JETMAN_DYING => self.step_dying(),
            STATE_FINAL => self.step_final(ctx, held, a_pressed),
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

    /// The pod's step (player.md 4.2).
    fn step_pod(
        &mut self,
        ctx: &Context,
        held: Buttons,
        pressed: Buttons,
        a_pressed: bool,
        tow: &mut Tow,
    ) {
        // 4.2 step 1 and 7.1: an empty tank destroys the pod, unless the mothership is taking
        // the item in (then it is an ordinary step with no fuel, and the test is repeated).
        if self.fuel == 0 && !(tow.holding() && tow.taken_in) {
            self.destroy_pod(tow);
            return;
        }
        self.switch_shield(held, pressed, ctx);
        if self.shield || pressed.has(Buttons::DOWN) {
            // 8.1: the shield, or a new press of Down, lets the item go.
            tow.drop_item(3);
        }
        let mut carry_v = self.motion_before(ctx, held, tow);
        // The terrain test leaves 20 + (y AND 15) of the position after the move, before any
        // correction, in a scratch byte the torque reads when nothing is towed (8.3 d).
        self.torque_scratch = 20 + (self.body.y & 15) as u8;
        self.slope = SlopeEffect::default();
        self.terrain_test(ctx);
        // The vertical carry-in after slope responses (player.md 4.6): a floor response gives
        // 255 (it wins over a ceiling's), a ceiling alone the whole part of its new horizontal
        // speed.
        carry_v = self.slope_carry(carry_v);
        // 4.3: rotation, Left before Right.
        if held.has(Buttons::LEFT) {
            self.facing = (self.facing + 31) & 31;
        } else if held.has(Buttons::RIGHT) {
            self.facing = (self.facing + 1) & 31;
        }
        self.motion_after(held, carry_v, tow);
        // 4.2 step 11: the exhaust flame (4.8).
        self.update_flame(held, ctx.frame);
        // 4.2 step 12: shield upkeep.
        if self.shield {
            self.fuel = self.fuel.saturating_sub(1);
        }
        // 4.2 step 13: shooting.
        self.fire = a_pressed;
        self.hit_pause_countdown();
    }

    /// The continuous motion before the terrain test: momentum killer, gravity and the towed
    /// load's lift, the speed caps and the move. Returns the thrust's vertical carry-in.
    fn motion_before(&mut self, ctx: &Context, held: Buttons, tow: &mut Tow) -> u8 {
        if self.equipment.device == 9 && !held.has(Buttons::B) {
            // Momentum killer (4.7): both speeds shrink by 0.20 towards 0.
            self.body.h_speed = self.body.h_speed.saturating_sub(0x20);
            self.body.v_speed = self.body.v_speed.saturating_sub(0x20);
        }
        let carry_v = self.gravity(ctx, tow);
        for speed in [&mut self.body.h_speed, &mut self.body.v_speed] {
            if *speed >= CAP_POD {
                *speed = CAP_POD;
            }
        }
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body.advance(true, bottom_page);
        carry_v
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

    /// The continuous motion after the terrain test: thrust with B held and the rope's swing.
    fn motion_after(&mut self, held: Buttons, carry_v: u8, tow: &mut Tow) {
        if held.has(Buttons::B) {
            let slope = self.slope.floor || self.slope.ceiling;
            let carry_h = if slope {
                self.slope.fraction
            } else {
                self.camera_carry
            };
            self.thrust(held, carry_v, carry_h, tow);
        }
        // 8.2: the rope swings and the item is placed (by the caller, which owns the objects).
        if tow.holding() {
            tow.swing();
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
        // Jetman's flame acts on slot 1 itself (`Gameplay::jetman_flame`).
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
    fn switch_shield(&mut self, _held: Buttons, pressed: Buttons, ctx: &Context) {
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

    /// Applies gravity and returns the thrust carry-in byte of this step (the gravity value
    /// as it was when gravity was applied, before the pod moved). A towed load swings and
    /// lightens the pod (player.md 8.3).
    fn gravity(&mut self, ctx: &Context, tow: &mut Tow) -> u8 {
        let (down, a, carry) = self.gravity_value(ctx.level);
        if self.antigravity {
            // 4.7: the pull is skipped, the towing pull with it; the carry-in byte is still set.
            return carry;
        }
        let body = &mut self.body;
        if body.v_down == down {
            body.v_speed += a;
            if body.v_speed >> 8 >= 3 {
                body.v_speed = CAP_THRUST;
            }
        } else if body.v_speed > a {
            body.v_speed -= a;
        } else {
            body.v_speed = a - body.v_speed;
            body.v_down = down;
        }
        if tow.active() {
            // 8.3 a and b: both use the angle and W as the previous step left them.
            let (amount, x) = tow.lift(a);
            // Byte 0 of the table is the pod's own direction, as it stands after the pull and
            // the gravity above.
            tow.sign_bits = tow.sign_bits & !1 | u64::from(self.body.v_down);
            let adds = tow.lift_adds(x, self.body.v_down, ctx.original_rules);
            tow.pull(a, !down);
            self.lighten(amount, adds);
        }
        carry
    }

    /// The swinging load changes the vertical speed's magnitude by `amount` (1/65536 px per
    /// step, player.md 8.3 b): it adds with no cap, or subtracts, and a speed that would go
    /// below 0 becomes 0 with the direction flipped (the leftover is discarded). The low byte
    /// goes to the vertical sub-fraction.
    fn lighten(&mut self, amount: u32, adds: bool) {
        let body = &mut self.body;
        let current = u32::from(body.v_speed) << 8 | u32::from(self.accumulator_v);
        let next = if adds {
            current + amount
        } else if current >= amount {
            current - amount
        } else {
            body.v_down = !body.v_down;
            0
        };
        body.v_speed = (next >> 8) as u16;
        self.accumulator_v = next as u8;
    }

    /// The load's pull on the speeds, as signed amounts (right and down positive, 1/65536 px per
    /// step). Spec gap, measured: like the lift (player.md 8.3 b), a pull against the motion that
    /// is larger than the speed leaves 0 and turns the direction, discarding the rest. The low
    /// bytes go to the sub-fractions.
    fn nudge(&mut self, to_right: i32, to_down: i32) {
        let body = &mut self.body;
        let h = pull_axis(
            u32::from(body.h_speed) << 8 | u32::from(self.accumulator_h),
            !body.h_left,
            to_right,
        );
        let v = pull_axis(
            u32::from(body.v_speed) << 8 | u32::from(self.accumulator_v),
            body.v_down,
            to_down,
        );
        (body.h_speed, self.accumulator_h, body.h_left) = ((h.0 >> 8) as u16, h.0 as u8, !h.1);
        (body.v_speed, self.accumulator_v, body.v_down) = ((v.0 >> 8) as u16, v.0 as u8, v.1);
    }

    /// The movement angle RAM $9C from the speeds (player.md 5.1), 1/256 turn, 0 = up.
    pub fn movement_angle(&self) -> u8 {
        let (mut h, mut v) = (self.body.h_speed, self.body.v_speed);
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

    /// Fuel use and acceleration of a step with B held (player.md 4.4-4.6).
    fn thrust(&mut self, held: Buttons, carry_v: u8, carry_h: u8, tow: &mut Tow) {
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
        self.fuel -= cost;

        let f = usize::from(self.facing);
        let (tv, th) = (THRUST[f & 15], THRUST[(f + 8) & 15]);
        let (mut av, mut ah) = (self.acceleration(tv, boost), self.acceleration(th, boost));
        let (mut carry_v, mut carry_h) = (carry_v, carry_h);
        if tow.active() {
            // 8.3 (c): the acceleration of 4.4 is scaled by F, its
            // product's high byte is the acceleration applied and its low byte the carry-in.
            let scale = u16::from(tow.thrust_scale(self.facing));
            let (pv, ph) = (av * scale, ah * scale);
            (av, ah) = (pv >> 8, ph >> 8);
            (carry_v, carry_h) = (pv as u8, ph as u8);
        }
        let thrust_down = (9..=23).contains(&f);
        let thrust_left = (17..=31).contains(&f);
        let body = &mut self.body;

        // Vertical, every thrust step.
        if thrust_down == body.v_down {
            let (acc, wrapped) = self.accumulator_v.overflowing_add(carry_v);
            self.accumulator_v = acc;
            grow(&mut body.v_speed, av + u16::from(wrapped));
        } else {
            let (acc, borrowed) = self.accumulator_v.overflowing_sub(carry_v);
            self.accumulator_v = acc;
            let a = av + u16::from(borrowed);
            if body.v_speed > a {
                body.v_speed -= a;
            } else {
                body.v_speed = a - body.v_speed;
                body.v_down = thrust_down;
                self.accumulator_v = carry_v;
                // A reversal gains one extra step of acceleration, including the borrow's 1
                // (measured; player.md 4.6 says av).
                grow(&mut body.v_speed, a);
            }
        }

        // Horizontal, only when the (scaled) acceleration is not 0.
        if ah != 0 {
            if thrust_left == body.h_left {
                let (acc, wrapped) = self.accumulator_h.overflowing_add(carry_h);
                self.accumulator_h = acc;
                grow(&mut body.h_speed, ah + u16::from(wrapped));
            } else {
                let (acc, borrowed) = self.accumulator_h.overflowing_sub(carry_h);
                self.accumulator_h = acc;
                let a = ah + u16::from(borrowed);
                if body.h_speed > a {
                    body.h_speed -= a;
                } else {
                    body.h_speed = a - body.h_speed;
                    body.h_left = thrust_left;
                    self.accumulator_h = 0;
                }
            }
        }
        // 8.3 d: the torque on W runs on every thrust step, towed or not; without a load its
        // index is the leftover 20 + (y AND 15) of the terrain test.
        tow.torque(self.facing, self.torque_scratch);
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
        self.body.h_speed = 0;
        self.body.v_speed = 0;
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
            let whole = ((self.body.h_speed >> 8) + (self.body.v_speed >> 8)) as u8;
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
    /// warp levels and outside a hit pause; unshielded it is x4 and starts a hit pause.
    fn impact(&mut self, ctx: &Context, amount: u16) {
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

    /// The slope response for contact angle `a` (player.md 5.2, levels.md 6.4).
    fn slope_response(&mut self, ctx: &Context, a: u8, ceiling: bool) {
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
        let sum = self.body.h_speed.wrapping_add(self.body.v_speed);
        self.impact(ctx, sum.wrapping_mul(2));

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
        let body = &mut self.body;
        (body.h_speed, body.v_speed) = (h, v);
        (body.h_left, body.v_down) = match s_new >> 6 {
            0 => (false, false),
            1 => (false, true),
            2 => (true, true),
            _ => (true, false),
        };
        // Halved unless both whole parts are 0 and the fractions sum to $80 or less.
        let small = h >> 8 == 0 && v >> 8 == 0 && (h & 0xFF) + (v & 0xFF) <= 0x80;
        if !small {
            body.h_speed >>= 1;
            body.v_speed >>= 1;
        }
    }

    /// The terrain test and its responses (levels.md 6.4).
    pub(super) fn terrain_test(&mut self, ctx: &Context) {
        self.surface = 0;
        self.in_sky = super::terrain::window_in_sky(ctx.world, ctx.level, self.body.x, self.body.y);
        let window = Window::collect(ctx.world, ctx.level, self.body.x, self.body.y);
        self.list_size = window.list_size();
        self.index_table_busy = window.lists[..15].iter().any(|list| !list.is_empty());
        let ow = i32::from(self.width);
        let h = i32::from(self.half_height);
        let px = i32::from(self.body.x & 15) + 16 + ow / 2;
        let py = i32::from(self.body.y & 15) + 20;

        // Floors.
        let b = py + h;
        let t = b - i32::from(self.body.v_speed >> 8) - 3;
        let mut bottom = b;
        let state = self.state;
        // A soft landing (levels.md 6.4 step 3): Jetman slower than 2 px per step.
        let mut landing = false;
        if let Some(Surface::Flat { y, .. }) = window.lists[0].iter().copied().find(
            |s| matches!(*s, Surface::Flat { x0, width, y } if x0 <= px && px <= x0 + width && t <= y && y < b),
        ) {
            self.body.move_by(0, y - bottom);
            bottom = y;
            self.surface |= 1;
            if matches!(state, STATE_JETMAN_FLYING | STATE_JETMAN_WALKING)
                && self.body.v_speed >> 8 < 2
            {
                landing = true;
            } else {
                self.body.v_down = !self.body.v_down;
                self.impact(ctx, self.body.v_speed.wrapping_mul(4));
                self.body.v_speed >>= 1;
            }
        }
        // levels.md 6.4: the band is one deeper when the y fraction is not smaller than the
        // speed's fraction, whichever way the pod moves.
        let fraction_bonus = if self.body.y_fraction >= (self.body.v_speed & 0xFF) as u8 {
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
                    self.body.h_speed = whole << 8 | (self.body.h_speed & 0xFF);
                    self.body.h_left = direction % 2 == 0;
                    self.body.v_down = true;
                } else {
                    const CONTACT: [u8; 6] = [0x15, 0xEB, 0x20, 0xE0, 0x2A, 0xD6];
                    self.slope_response(ctx, CONTACT[usize::from(direction) - 1], false);
                }
                break 'floors;
            }
        }
        if landing {
            // No slope hit: Jetman stands (the horizontal speed is kept, player.md 12).
            self.body.v_speed = 0;
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
            if right_face.is_some() {
                self.correct(1, 0);
                self.body.h_left = false;
            } else if let Some(x) = left_face {
                self.correct(x - (px + ow / 2), 0);
                self.body.h_left = true;
            }
            self.surface |= 0x20;
            self.look = None;
            self.impact(ctx, self.body.h_speed.wrapping_mul(4));
            self.body.h_speed >>= 1;
        }

        // Ceilings, with the original P.
        let top = py - h;
        let reach = top + i32::from(self.body.v_speed >> 8) + 6;
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
            self.impact(ctx, self.body.v_speed.wrapping_mul(4));
            self.body.v_speed >>= 1;
            self.body.v_down = true;
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
                    let push = if direction % 2 == 0 { 1 } else { -1 };
                    self.body.move_by(push, line + 1 - top);
                    self.surface |= 0x20;
                    const CONTACT: [u8; 6] = [0x60, 0xA0, 0x55, 0xAA, 0x6A, 0x95];
                    self.slope_response(ctx, CONTACT[usize::from(direction) - 10], true);
                    break 'ceilings;
                }
            }
        }

        // The window grows while the shield is on and nothing was touched (4.7).
        if self.shield && self.surface == 0 {
            if self.body.y >= 32 && self.width < 12 {
                self.width += 1;
            }
            if self.half_height < 12 && !window.has_flat() {
                self.half_height += 1;
            }
        }
    }
}

/// Adds `by` to a speed, which stops at 3.00 once its whole part reaches 3 (player.md 4.6).
fn grow(speed: &mut u16, by: u16) {
    *speed += by;
    if *speed >> 8 >= 3 {
        *speed = CAP_THRUST;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    fn pod_with(h: u16, h_left: bool, v: u16, v_down: bool) -> Pod {
        let world = synthetic_world();
        let mut pod = Pod::at_level_start(&world.levels[0]);
        pod.body.h_speed = h;
        pod.body.h_left = h_left;
        pod.body.v_speed = v;
        pod.body.v_down = v_down;
        pod
    }

    #[test]
    fn position_update_keeps_sub_pixels() {
        // player.md 4.9: 2.80 px per step to the right from fraction $40: 2, 3, 2, 3 ...
        let mut body = Body {
            x: 100,
            x_fraction: 0x40,
            h_speed: 0x0280,
            y: 100,
            ..Body::default()
        };
        let mut xs = Vec::new();
        for _ in 0..4 {
            let before = body.x;
            body.advance(false, 3);
            xs.push(body.x - before);
        }
        assert_eq!(xs, [2, 3, 2, 3]);
        assert_eq!(body.x_fraction, 0x40);
    }

    #[test]
    fn the_player_stops_at_the_ceiling_and_the_map_bottom() {
        let mut body = Body {
            y: 18,
            v_speed: 0x0300,
            ..Body::default()
        };
        body.advance(true, 3);
        assert_eq!((body.y, body.v_speed), (16, 0));
        let mut body = Body {
            y: 0x03FE,
            v_speed: 0x0300,
            v_down: true,
            y_fraction: 0x80,
            ..Body::default()
        };
        body.advance(true, 3);
        assert_eq!((body.y, body.y_fraction, body.v_speed), (0x03FF, 0, 0));
    }

    #[test]
    fn an_empty_tank_destroys_the_pod_unless_an_item_is_being_taken_in() {
        // player.md 7.1: with the item being taken in the step is an ordinary one.
        let world = synthetic_world();
        let level = &world.levels[0];
        let ctx = Context {
            world: &world,
            level,
            frame: 0,
            original_rules: false,
        };
        let mut pod = pod_with(0, false, 0, false);
        pod.fuel = 0;
        let mut tow = Tow {
            slot: 12,
            taken_in: true,
            ..Tow::default()
        };
        pod.step(&ctx, Buttons(0), Buttons(0), &mut tow);
        assert_eq!(pod.state, STATE_POD);
        assert!(tow.holding());
        tow.taken_in = false;
        pod.step(&ctx, Buttons(0), Buttons(0), &mut tow);
        assert_eq!(pod.state, STATE_POD_EXPLODING);
        assert!(
            !tow.holding(),
            "the item is let go; the rope's other bytes stay"
        );
    }

    #[test]
    fn slope_response_worked_example() {
        // player.md 5.2: 0.76 left and 1.67 down, moving at $8F, a direction-2 floor ($EB):
        // 0.E4 left, 0.26 up, and the fuel drops by $0EE8 with a hit pause of 48.
        let world = synthetic_world();
        let level = &world.levels[0];
        let ctx = Context {
            world: &world,
            level,
            frame: 0,
            original_rules: false,
        };
        let mut pod = pod_with(0x0076, true, 0x0167, true);
        pod.fuel = 0xC830;
        pod.move_angle = 0x8F;
        pod.slope_response(&ctx, 0xEB, false);
        assert_eq!(pod.body.h_speed, 0x00E4);
        assert!(pod.body.h_left);
        assert_eq!(pod.body.v_speed, 0x0026);
        assert!(!pod.body.v_down);
        assert_eq!(pod.fuel, 0xB948);
        assert_eq!(pod.hit_pause, 48);
        assert_eq!(pod.move_angle, 0xC7);
    }

    #[test]
    fn the_fuel_gauge_eases_to_the_fuel() {
        let mut pod = pod_with(0, false, 0, false);
        pod.fuel = 0xE000;
        pod.fuel_shown = 0;
        pod.ease_fuel_display();
        // Gap 224: 224 / 64 = 3 up.
        assert_eq!(pod.fuel_shown, 3);
        pod.fuel_shown = 0xDF;
        pod.ease_fuel_display();
        assert_eq!(pod.fuel_shown, 0xE0);
        pod.fuel = 0x1000;
        pod.fuel_shown = 0x21;
        pod.ease_fuel_display();
        assert_eq!(pod.fuel_shown, 0x1E, "a gap over 16 goes down by 3");
        pod.fuel_shown = 0x20;
        pod.ease_fuel_display();
        assert_eq!(pod.fuel_shown, 0x1F, "a gap of 16 or less goes down by 1");
    }

    #[test]
    fn the_flame_lingers_after_b_is_released() {
        let mut pod = pod_with(0, false, 0, false);
        pod.update_flame(Buttons(Buttons::B), 0);
        assert_eq!(pod.flame, Some(2));
        pod.update_flame(Buttons(Buttons::B), 2);
        assert_eq!(pod.flame, Some(1));
        pod.update_flame(Buttons(0), 4);
        assert_eq!(
            pod.flame,
            Some(1),
            "d only grows when the counter AND 7 is 0"
        );
        pod.update_flame(Buttons(0), 8);
        assert_eq!(pod.flame, Some(2));
        pod.update_flame(Buttons(0), 16);
        assert_eq!(pod.flame, None);
    }

    #[test]
    fn movement_angle_follows_the_speeds() {
        assert_eq!(pod_with(0, false, 0x0100, false).movement_angle(), 0x00);
        assert_eq!(pod_with(0, false, 0x0100, true).movement_angle(), 0x80);
        assert_eq!(pod_with(0x0100, false, 0, false).movement_angle(), 0x40);
        assert_eq!(pod_with(0x0100, true, 0, false).movement_angle(), 0xC0);
        assert_eq!(
            pod_with(0x0100, false, 0x0100, false).movement_angle(),
            0x20
        );
        assert_eq!(pod_with(0x0100, true, 0x0100, true).movement_angle(), 0xA0);
    }

    #[test]
    fn gravity_turns_a_slow_climb_around() {
        // levels.md 6.5: 0.05 up with g = 24 (a = 6) becomes 0.01 down.
        let world = synthetic_world();
        let mut level = world.levels[3].clone();
        level.gravity = 24;
        let ctx = Context {
            world: &world,
            level: &level,
            frame: 0,
            original_rules: false,
        };
        let mut pod = pod_with(0, false, 0x0005, false);
        pod.body.y = 500;
        assert_eq!(pod.gravity(&ctx, &mut Tow::default()), 24);
        assert_eq!((pod.body.v_speed, pod.body.v_down), (0x0001, true));
    }

    #[test]
    fn thrust_uses_fuel_and_accelerates_along_the_facing() {
        let world = synthetic_world();
        let ctx = Context {
            world: &world,
            level: &world.levels[0],
            frame: 0,
            original_rules: false,
        };
        let mut pod = pod_with(0, false, 0, true);
        pod.facing = 10;
        pod.body.y = 500;
        let before = pod.fuel;
        pod.step(
            &ctx,
            Buttons(Buttons::B),
            Buttons(Buttons::B),
            &mut Tow::default(),
        );
        assert_eq!(pod.fuel, before - 2);
        // player.md 4.4: facing 10, normal engine: 6 down, 14 right (plus gravity 2 down).
        assert_eq!(pod.body.h_speed, 14);
        assert_eq!(pod.body.v_speed >> 8, 0);
        assert!(!pod.body.h_left && pod.body.v_down);
    }

    #[test]
    fn an_empty_tank_destroys_the_pod() {
        let world = synthetic_world();
        let ctx = Context {
            world: &world,
            level: &world.levels[0],
            frame: 0,
            original_rules: false,
        };
        let mut pod = pod_with(0, false, 0, true);
        pod.fuel = 0;
        pod.step(&ctx, Buttons(0), Buttons(0), &mut Tow::default());
        assert_eq!(pod.state, STATE_POD_EXPLODING);
    }
}
