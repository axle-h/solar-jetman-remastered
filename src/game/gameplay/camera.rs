//! The camera (`docs/spec/levels.md` 5.2): placed from the player at the start of play, then
//! moved in half-steps in both passes of a logic step.

use crate::game::level::{Level, Point};

/// Band the player stays inside on screen (levels.md 5.2).
const BAND_RIGHT: i32 = 144;
const BAND_LEFT: i32 = 112;
const BAND_DOWN: i32 = 96;
const BAND_UP: i32 = 64;

/// The camera: the world position of the playfield's top-left pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub position: Point,
    /// The player's position change over the A pass's player handling, per axis (the 8-bit
    /// difference of the low bytes); both passes move the camera by halves of it.
    pub delta: (i8, i8),
}

impl Camera {
    /// The camera at the start of play for a player at (`x`, `y`) (levels.md 5.2).
    pub fn at_start(level: &Level, x: u16, y: u16) -> Camera {
        let byte8 = u16::from(level.width.saturating_sub(1)) >> 1;
        let byte9 = u16::from(level.height.saturating_sub(1)) >> 1;
        let mut cx = x.saturating_sub(128);
        if cx >> 8 >= byte8 {
            cx = 256 * byte8;
        }
        let mut cy = y.saturating_sub(80);
        if cy >> 8 >= byte9 {
            cy = 256 * byte9 + 48;
        }
        Camera {
            position: Point {
                x: cx & !31,
                y: cy & !31,
            },
            delta: (0, 0),
        }
    }

    /// Notes how far the player moved in this A pass, from the low bytes before and after.
    pub fn set_delta(&mut self, before: (u8, u8), x: u16, y: u16) {
        self.delta = (
            (x as u8).wrapping_sub(before.0) as i8,
            (y as u8).wrapping_sub(before.1) as i8,
        );
    }

    /// One pass's camera move: `second` is false in the A pass (half of the change, rounded
    /// toward zero) and true in the B pass (the rest). Applied per axis only if the player is
    /// outside the band in the direction of the step. Returns the recorded half-step
    /// (zero where the camera arrived at a limit).
    pub fn half_step(&mut self, level: &Level, second: bool, player: (u16, u16)) -> (i32, i32) {
        let half = |d: i8| {
            let first = d / 2;
            let step = if second { d - first } else { first };
            // A half-step of -9 is let through; anything lower becomes -8 (levels.md 5.2).
            i32::from(match step {
                9.. => 8,
                ..=-10 => -8,
                s => s,
            })
        };
        let (dx, dy) = (half(self.delta.0), half(self.delta.1));
        let screen_x = i32::from(player.0) - i32::from(self.position.x);
        let screen_y = i32::from(player.1) - i32::from(self.position.y);
        let on = |screen: i32| (0..=255).contains(&screen);
        let mut x = i32::from(self.position.x);
        let mut y = i32::from(self.position.y);
        if on(screen_x) && ((dx > 0 && screen_x >= BAND_RIGHT) || (dx < 0 && screen_x < BAND_LEFT))
        {
            x += dx;
        }
        if on(screen_y) && ((dy > 0 && screen_y >= BAND_DOWN) || (dy < 0 && screen_y < BAND_UP)) {
            y += dy;
        }
        let limit = level.camera_limit();
        let before = self.position;
        // The recorded half-step (RAM $50, $51) is zeroed in the frame in which the camera
        // arrives at a limit (levels.md 5.2, "Limits"), and when the band holds the camera.
        let (mut recorded_x, mut recorded_y) = (0, 0);
        let (mut nx, mut ny) = (x, y);
        let moved_x = nx != i32::from(before.x);
        let moved_y = ny != i32::from(before.y);
        if moved_x {
            if nx < 0 {
                nx = 0;
            } else if nx >> 8 >= i32::from(limit.x) >> 8 {
                nx = i32::from(limit.x);
            } else {
                recorded_x = dx;
            }
        }
        if moved_y {
            if ny < 0 {
                ny = 0;
            } else if ny > i32::from(limit.y) {
                ny = i32::from(limit.y);
            } else {
                recorded_y = dy;
            }
        }
        self.position = Point {
            x: nx as u16,
            y: ny as u16,
        };
        (recorded_x, recorded_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    #[test]
    fn start_position_rounds_to_32() {
        let world = synthetic_world();
        let mut level = world.levels[0].clone();
        level.width = 8;
        level.height = 8;
        // levels.md 5.2: level 0 (635, 191) gives (480, 96); the far corner is limited.
        assert_eq!(
            Camera::at_start(&level, 635, 191).position,
            Point { x: 480, y: 96 }
        );
        assert_eq!(
            Camera::at_start(&level, 10, 10).position,
            Point { x: 0, y: 0 }
        );
        assert_eq!(
            Camera::at_start(&level, 1000, 1000).position,
            Point { x: 768, y: 800 }
        );
    }

    #[test]
    fn half_steps_split_the_change_and_obey_the_band() {
        let world = synthetic_world();
        let mut level = world.levels[0].clone();
        level.width = 8;
        level.height = 8;
        let mut camera = Camera {
            position: Point { x: 100, y: 100 },
            delta: (5, -5),
        };
        // Player far right and far up of the band: both axes move.
        camera.half_step(&level, false, (100 + 200, 100 + 10));
        assert_eq!(camera.position, Point { x: 102, y: 98 });
        camera.half_step(&level, true, (100 + 200, 100 + 10));
        assert_eq!(camera.position, Point { x: 105, y: 95 });
        // Inside the band nothing moves, and the half-step is lost.
        camera.half_step(&level, false, (105 + 128, 95 + 80));
        assert_eq!(camera.position, Point { x: 105, y: 95 });
    }
}
