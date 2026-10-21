//! The final level, 26 (`docs/spec/levels.md` 5.2, `docs/spec/player.md` 9.1,
//! `docs/spec/behaviour.md` 4.16): the camera's route, the pod's flight in state 6 and the
//! objects that stand in the way.

use super::motion::{self, PX};
use super::pod::{Context, Pod, STATE_JETMAN_DYING, TickInput};
use crate::game::gameplay::input::Buttons;
use crate::game::level::Point;

/// The level the route belongs to.
pub const FINAL_LEVEL: u8 = 26;

/// The moves of the 26 route steps (levels.md 5.2): bits 0-1 give x (01 right, bit 1 left),
/// bits 2-3 give y (bit 2 alone down, bit 3 up); 0 ends the route.
const MOVES: [u8; 26] = [
    0x01, 0x09, 0x01, 0x05, 0x06, 0x05, 0x06, 0x05, 0x01, 0x09, 0x0A, 0x09, 0x0A, 0x09, 0x01, 0x09,
    0x01, 0x05, 0x01, 0x05, 0x06, 0x05, 0x01, 0x09, 0x01, 0x00,
];

/// The waypoints: step i's move applies until the camera x reaches waypoint i.
const WAYPOINTS: [u8; 26] = [
    0x82, 0x84, 0x66, 0x47, 0x86, 0x07, 0xC6, 0x07, 0x8D, 0x0E, 0x0D, 0x8D, 0x0D, 0x8D, 0x8E, 0xCE,
    0x11, 0xD1, 0xD3, 0xD5, 0x95, 0xD5, 0x17, 0x19, 0x1A, 0x3F,
];

/// The camera's x for a waypoint byte.
fn waypoint_x(w: u8) -> u16 {
    256 * u16::from(w & 0x1F) + u16::from(w & 0xE0)
}

/// One axis of the end routine's guide (levels.md 5.2, "End"): a speed (8.8), a direction byte
/// and a position (8.8; only its high byte matters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
struct Guide {
    speed: u16,
    dir: u8,
    pos: u16,
}

impl Guide {
    /// Moves the position by the speed in the direction (1: down or left) and returns the
    /// old position byte.
    fn shift(&mut self) -> u8 {
        let old = (self.pos >> 8) as u8;
        self.pos = if self.dir == 1 {
            self.pos.wrapping_sub(self.speed)
        } else {
            self.pos.wrapping_add(self.speed)
        };
        old
    }
}

/// The camera's route: the step RAM $E0, the end flag (RAM $9A bit 6), the camera's move
/// (mx, my) that the A pass applies first and the B pass applies again (player.md 9.1 item
/// 10-11), and the end routine's guides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Route {
    pub step: u8,
    pub done: bool,
    pub mx: i32,
    pub my: i32,
    vertical: Guide,
    horizontal: Guide,
}

/// The screen limits the pod is kept within, as its offset from the camera (player.md 9.1).
const LEFT: i32 = 24;
const RIGHT: i32 = 232;
const TOP: i32 = 16;
const BOTTOM: i32 = 176;

fn clamp_move(v: i32) -> i32 {
    v.clamp(-3, 3)
}

impl Route {
    /// Moves the camera by the current move; returns it.
    pub fn move_camera(&self, camera: &mut Point) -> (i32, i32) {
        camera.x = camera.x.wrapping_add_signed(self.mx as i16);
        camera.y = camera.y.wrapping_add_signed(self.my as i16);
        (self.mx, self.my)
    }

    /// The route code of the A pass (levels.md 5.2), after the camera's move: the next move
    /// (or, after the route, the end routine's sway). `cores` is the number of objects in
    /// slots 23-27 with a state of 6 or more, `random` RAM $5A, `counter` RAM $5D.
    pub fn run(&mut self, camera: Point, cores: u8, random: u8, counter: u32) {
        if self.done {
            self.sway(camera, cores, random, counter);
            return;
        }
        let waypoint = WAYPOINTS[usize::from(self.step)];
        let target = waypoint_x(waypoint);
        if camera.x == target || camera.x == target + 1 {
            self.step += 1;
        }
        let m = MOVES.get(usize::from(self.step)).copied().unwrap_or(0);
        if m == 0 {
            self.done = true;
            self.step = 0x80;
            (self.mx, self.my) = (0, 0);
            self.vertical = Guide {
                speed: 0x40,
                ..Guide::default()
            };
            self.horizontal = Guide::default();
            return;
        }
        self.mx = match m & 3 {
            1 => 1,
            2 | 3 => -1,
            _ => 0,
        };
        self.my = if m & 8 != 0 {
            -1
        } else if m & 4 != 0 {
            1
        } else {
            0
        };
    }

    /// The end routine (levels.md 5.2, "End").
    fn sway(&mut self, camera: Point, cores: u8, random: u8, counter: u32) {
        let mut n = cores;
        if n == 0 {
            n = (counter & 0x1E) as u8;
            if n == 0 {
                return;
            }
        }
        let low = (camera.x & 255) as u8;
        let v = &mut self.vertical;
        let up = u8::from(camera.y >= 512);
        if up == v.dir {
            v.speed += 4;
            if v.speed >> 8 != 0 {
                v.speed = 0x100;
            }
        } else {
            v.speed = v.speed.saturating_sub(3);
            if v.speed == 0 {
                v.dir ^= 1;
            }
        }
        let old = v.shift();
        if n == 1 {
            let j = match (random & 7).wrapping_sub(4) as i8 {
                0 => 4,
                j => j,
            };
            v.pos = v.pos.wrapping_add_signed(i16::from(j) << 8);
        }
        self.my = clamp_move(i32::from(old.wrapping_sub((v.pos >> 8) as u8) as i8));
        let h = &mut self.horizontal;
        let c = u8::from(low >= 0x1C);
        if c != h.dir {
            h.speed = (h.speed + 3).min(0x300);
        } else {
            h.speed = h.speed.saturating_sub(2);
            if h.speed == 0 {
                h.dir ^= 1;
            }
        }
        let old = h.shift();
        self.mx = clamp_move(i32::from(old.wrapping_sub((h.pos >> 8) as u8) as i8));
        if (self.mx >= 0 && low >= 0x38) || (self.mx < 0 && low < 8) {
            h.dir = u8::from(self.mx < 0);
            h.speed = 0;
        }
    }
}

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

/// The speeds and directions of the pieces a shot object splits into, by slot offset Y
/// (behaviour.md 4.16): horizontal whole speed, vertical whole speed, flags X, flags Y.
const SPLIT: [(u16, u16, u8, u8); 8] = [
    (3, 0, 1, 1),
    (2, 1, 0, 1),
    (1, 2, 1, 0),
    (0, 3, 0, 0),
    (0, 3, 1, 0),
    (1, 2, 1, 1),
    (2, 1, 0, 0),
    (3, 0, 0, 1),
];

/// The frames of the opening core ($B7) by its animation step.
const CORE_FRAMES: [u8; 8] = [0xB4, 0xB5, 0xB6, 0xB6, 0xB6, 0xB6, 0xB6, 0xB5];

/// The points of the background effect (behaviour.md 4.16): x and y of a new piece.
const EFFECT_X: [u16; 12] = [18, 20, 22, 22, 24, 24, 26, 26, 28, 28, 28, 30];
const EFFECT_Y: [u16; 12] = [64, 80, 16, 112, 64, 144, 192, 208, 32, 176, 208, 96];

/// The background effect's state: the active point RAM $EB (None: $80) and the phase RAM $E9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Effect {
    pub point: Option<u8>,
    pub phase: u8,
}

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

    #[test]
    fn the_route_ends_with_the_move_byte_zero() {
        let mut route = Route {
            step: 24,
            ..Route::default()
        };
        route.run(Point { x: 6656, y: 0 }, 5, 0, 0);
        assert!(route.done && route.step == 0x80 && (route.mx, route.my) == (0, 0));
    }
}

/// The hue of the final level's terrain by the frame counter's high nibble (bank 0 $AA82,
/// graphics.md 7.5).
const HUES: [u8; 16] = [3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 0xA, 9, 8, 7, 6, 5, 4];

/// The terrain colours after a core hit, by the number of cores alive (graphics.md 7.5).
const AFTER_HIT: [[u8; 3]; 6] = [
    [0x0F, 0x0F, 0x0F],
    [0x20, 0x10, 0x0F],
    [0x22, 0x12, 0x02],
    [0x37, 0x27, 0x07],
    [0x39, 0x29, 0x09],
    [0x34, 0x24, 0x04],
];

/// Palette entries 9-11 on the final level (graphics.md 7.5): every chunk of the level is in
/// background palette 2, whose colours cycle along the route and flash when a core is hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TerrainColours {
    /// The colours as the program last wrote them (RAM $7F-$81); `None`: the level's own.
    pub shadow: Option<[u8; 3]>,
    /// The colours on screen (uploaded with the palette).
    pub shown: Option<[u8; 3]>,
    /// The step of the core-hit script being played (0-4).
    script: Option<u8>,
}

impl TerrainColours {
    /// The A pass's part (graphics.md 7.5): along the route the hue follows the counter; after
    /// it, a core hit plays the four-step script, then the colours of the cores left.
    pub fn step(&mut self, routed: bool, route_done: bool, counter: u32, hit: bool, cores: u8) {
        if routed && !route_done {
            let h = HUES[usize::from((counter as u8) >> 4)];
            self.shadow = Some([0x30 + h, 0x20 + h, h]);
            return;
        }
        if !route_done {
            return;
        }
        if hit {
            self.script = Some(0);
        }
        let Some(i) = self.script else { return };
        let left = AFTER_HIT[usize::from(cores.min(5))];
        self.shadow = Some(match i {
            0 => [0x20, left[1], 0x3F],
            1 => [0x30, 0x00, 0x3F],
            2 => [0x30, 0x20, 0x3F],
            3 => [0x36, 0x26, 0x16],
            _ => left,
        });
        self.script = (i < 4).then_some(i + 1);
    }

    /// Levels 3 and 11 (levels.md 6.6, "Terrain colours"): every 16th step the camera's depth
    /// picks the colours of background palette 2, the header's near the top and darker ones
    /// below. `at_once` puts them on screen too, as a level load does.
    pub fn by_depth(&mut self, level: u8, camera_y: u16, at_once: bool) {
        let deep = |high: u16| usize::from((camera_y >> 8) >= high);
        let colours = match level {
            3 => DEPTH_COLOURS[deep(3)],
            11 => DEPTH_COLOURS[2 + deep(9)],
            _ => return,
        };
        self.shadow = Some(colours);
        if at_once {
            self.shown = self.shadow;
        }
    }
}

/// Palette entries 9-11 of levels 3 and 11 by depth (levels.md 6.6): level 3 above and below
/// camera y 768, level 11 above and below 2,304.
const DEPTH_COLOURS: [[u8; 3]; 4] = [
    [0x28, 0x16, 0x05],
    [0x0F, 0x32, 0x12],
    [0x38, 0x29, 0x19],
    [0x16, 0x33, 0x13],
];
