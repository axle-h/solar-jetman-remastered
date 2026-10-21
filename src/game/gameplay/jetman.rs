//! Jetman outside the pod, the pod's destruction and its assembling (`docs/spec/player.md` 6, 7).
//!
//! These are further handlers of [`Pod`], the player object (slot 0): the pod's own step is in
//! [`super::pod`].

use super::input::Buttons;
use super::pod::{
    Context, Event, FULL_FUEL, Pod, STATE_ASSEMBLING, STATE_JETMAN_DYING, STATE_JETMAN_FLYING,
    STATE_POD, STATE_POD_EXPLODING,
};

/// Frames of Jetman (player.md 2): the walk cycle by position, the three flying poses, the
/// shooting pose.
const WALK: [u8; 4] = [0x25, 0x26, 0x27, 0x26];
const POSE_SHOOT: u8 = 0x28;
const POSE_UP: u8 = 0x29;
const POSE_DOWN: u8 = 0x2A;

/// Gravity of Jetman flying: a fixed value of 49, a = 12 per step (levels.md 6.5).
const JETMAN_PULL: u16 = 12;
const CAP: u16 = 0x0300;
const CAP_SIDEWAYS: u16 = 0x0700;

/// The steps of the explosion and assembly animation: the counter passes this (player.md 7.1).
const ANIMATION_END: u8 = 0x12;

impl Pod {
    /// Shrinks a speed by `amount`; through 0 the leftover becomes a speed the other way
    /// (the direction flips). Returns whether it flipped.
    fn brake(speed: &mut u16, flag: &mut bool, amount: u16) -> bool {
        if *speed > amount {
            *speed -= amount;
            false
        } else {
            *speed = amount - *speed;
            *flag = !*flag;
            true
        }
    }

    /// Jetman is killed (player.md 7.2).
    fn kill_jetman(&mut self) {
        self.state = STATE_JETMAN_DYING;
        self.timer = 96;
        self.equipment.bullet_upgrade = 0;
        self.facing = 0xFF;
        self.body.h_speed = 0;
        self.body.v_speed = 0;
        self.counter = 0;
        self.flame = None;
        self.events.push(Event::JetmanKilled);
    }

    /// Jetman flying (state 2, player.md 6.1).
    pub(super) fn step_flying(&mut self, ctx: &Context, held: Buttons, a_pressed: bool) {
        if self.fuel == 0 {
            self.kill_jetman();
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        // Gravity with the fixed value, the 3.00 caps, the move and the friction.
        let body = &mut self.body;
        if body.v_down {
            body.v_speed += JETMAN_PULL;
        } else if body.v_speed > JETMAN_PULL {
            body.v_speed -= JETMAN_PULL;
        } else {
            body.v_speed = JETMAN_PULL - body.v_speed;
            body.v_down = true;
        }
        for speed in [&mut body.h_speed, &mut body.v_speed] {
            if *speed >> 8 >= 3 {
                *speed = CAP;
            }
        }
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        body.advance(true, bottom_page);
        body.h_speed = body.h_speed.saturating_sub(2);
        self.slope = Default::default();
        self.terrain_test(ctx);
        // A sloped floor without a flat one, standing still: pushed up.
        if self.surface & 0x1E != 0 && self.surface & 1 == 0 && self.body.v_speed >> 8 == 0 {
            self.body.v_speed = 0x0180;
            self.body.v_down = false;
        }
        self.steer_flying(held);
        if a_pressed {
            self.facing = POSE_SHOOT;
            self.timer = 8;
            self.fire = true;
        }
        self.update_flame(held, ctx.frame);
    }

    /// The steering of Jetman flying (player.md 6.1 steps 5-6).
    fn steer_flying(&mut self, held: Buttons) {
        let body = &mut self.body;
        let (left, right) = (held.has(Buttons::LEFT), held.has(Buttons::RIGHT));
        if left || right {
            // Left wins over Right; the mirror image for Right.
            let towards_left = left;
            self.look = Some(towards_left);
            if body.h_left != towards_left {
                Self::brake(&mut body.h_speed, &mut body.h_left, 0x38);
            } else {
                body.h_speed = (body.h_speed + 0x28).min(CAP_SIDEWAYS);
            }
        }
        let shooting = self.timer != 0;
        if held.has(Buttons::DOWN) {
            if !shooting {
                self.facing = POSE_DOWN;
            }
            if !body.v_down {
                if body.v_speed > 0x1C {
                    body.v_speed -= 0x1C;
                } else {
                    body.v_speed = 0;
                    body.v_down = true;
                }
            } else {
                body.v_speed = (body.v_speed + 0x1A).min(CAP);
                if body.v_speed >> 8 >= 3 {
                    body.v_speed = CAP;
                }
            }
        } else if held.has(Buttons::B) {
            if !shooting {
                self.facing = POSE_UP;
            }
            if body.v_down {
                if Self::brake(&mut body.v_speed, &mut body.v_down, 0x20) {
                    body.v_speed += 0x30;
                }
            } else {
                body.v_speed += 0x30;
            }
            if body.v_speed >> 8 >= 3 {
                body.v_speed = CAP;
            }
        } else if !shooting {
            self.facing = if body.v_down { POSE_DOWN } else { POSE_UP };
        }
    }

    /// Jetman standing and walking (state 3, player.md 6.2).
    pub(super) fn step_walking(&mut self, ctx: &Context, held: Buttons, a_pressed: bool) {
        if self.fuel == 0 {
            self.kill_jetman();
            return;
        }
        if let Some(left) = self.look {
            self.body.h_left = left;
        }
        // Held on the floor: down at 1.00, one pixel lower, then the terrain test catches him.
        self.body.v_down = true;
        self.body.v_speed = 0x0100;
        self.body.y = self.body.y.wrapping_add(1);
        self.slope = Default::default();
        self.terrain_test(ctx);
        if self.surface & 1 == 0 {
            self.state = STATE_JETMAN_FLYING;
            self.facing = POSE_DOWN;
            self.body.v_speed = self.body.v_speed.max(0x0100);
            self.update_flame(held, ctx.frame);
            return;
        }
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body.advance(true, bottom_page);
        if held.has(Buttons::B) {
            self.state = STATE_JETMAN_FLYING;
            self.body.v_speed = 0x0040;
            self.body.v_down = false;
            return;
        }
        let body = &mut self.body;
        let (left, right) = (held.has(Buttons::LEFT), held.has(Buttons::RIGHT));
        if left || right {
            if body.h_left != left {
                Self::brake(&mut body.h_speed, &mut body.h_left, 0x28);
            } else if body.h_speed >> 8 == 0 {
                body.h_speed += 0x13;
                if body.h_speed >> 8 != 0 {
                    body.h_speed = 0x0100;
                }
            }
        } else {
            body.h_speed = body.h_speed.saturating_sub(0x20);
        }
        if a_pressed {
            self.fire = true;
        }
        self.facing = WALK[usize::from((self.body.x >> 2) & 3)];
        self.update_flame(held, ctx.frame);
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
        self.body.h_speed = 0;
        self.body.v_speed = 0;
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
