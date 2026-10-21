//! The original's attract demo autopilot (`docs/spec/game-flow.md` 7.1, "Demo autopilot"): an
//! input generator that steers the pod along a list of waypoints. The remake's own demos are
//! flown by `crate::game::pilot`; this one replays the original's, for `replay-compare --demos`.
//! Its odd steering is kept, so it flies the same path as the original's (`docs/deviations.md`,
//! "Kept on purpose").

use super::input::Buttons;
use super::objdata::DEMO_LIST_STARTS;

/// What the autopilot looks at in an A pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Observed {
    /// The level number (6, 4 or 10).
    pub level: usize,
    /// Something hangs on the tractor beam.
    pub towed: bool,
    /// The pod's position in pixels and its speeds (8.8, magnitudes) with their directions.
    pub x: u16,
    pub y: u16,
    pub h_speed: u16,
    pub v_speed: u16,
    pub moving_left: bool,
    pub moving_down: bool,
    /// The pod's facing, 0-31 (1/32 turn clockwise from up).
    pub facing: u8,
    /// The frame counter's low byte.
    pub counter: u8,
    /// Whether the weapon's target slots 23-30 hold something in state 6 or more, each.
    pub shootable: [bool; 8],
}

/// The steering state that lives between passes (RAM $0510 and $0511).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Autopilot {
    /// The index of the waypoint being flown to, counted in the three lists laid end to end.
    pub waypoint: usize,
    /// Where the level's own list starts in the three lists laid end to end.
    pub base: usize,
    /// The shield timer: after something came near it counts down, one per A pass.
    pub timer: u8,
}

/// The input bytes of one A pass: held and newly pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Output {
    pub held: Buttons,
    pub pressed: Buttons,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mode {
    Free,
    Cruise,
    Brake,
}

impl Autopilot {
    /// A demo on `level` starts at the first waypoint of its list; `timer` is what the
    /// previous demo left (the timer is never reset, `game-flow.md` 7.1).
    pub fn new(level: usize, timer: u8) -> Autopilot {
        let list = match level {
            6 => 0,
            4 => 1,
            _ => 2,
        };
        Autopilot {
            waypoint: DEMO_LIST_STARTS[list],
            base: DEMO_LIST_STARTS[list],
            timer,
        }
    }

    /// Builds the pass's input bytes. `waypoints` is the whole list of 88 (x, y, radius).
    pub fn decide(&mut self, waypoints: &[(u16, u16, u8)], seen: &Observed) -> Output {
        let (mut held, mut pressed) = (0u8, 0u8);
        let (mut distance, mut dx, mut dy);
        loop {
            // Rule 1: the shield switch, only while nothing is towed.
            if !seen.towed {
                if self.timer != 0 {
                    let old = self.timer;
                    self.timer -= 1;
                    if old >= 30 {
                        pressed |= Buttons::DOWN;
                    }
                } else {
                    pressed |= Buttons::UP;
                }
            }
            // Rule 2: the waypoint is reached.
            let (wx, wy, radius) = waypoints
                .get(self.waypoint)
                .copied()
                .unwrap_or_else(|| *waypoints.last().unwrap_or(&(0, 0, 0)));
            dx = i32::from(wx) - i32::from(seen.x);
            dy = i32::from(wy) - i32::from(seen.y);
            distance = dx.abs() + dy.abs();
            let reached =
                dx.abs() < 256 && dy.abs() < 256 && distance < 256 && distance < i32::from(radius);
            if reached && self.waypoint + 1 < waypoints.len() {
                self.waypoint += 1;
            } else {
                break;
            }
        }
        // Rule 3: the mode, from the way the pod moves relative to the waypoint.
        let toward_x = (dx >= 0) == !seen.moving_left;
        let toward_y = (dy < 0) == !seen.moving_down;
        let speed = seen.h_speed.wrapping_add(seen.v_speed);
        let mode = if toward_x && toward_y && speed >= 0x10 {
            if i32::from(speed / 4) >= distance {
                Mode::Brake
            } else {
                Mode::Cruise
            }
        } else {
            Mode::Free
        };
        // Rules 4 and 5: the sector of the waypoint and the facing it wants.
        let sector = sector(dx, dy);
        let wanted = match mode {
            Mode::Brake => (32 - 4 * sector as u8) & 31,
            _ => 4 * sector as u8,
        };
        // Rule 6: steering and thrust.
        let facing = seen.facing & 31;
        if facing != wanted {
            held |= if wanted.wrapping_sub(facing) & 31 < 16 {
                Buttons::RIGHT
            } else {
                Buttons::LEFT
            };
            held |= if seen.towed {
                0x60
            } else {
                match seen.level {
                    4 => 0,
                    10 => 0x40,
                    _ => 0x60,
                }
            };
        } else {
            match mode {
                Mode::Free => held |= 0x60,
                Mode::Cruise => {
                    if !faster(seen, 1) {
                        held |= 0x40;
                    }
                }
                Mode::Brake => {
                    if !faster(seen, 3) {
                        held |= 0x40;
                    }
                }
            }
        }
        // Rule 7: the weapon pulse.
        if seen.counter.is_multiple_of(32) && seen.shootable[usize::from(seen.counter / 32)] {
            let f = if seen.counter & 0x20 != 0 { 0x80 } else { 0xA0 };
            pressed = f;
            held |= f;
        }
        Output {
            held: Buttons(held),
            pressed: Buttons(pressed),
        }
    }
}

/// Whether the pod already moves at `limit` pixels per step: horizontally with the facing on
/// the side it moves toward (never for 0, 8, 16 and 24), or vertically with no direction test.
fn faster(seen: &Observed, limit: u16) -> bool {
    let f = seen.facing & 31;
    let side = if seen.moving_left {
        (17..=23).contains(&f) || (25..=31).contains(&f)
    } else {
        (1..=7).contains(&f) || (9..=15).contains(&f)
    };
    (seen.h_speed >> 8 >= limit && side) || seen.v_speed >> 8 >= limit
}

/// The sector of the waypoint, 0 = up, clockwise (rule 4). The tests keep their quirks: the
/// horizontal test compares low bytes when the high bytes are equal, and doubling loses its
/// carry between the bytes.
fn sector(dx: i32, dy: i32) -> usize {
    let (ax, ay) = (dx.unsigned_abs() as u16, dy.unsigned_abs() as u16);
    let (ax_hi, ax_lo, ay_hi, ay_lo) = (ax >> 8, ax & 255, ay >> 8, ay & 255);
    let diagonal = || match (dx >= 0, dy >= 0) {
        (true, true) => 3,
        (true, false) => 1,
        (false, true) => 5,
        (false, false) => 7,
    };
    if ax >= ay {
        if ax_hi == ay_hi && ax_lo < 2 * ay_lo {
            diagonal()
        } else if dx < 0 {
            6
        } else {
            2
        }
    } else {
        let doubled = ((ax_hi * 2) & 255) << 8 | ((ax_lo * 2) & 255);
        if ay >= doubled {
            diagonal()
        } else if dy < 0 {
            0
        } else {
            4
        }
    }
}

/// How near an object must be to the pod to start the shield timer (a Chebyshev-like test of
/// the spec: both distances and their sum below 256).
pub fn near(dx: i32, dy: i32) -> bool {
    dx.abs() < 256 && dy.abs() < 256 && dx.abs() + dy.abs() < 256
}

/// The demo's state inside gameplay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Demo {
    pub pilot: Autopilot,
    pub waypoints: Vec<(u16, u16, u8)>,
    /// The real controller as it was read last, to see a newly pressed button.
    pub real_held: Buttons,
    /// A button was pressed: the demo is over.
    pub over: bool,
    /// The demo makes its sound requests (an addition; the original's demo drops them).
    #[serde(default)]
    pub sound: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sectors_keep_the_originals_quirks() {
        // The spec's verified outputs (dx, dy to sector): right 2, down-right 3, down 4,
        // up-right 1, up-left 7.
        let cases = [
            ((300, 250), 2),
            ((200, 120), 3),
            ((250, 100), 2),
            ((100, 150), 4),
            ((100, 200), 3),
            ((130, 200), 3),
            ((200, 300), 3),
            ((0, -50), 1),
            ((256, 255), 2),
            ((255, 256), 3),
            ((-200, -250), 7),
        ];
        for ((dx, dy), want) in cases {
            assert_eq!(sector(dx, dy), want, "({dx}, {dy})");
        }
    }

    #[test]
    fn the_first_pass_of_demo_1_steers_right_and_lowers_the_shield() {
        // Level 6 at the start (x 1915, y 191, 1.5 up), the waypoint up and slightly right: the
        // pod cruises (S = 1.5), wants facing 4 while it faces 0: Right + $60 held, Up pressed.
        let waypoints = [(1928, 86, 64)];
        let seen = Observed {
            level: 6,
            towed: false,
            x: 1915,
            y: 191,
            h_speed: 0,
            v_speed: 0x180,
            moving_left: false,
            moving_down: false,
            facing: 0,
            counter: 0x02,
            shootable: [false; 8],
        };
        let mut pilot = Autopilot::new(6, 0);
        let out = pilot.decide(&waypoints, &seen);
        assert_eq!((out.held.0, out.pressed.0), (0x61, 0x08));
        // The pod's step starts the timer at 32; then Down is pressed for three passes.
        pilot.timer = 32;
        let downs: Vec<u8> = (0..5)
            .map(|_| pilot.decide(&waypoints, &seen).pressed.0)
            .collect();
        assert_eq!(downs, [4, 4, 4, 0, 0]);
    }

    #[test]
    fn a_fitted_weapon_costs_gauge_units_and_the_last_shot_empties_it() {
        use crate::game::gameplay::{Gameplay, Seed};
        let world = crate::game::level::tests::synthetic_world();
        let seed = Seed {
            weapon: 8,
            ..Seed::default()
        };
        let mut game = Gameplay::new(&world, 0, seed);
        game.held = Buttons(Buttons::A | Buttons::SELECT);
        // Star bullets cost 32 of the 224 units, taken after the shot.
        assert!(game.fire_special_weapon(&world.objects));
        assert_eq!(game.pod.weapon_energy, 0xC0);
        game.pod.weapon_energy = 20;
        assert!(game.fire_special_weapon(&world.objects));
        assert_eq!(game.pod.weapon_energy, 0);
        // An empty gauge, or no Select, gives the plain bullet.
        assert!(!game.fire_special_weapon(&world.objects));
        game.pod.weapon_energy = 0x80;
        game.held = Buttons(Buttons::A);
        assert!(!game.fire_special_weapon(&world.objects));
    }

    #[test]
    fn the_demo_leaves_the_waypoints_x_high_byte_for_the_thrust() {
        use crate::game::gameplay::{Gameplay, Seed};
        let mut world = crate::game::level::tests::synthetic_world();
        world.objects.demo_waypoints = vec![(1928, 86, 64); 88];
        let mut game = Gameplay::new(&world, 0, Seed::default());
        game.start_demo(&world, 0, Buttons(0), false);
        game.pass(&world, Buttons(0));
        assert_eq!(game.pod.camera_carry, 7);
    }
}
