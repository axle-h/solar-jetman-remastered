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
        }
    }

    /// One tick's camera move (`docs/scaled.md`): the player's whole-pixel move
    /// over the tick from `before`, in two half-moves through the band, each held to the
    /// Original's 8 px a pass scaled to the tick (at least 1 px). Returns the camera's move.
    pub fn tick(
        &mut self,
        level: &Level,
        before: (u16, u16),
        player: (u16, u16),
        f: u32,
    ) -> (i32, i32) {
        let start = self.position;
        let d = (
            i32::from((player.0 as u8).wrapping_sub(before.0 as u8) as i8),
            i32::from((player.1 as u8).wrapping_sub(before.1 as u8) as i8),
        );
        let cap = ((8 * f).div_ceil(1 << 16)).max(1) as i32;
        let first = (d.0 / 2, d.1 / 2);
        for half in [first, (d.0 - first.0, d.1 - first.1)] {
            let half = (half.0.clamp(-cap, cap), half.1.clamp(-cap, cap));
            self.move_in_band(level, half, player);
        }
        (
            i32::from(self.position.x) - i32::from(start.x),
            i32::from(self.position.y) - i32::from(start.y),
        )
    }

    /// Moves the camera by `step` on each axis where the player is outside the band in the
    /// step's direction, within the level's limits.
    fn move_in_band(&mut self, level: &Level, step: (i32, i32), player: (u16, u16)) {
        let screen_x = i32::from(player.0) - i32::from(self.position.x);
        let screen_y = i32::from(player.1) - i32::from(self.position.y);
        let on = |screen: i32| (0..=255).contains(&screen);
        let mut x = i32::from(self.position.x);
        let mut y = i32::from(self.position.y);
        let (dx, dy) = step;
        if on(screen_x) && ((dx > 0 && screen_x >= BAND_RIGHT) || (dx < 0 && screen_x < BAND_LEFT))
        {
            x += dx;
        }
        if on(screen_y) && ((dy > 0 && screen_y >= BAND_DOWN) || (dy < 0 && screen_y < BAND_UP)) {
            y += dy;
        }
        let limit = level.camera_limit();
        if x != i32::from(self.position.x) {
            x = if x < 0 {
                0
            } else if x >> 8 >= i32::from(limit.x) >> 8 {
                i32::from(limit.x)
            } else {
                x
            };
        }
        if y != i32::from(self.position.y) {
            y = y.clamp(0, i32::from(limit.y));
        }
        self.position = Point {
            x: x as u16,
            y: y as u16,
        };
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
    fn a_tick_follows_the_pod_out_of_the_band_in_two_held_half_moves() {
        let world = synthetic_world();
        let mut level = world.levels[0].clone();
        level.width = 8;
        level.height = 8;
        let mut camera = Camera {
            position: Point { x: 100, y: 100 },
        };
        // The pod right of and above the band moves 5 right and 5 up in a whole-step tick:
        // the Original's two halves, 2 then 3.
        let moved = camera.tick(&level, (295, 115), (300, 110), 1 << 16);
        assert_eq!((moved, camera.position), ((5, -5), Point { x: 105, y: 95 }));
        // In a 144 Hz tick each half is held to 2 px.
        let moved = camera.tick(&level, (300, 110), (308, 102), (1 << 16) * 30 / 144);
        assert_eq!(moved, (4, -4));
        // Inside the band nothing moves.
        let moved = camera.tick(&level, (230, 175), (233, 175), 1 << 16);
        assert_eq!(moved, (0, 0));
    }
}
