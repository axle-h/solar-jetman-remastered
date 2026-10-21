//! Jetman outside the pod, the pod's destruction and its assembling (`docs/spec/player.md` 6, 7).
//!
//! These are further handlers of [`Pod`], the player object (slot 0): the pod's own step is in
//! [`super::pod`].

use super::clock::STEP;
use super::input::Buttons;
use super::motion::{self, Axis, PX, UNIT};
use super::pod::{
    Context, Event, FULL_FUEL, Pod, STATE_ASSEMBLING, STATE_JETMAN_DYING, STATE_JETMAN_FLYING,
    STATE_POD, STATE_POD_EXPLODING, TickInput,
};

/// Frames of Jetman (player.md 2): the walk cycle by position, the three flying poses, the
/// shooting pose.
const WALK: [u8; 4] = [0x25, 0x26, 0x27, 0x26];
const POSE_SHOOT: u8 = 0x28;
const POSE_UP: u8 = 0x29;
const POSE_DOWN: u8 = 0x2A;

/// Gravity of Jetman flying: a fixed value of 49, a = 12 per step (levels.md 6.5).
const JETMAN_PULL: u16 = 12;
const CAP: u32 = 3 * PX;
const CAP_SIDEWAYS: u32 = 7 * PX;

/// The steps of the explosion and assembly animation: the counter passes this (player.md 7.1).
const ANIMATION_END: u8 = 0x12;

impl Pod {
    /// Jetman is killed (player.md 7.2).
    fn kill_jetman(&mut self) {
        self.state = STATE_JETMAN_DYING;
        self.timer = 96;
        self.equipment.bullet_upgrade = 0;
        self.facing = 0xFF;
        (self.body.h_mag, self.body.v_mag) = (0, 0);
        self.counter = 0;
        self.flame = None;
        self.events.push(Event::JetmanKilled);
    }

    /// Jetman flying, the step logic (state 2, player.md 6.1): an empty tank kills him, the
    /// shooting pose's timer, the flame.
    pub(super) fn step_flying(&mut self, ctx: &Context, held: Buttons) {
        if self.fuel == 0 {
            self.kill_jetman();
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        self.update_flame(held, ctx.frame);
    }

    /// One tick of Jetman flying (player.md 6.1, `docs/scaled.md`): friction, steering (acting in
    /// the tick that sees the buttons), gravity, the caps, the move and the terrain.
    pub(super) fn tick_flying(&mut self, ctx: &Context, input: &TickInput) {
        let f = ctx.moment.f;
        let before = (self.body.h_axis().signed(), self.body.v_axis().signed());
        self.body.h_mag = self.body.h_mag.saturating_sub(motion::units(2, f));
        self.steer_flying(input.held, f);
        let mut speed = self.body.v_axis();
        motion::gravity(&mut speed, JETMAN_PULL, false, f);
        self.body.set_v_axis(speed);
        self.body.h_mag = self.body.h_mag.min(CAP);
        self.body.v_mag = self.body.v_mag.min(CAP);
        let after = (self.body.h_axis().signed(), self.body.v_axis().signed());
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body.shift(
            motion::displacement(before.0, after.0, f),
            motion::displacement(before.1, after.1, f),
            true,
            bottom_page,
        );
        self.slope = Default::default();
        self.terrain_test(ctx);
        // A sloped floor without a flat one, standing still: pushed up.
        if self.surface & 0x1E != 0 && self.surface & 1 == 0 && self.body.v_mag < PX {
            self.body.v_mag = 0x0180 * UNIT;
            self.body.v_down = false;
        }
        if input.fire && self.state == STATE_JETMAN_FLYING {
            self.facing = POSE_SHOOT;
            self.timer = 8;
            self.fire = true;
        }
    }

    /// The steering of Jetman flying over a tick of fraction `f` (player.md 6.1 steps 5-6).
    /// A turn through 0 keeps the step's whole extra push, as the Original's reversal does
    /// (`docs/scaled.md`).
    fn steer_flying(&mut self, held: Buttons, f: u32) {
        let (left, right) = (held.has(Buttons::LEFT), held.has(Buttons::RIGHT));
        if left || right {
            // Left wins over Right; the mirror image for Right.
            let towards_left = left;
            self.look = Some(towards_left);
            let mut speed = self.body.h_axis();
            if speed.negative != towards_left {
                speed.accelerate(motion::units(0x38, f), towards_left);
            } else {
                speed.magnitude = (speed.magnitude + motion::units(0x28, f)).min(CAP_SIDEWAYS);
            }
            self.body.set_h_axis(speed);
        }
        let shooting = self.timer != 0;
        let body = &mut self.body;
        if held.has(Buttons::DOWN) {
            if !shooting {
                self.facing = POSE_DOWN;
            }
            let amount = motion::units(0x1C, f);
            if !body.v_down {
                if body.v_mag > amount {
                    body.v_mag -= amount;
                } else {
                    body.v_mag = 0;
                    body.v_down = true;
                }
            } else {
                body.v_mag = (body.v_mag + motion::units(0x1A, f)).min(CAP);
            }
        } else if held.has(Buttons::B) {
            if !shooting {
                self.facing = POSE_UP;
            }
            if body.v_down {
                let mut speed = Axis {
                    magnitude: body.v_mag,
                    negative: false,
                };
                if speed.accelerate(motion::units(0x20, f), true) {
                    speed.magnitude += 0x30 * UNIT;
                }
                body.set_v_axis(Axis {
                    magnitude: speed.magnitude,
                    negative: speed.negative,
                });
            } else {
                body.v_mag += motion::units(0x30, f);
            }
            body.v_mag = body.v_mag.min(CAP);
        } else if !shooting {
            self.facing = if body.v_down { POSE_DOWN } else { POSE_UP };
        }
    }

    /// Jetman standing and walking, the step logic (state 3, player.md 6.2).
    pub(super) fn step_walking(&mut self, ctx: &Context, held: Buttons) {
        if self.fuel == 0 {
            self.kill_jetman();
            return;
        }
        self.update_flame(held, ctx.frame);
    }

    /// One tick of Jetman standing and walking (player.md 6.2, `docs/scaled.md`): the floor test,
    /// taking off, the walk and the move.
    pub(super) fn tick_walking(&mut self, ctx: &Context, input: &TickInput) {
        let f = ctx.moment.f;
        let held = input.held;
        if let Some(left) = self.look {
            self.body.h_left = left;
        }
        // Held on the floor: down at 1.00, one pixel lower, then the terrain test catches him
        // (the pixel comes back in the same tick).
        self.body.v_down = true;
        self.body.v_mag = PX;
        self.body.y = self.body.y.wrapping_add(1);
        self.slope = Default::default();
        self.terrain_test(ctx);
        if self.surface & 1 == 0 {
            self.state = STATE_JETMAN_FLYING;
            self.facing = POSE_DOWN;
            self.body.v_mag = self.body.v_mag.max(PX);
            return;
        }
        self.body.v_mag = 0;
        if held.has(Buttons::B) {
            self.state = STATE_JETMAN_FLYING;
            self.body.v_mag = 0x0040 * UNIT;
            self.body.v_down = false;
            return;
        }
        let before = self.body.h_axis().signed();
        let (left, right) = (held.has(Buttons::LEFT), held.has(Buttons::RIGHT));
        let mut speed = self.body.h_axis();
        if left || right {
            if speed.negative != left {
                if speed.accelerate(motion::units(0x28, f), left) {
                    // The rest of the step brakes on (`docs/scaled.md`).
                    self.brake_until = ctx.now.div_ceil(STEP) * STEP;
                }
            } else if ctx.now <= self.brake_until {
                speed.magnitude += motion::units(0x28, f);
            } else if speed.magnitude < PX {
                speed.magnitude = (speed.magnitude + motion::units(0x13, f)).min(PX);
            }
        } else {
            speed.brake(motion::units(0x20, f));
        }
        self.body.set_h_axis(speed);
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        let after = self.body.h_axis().signed();
        self.body
            .shift(motion::displacement(before, after, f), 0, true, bottom_page);
        if input.fire {
            self.fire = true;
        }
        self.facing = WALK[usize::from((self.body.x >> 2) & 3)];
    }

    /// The pod exploding (state 4, player.md 7.1) and Jetman assembling a new pod (state 8,
    /// 6.3, the same animation run backwards).
    pub(super) fn step_explosion(&mut self, ctx: &Context) {
        if self.counter <= ANIMATION_END {
            self.slope = Default::default();
            self.terrain_test(ctx);
            if self.surface == 0 {
                if self.half_height < 9 {
                    self.half_height += 1;
                }
            } else if self.surface & 0x1F != 0 {
                self.body.y = self.body.y.wrapping_sub(1);
            }
            // Measured: the last increment stops at $13, and the next step ends the animation.
            self.counter = (self.counter + 2).min(ANIMATION_END + 1);
            return;
        }
        if self.state == STATE_POD_EXPLODING {
            // Jetman climbs out (player.md 7.1).
            self.state = STATE_JETMAN_FLYING;
            self.facing = POSE_UP;
            self.body.y = self.body.y.wrapping_sub(2);
            self.invulnerable = 96;
            self.bay = false;
            // The end of the explosion clears RAM $98 (game-flow.md 9.2).
            self.palette_bits = 0;
            self.job_request = Some(9);
        } else {
            // The new pod is ready (player.md 6.3).
            debug_assert_eq!(self.state, STATE_ASSEMBLING);
            self.state = STATE_POD;
            self.facing = 8;
            self.width = 6;
            self.half_height = 6;
            self.events.push(Event::PodAssembled);
        }
    }

    /// Jetman touches a spare pod (player.md 6.3).
    pub fn find_spare_pod(&mut self) {
        self.invulnerable = 96;
        self.fuel = FULL_FUEL;
        self.facing = 0xFF;
        (self.body.h_mag, self.body.v_mag) = (0, 0);
        self.counter = 0;
        self.flame = None;
        self.state = STATE_ASSEMBLING;
        self.events.push(Event::SparePodFound);
    }

    /// Jetman dying (state 5, player.md 7.2): the death timer runs, then transition 4.
    pub(super) fn step_dying(&mut self) {
        self.timer = self.timer.wrapping_sub(1);
        if self.timer == 0 {
            self.events.push(Event::JetmanLost);
        }
    }

    /// The new pod materialising (state 7, player.md 7.3): the particles themselves are objects
    /// (in [`super::objects`]); the pod only stays protected.
    pub(super) fn step_respawn(&mut self) {
        self.invulnerable = 128;
    }
}
