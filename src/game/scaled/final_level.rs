//! The final level, 26 (`docs/spec/levels.md` 5.2, `docs/spec/player.md` 9.1,
//! `docs/spec/behaviour.md` 4.16): the camera's route, the pod's flight in state 6 and the
//! objects that stand in the way.

use super::motion::{self, PX};
use super::pod::{Context, Pod, STATE_JETMAN_DYING, TickInput};
use crate::game::gameplay::final_level::{
    BOTTOM, CORE_FRAMES, EFFECT_X, EFFECT_Y, LEFT, RIGHT, SPLIT, TOP,
};
pub use crate::game::gameplay::final_level::{Effect, FINAL_LEVEL, Route, TerrainColours};
use crate::game::gameplay::input::Buttons;

impl Pod {
    /// The carry of the route code (player.md 9.1 item 10), each axis separately; both give
    /// a speed of at least 2.00.
    pub fn carry(&mut self, held: Buttons, mx: i32, my: i32) {
        let body = &mut self.body;
        if !held.has(Buttons::UP) && !held.has(Buttons::DOWN) && my != 0 {
            let d = my > 0;
            if body.v_down == d {
                if body.v_speed() >> 8 < 2 {
                    body.set_v_speed(0x0200);
                }
            } else {
                body.set_v_speed(body.v_speed().saturating_sub(0x20));
                if body.v_speed() == 0 {
                    body.v_down = d;
                }
            }
        }
        if !held.has(Buttons::LEFT) && !held.has(Buttons::RIGHT) && mx != 0 {
            let d = u8::from(mx < 0);
            if self.h_byte & 1 == d {
                if body.h_speed() >> 8 < 2 {
                    body.set_h_speed(0x0200);
                }
            } else {
                body.set_h_speed(body.h_speed().saturating_sub(0x20));
                if body.h_speed() == 0 {
                    self.h_byte = if (self.h_byte & 0xFE) | d == 0 { 0 } else { 5 };
                    body.h_left = self.h_byte & 1 != 0;
                }
            }
        }
    }

    /// The screen limits of the pod's step (player.md 9.1 item 6), against the camera at the
    /// end of the previous step. After the route's end the vertical limits push the camera.
    fn keep_on_screen(&mut self, end: bool) {
        let (cx, cy) = (
            i32::from(self.route_camera.x),
            i32::from(self.route_camera.y),
        );
        let dx = i32::from(self.body.x) - LEFT - cx;
        if dx < 0 {
            self.body.x = (cx + LEFT) as u16;
        } else if dx >= 208 {
            self.body.x = (cx + RIGHT) as u16;
        }
        let d = i32::from(self.body.y) - TOP - cy;
        if !end {
            if d < 0 {
                self.body.y = (cy + TOP) as u16;
            } else if d >= 160 {
                self.body.y = (cy + BOTTOM) as u16;
            }
            return;
        }
        self.route_my = 0;
        if d < 0 {
            if (256..464).contains(&cy) {
                self.body.y = (cy + TOP) as u16;
            } else {
                self.route_my = i32::from((((d & 0xFF) >> 1) | 0x80) as u8 as i8);
            }
        } else if d >= 160 {
            if (576..768).contains(&cy) && cy & 0xFF >= 0x40 {
                self.body.y = (cy + BOTTOM) as u16;
            } else {
                let excess = d - 160;
                self.route_my = (excess & 0xFF) >> 1;
                if self.route_my == 0 {
                    // A slip of the program: an excess of 0 outside the band corrects to one
                    // pixel too high.
                    let y = cy + BOTTOM - i32::from(excess == 0);
                    self.body.y = y as u16;
                }
            }
        }
    }

    /// The player's step logic on the final level (player.md 9.1): an empty tank kills
    /// Jetman, and his picture.
    pub(super) fn step_final(&mut self, ctx: &Context, held: Buttons) {
        if self.fuel == 0 {
            self.state = STATE_JETMAN_DYING;
            return;
        }
        self.facing = if held.0 & 0x0F != 0 && ctx.frame & 4 != 0 {
            0x2E
        } else {
            0x2D
        };
    }

    /// One tick of the player on the final level (player.md 9.1, `docs/scaled.md`):
    /// eight-way flight without gravity or thrust, its accelerations scaled by the tick, then
    /// the move, the screen limits, the terrain test (any contact except a flat ceiling empties
    /// the tank) and the gun.
    pub(super) fn tick_final(&mut self, ctx: &Context, input: &TickInput) {
        let f = ctx.moment.f;
        let held = input.held;
        if self.h_byte & 1 != u8::from(self.body.h_left) {
            self.h_byte ^= 1;
        }
        let before = (self.body.h_axis().signed(), self.body.v_axis().signed());
        let d_h = if held.has(Buttons::LEFT) {
            Some(5)
        } else if held.has(Buttons::RIGHT) {
            Some(0)
        } else {
            None
        };
        let body = &mut self.body;
        if d_h == Some(self.h_byte) {
            // The Original's "a whole part over 4 drops the fraction" holds it at 5.00.
            body.h_mag = (body.h_mag + motion::units(0x80, f)).min(5 * PX);
        } else {
            body.h_mag = body.h_mag.saturating_sub(motion::units(0xFF, f));
            if body.h_mag == 0 {
                self.h_byte = d_h.unwrap_or(0);
                body.h_left = self.h_byte & 1 != 0;
            }
        }
        let d_v = if held.has(Buttons::UP) {
            Some(false)
        } else if held.has(Buttons::DOWN) {
            Some(true)
        } else {
            None
        };
        if d_v == Some(body.v_down) {
            body.v_mag = (body.v_mag + motion::units(0xFF, f)).min(3 * PX);
        } else {
            body.v_mag = body.v_mag.saturating_sub(motion::units(0x80, f));
            if body.v_mag == 0 {
                body.v_down = d_v.unwrap_or(false);
            }
        }
        let after = (self.body.h_axis().signed(), self.body.v_axis().signed());
        let bottom_page = ctx.level.height.saturating_sub(1) >> 1;
        self.body.shift(
            motion::displacement(before.0, after.0, f),
            motion::displacement(before.1, after.1, f),
            true,
            bottom_page,
        );
        self.keep_on_screen(self.route_end);
        self.slope = super::pod::SlopeEffect::default();
        self.terrain_test(ctx);
        if self.h_byte & 1 != u8::from(self.body.h_left) {
            self.h_byte ^= 1;
        }
        if self.surface != 0 {
            self.fuel = 0;
        }
        if input.fire {
            self.fire = true;
        }
    }
}

// ---- The objects of the final level (behaviour.md 4.16) ---------------------------------------

use super::behave::{home_on, max_speed, near};
use super::objects::{Env, Object, ObjectTable, state};

impl ObjectTable {
    /// The prelude of states $B4-$B6 (behaviour.md 4.16): returns true when the object has
    /// been dealt with this step. In the camera pages $16 and $17 it flies on, and from page
    /// $18 it is gone.
    fn final_prelude(&mut self, slot: usize, env: &Env) -> bool {
        let page = env.camera.x >> 8;
        let o = &mut self.slots[slot];
        if page >= 0x18 {
            o.state = 0;
            return true;
        }
        if page >= 0x16 {
            Object::speed_up(&mut o.speed_x, 0x20);
            o.advance();
            return true;
        }
        false
    }

    /// Types 4F-53 ($B4), 54-58 ($B5) and 59-5D ($B6).
    pub(super) fn step_final_enemy(&mut self, slot: usize, env: &mut Env) {
        if self.final_prelude(slot, env) {
            return;
        }
        let pod = (env.pod.body.x, env.pod.body.y);
        let kind_state = self.slots[slot].state;
        let n = near(&self.slots[slot], &*env.pod);
        let o = &mut self.slots[slot];
        match kind_state {
            0xB4 if o.work6 & 0x80 == 0 => {
                o.work6 += 1;
                if o.work6 < 0x11 {
                    return;
                }
                o.work6 = 0x80;
                o.flags_x = 3;
                o.flags_y = u8::from(n.pod_below);
                o.speed_x = o.speed_x & 0xFF | 0x0200;
                o.speed_y = o.speed_y & 0xFF | 0x0200;
                return;
            }
            0xB5 if o.work6 & 0x80 == 0 => {
                o.speed_x = o.speed_x & 0xFF | 0x0200;
                o.flags_x = 1;
                o.advance_x();
                o.work6 = 0x80;
                return;
            }
            0xB6 if o.work6 & 0x80 == 0 => {
                if env.camera.x < o.x {
                    return;
                }
                o.speed_x = 0x0400 | u16::from(env.rng.out);
                o.flags_x = 0;
                o.frame = o.kind.wrapping_add(0x56);
                o.work6 = 0x80;
                return;
            }
            _ => {}
        }
        home_on(o, pod, 8, max_speed(4));
    }

    /// Type 5E ($B7), the core.
    pub(super) fn step_core(&mut self, slot: usize, env: &mut Env) {
        let f = env.f();
        let (r, r1) = (env.rng.out, env.rng.high);
        let o = &mut self.slots[slot];
        let (home_x, home_y) = (o.speed_x, o.speed_y);
        o.x = o.x.wrapping_add_signed(match o.x.cmp(&home_x) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -1,
        });
        o.y = o.y.wrapping_add_signed(match o.y.cmp(&home_y) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -1,
        });
        o.hit_points = o.hit_points.min(10);
        o.flags_x = 1;
        if (slot as u32 + f) & 6 != 0 {
            return;
        }
        if o.work6 == 0 {
            o.work5 = o.work5.wrapping_add(1);
            if o.work5.is_multiple_of(32) {
                o.work5 = (if slot % 2 == 1 { r1 } else { r }) & 0x1F;
                o.work6 = 1;
            }
        } else {
            o.work6 = (o.work6 + 1) & 7;
        }
        o.frame = CORE_FRAMES[usize::from(o.work6)];
    }

    /// State $21, a piece (behaviour.md 4.16, 5.8).
    pub(super) fn step_piece(&mut self, slot: usize, env: &mut Env) {
        let pod = (env.pod.body.x, env.pod.body.y);
        let o = &mut self.slots[slot];
        o.index = o.index.wrapping_add(1);
        if o.index & 0x80 != 0 {
            o.state = 0;
            return;
        }
        if o.index == 8 {
            o.frame = 0xAF;
        }
        if o.index < 32 {
            home_on(o, pod, 0x10, max_speed(4));
        } else {
            o.advance();
        }
    }

    /// A shot at a final-level object of states $B4-$B6 (behaviour.md 3.5 case 4): at frame $AF
    /// it is destroyed (the caller does it); else its frame returns to $AF and up to n pieces
    /// fly out, n being the frames above $AF. Returns whether the object is to be destroyed.
    pub(super) fn split_object(
        &mut self,
        env_data: &super::objdata::ObjectData,
        slot: usize,
    ) -> bool {
        let o = self.slots[slot];
        let n = usize::from(o.frame.wrapping_sub(0xAF));
        if n == 0 {
            return true;
        }
        let o = &mut self.slots[slot];
        o.frame = 0xAF;
        o.hit_points = 1;
        let (px, py) = (o.x, o.y);
        let mut made = 0;
        for y in (0..8).rev() {
            if made == n {
                break;
            }
            let target = 23 + y;
            if self.slots[target].state != 0 {
                continue;
            }
            let (h, v, fx, fy) = SPLIT[y];
            let piece = self.create(env_data, target, state::PIECE, 0xAF, px, py);
            piece.hit_points = 1;
            piece.work6 = 0x80;
            piece.speed_x = h << 8;
            piece.speed_y = v << 8;
            piece.flags_x = fx;
            piece.flags_y = fy;
            made += 1;
        }
        false
    }

    /// The final level's per-step routine (behaviour.md 4.16): counts the cores, runs the
    /// background effect once the camera has stopped, and tells when the level is over.
    /// `counter` is the frame counter as the step's B pass sees it (odd). Returns true when
    /// the end of the level starts.
    pub fn final_routine(
        &mut self,
        env: &mut Env,
        effect: &mut Effect,
        route_done: bool,
        counter: u32,
    ) -> bool {
        if !route_done {
            return false;
        }
        let cores = (23..=27).filter(|&s| self.slots[s].state >= 6).count() as u8;
        self.slots[30].index = cores;
        if cores == 0 {
            return counter & 0x1E == 0;
        }
        // The idle check, every 8th step.
        if effect.point.is_none() && counter & 0x0E == 0 {
            let r = env.rng.out & 15;
            if r < 12 {
                let row = 512 + i32::from(EFFECT_Y[usize::from(r)]) + 12 - i32::from(env.camera.y);
                if (0x30..=0xCF).contains(&row) {
                    effect.point = Some(r);
                    effect.phase = 0xFF;
                }
            }
        }
        if let Some(point) = effect.point
            && counter & 0x06 == 0
        {
            {
                effect.phase = effect.phase.wrapping_add(1);
                if effect.phase == 2 {
                    let i = usize::from(point);
                    if let Some(free) = (15..=30).find(|&s| self.slots[s].state == 0) {
                        let piece = self.create(
                            env.data,
                            free,
                            state::PIECE,
                            0x33,
                            6656 + 8 * EFFECT_X[i],
                            524 + EFFECT_Y[i],
                        );
                        piece.hit_points = 1;
                        piece.speed_x = 0x0200;
                        piece.flags_x = 1;
                    }
                } else if effect.phase == 5 {
                    effect.point = None;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_pod() -> Pod {
        let world = crate::game::level::tests::synthetic_world();
        let mut pod = Pod::at_level_start(&world.levels[0]);
        pod.body.set_h_speed(0);
        pod.body.h_left = false;
        pod.body.set_v_speed(0);
        pod.body.v_down = true;
        pod.h_byte = 0;
        pod
    }

    fn held(bits: u8) -> Buttons {
        Buttons(bits)
    }

    #[test]
    fn the_vertical_carry_is_two() {
        let mut pod = new_pod();
        pod.carry(held(0), 0, 1);
        assert_eq!((pod.body.v_speed(), pod.body.v_down), (0x0200, true));
    }

    #[test]
    fn the_horizontal_carry_is_two() {
        let mut pod = new_pod();
        pod.carry(held(0), 1, 0);
        assert_eq!(pod.body.h_speed(), 0x0200);
    }

    #[test]
    fn a_held_direction_stops_the_carry_on_its_axis() {
        let mut pod = new_pod();
        pod.carry(held(Buttons::DOWN), 1, 1);
        assert_eq!((pod.body.v_speed(), pod.body.h_speed()), (0, 0x0200));
    }
}
