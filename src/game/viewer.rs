//! A level viewer: a camera moved over a level's map by the player, stepped on the game's
//! fixed logic tick and drawn between steps by interpolation. It is the first use of the
//! step/draw split the game follows: [`LevelViewer::step`] advances one tick, and
//! [`LevelViewer::draw`] draws at a blend between the last two ticks.

use crate::game::graphics::LevelGraphics;
use crate::game::level::{Level, Point, World};
use crate::game::playfield::{Frame, draw_playfield};
use crate::game::view::Viewport;

/// Camera speed of the viewer in pixels per tick.
pub const SCROLL_SPEED: i32 = 4;

/// Directions held this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewerInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

/// The viewer's state: which level and where the camera is, now and one tick ago.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelViewer {
    level: usize,
    camera: Point,
    previous: Point,
}

impl LevelViewer {
    /// Starts on `level` with the camera where the level's header puts it.
    pub fn new(world: &World, level: usize) -> LevelViewer {
        let level = level.min(world.levels.len() - 1);
        let info = &world.levels[level];
        let camera = info.clamp_camera(info.camera_start);
        LevelViewer {
            level,
            camera,
            previous: camera,
        }
    }

    pub fn level(&self) -> usize {
        self.level
    }

    pub fn camera(&self) -> Point {
        self.camera
    }

    fn info<'a>(&self, world: &'a World) -> &'a Level {
        &world.levels[self.level]
    }

    /// One logic tick: moves the camera by the held directions, within the camera limits.
    pub fn step(&mut self, world: &World, input: ViewerInput) {
        self.previous = self.camera;
        let axis = |minus: bool, plus: bool| i32::from(plus) - i32::from(minus);
        let dx = axis(input.left, input.right) * SCROLL_SPEED;
        let dy = axis(input.up, input.down) * SCROLL_SPEED;
        let moved = Point {
            x: (i32::from(self.camera.x) + dx).clamp(0, i32::from(u16::MAX)) as u16,
            y: (i32::from(self.camera.y) + dy).clamp(0, i32::from(u16::MAX)) as u16,
        };
        self.camera = self.info(world).clamp_camera(moved);
    }

    /// Switches to another level (wrapping around), camera at its start.
    pub fn change_level(&mut self, world: &World, delta: isize) {
        let count = world.levels.len() as isize;
        let level = (self.level as isize + delta).rem_euclid(count) as usize;
        *self = LevelViewer::new(world, level);
    }

    /// The camera between the previous tick (`blend` 0) and the current one (`blend` 1),
    /// rounded to whole pixels.
    pub fn camera_at(&self, blend: f32) -> Point {
        let blend = blend.clamp(0.0, 1.0);
        let mix = |from: u16, to: u16| {
            (f32::from(from) + (f32::from(to) - f32::from(from)) * blend).round() as u16
        };
        Point {
            x: mix(self.previous.x, self.camera.x),
            y: mix(self.previous.y, self.camera.y),
        }
    }

    /// Draws the viewer's level at `blend` between the last two ticks, as wide as `frame`.
    pub fn draw(&self, frame: &mut Frame, world: &World, graphics: &LevelGraphics, blend: f32) {
        let level = self.info(world);
        let view = Viewport::around(level, self.camera_at(blend), frame.width as u16);
        draw_playfield(frame, world, level, graphics, view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    #[test]
    fn camera_moves_and_stays_within_the_map() {
        let world = synthetic_world(); // 2 × 2 chunks: limits (0, 56)
        let mut viewer = LevelViewer::new(&world, 0);
        assert_eq!(viewer.camera(), Point { x: 0, y: 8 });
        let down = ViewerInput {
            down: true,
            ..ViewerInput::default()
        };
        viewer.step(&world, down);
        assert_eq!(viewer.camera(), Point { x: 0, y: 12 });
        assert_eq!(viewer.camera_at(0.0), Point { x: 0, y: 8 });
        assert_eq!(viewer.camera_at(0.5), Point { x: 0, y: 10 });
        assert_eq!(viewer.camera_at(1.0), Point { x: 0, y: 12 });
        for _ in 0..100 {
            viewer.step(&world, down);
        }
        assert_eq!(viewer.camera(), Point { x: 0, y: 56 });
        let up_left = ViewerInput {
            up: true,
            left: true,
            ..ViewerInput::default()
        };
        for _ in 0..100 {
            viewer.step(&world, up_left);
        }
        assert_eq!(viewer.camera(), Point { x: 0, y: 0 });
        // Opposite directions cancel.
        viewer.step(
            &world,
            ViewerInput {
                up: true,
                down: true,
                left: false,
                right: false,
            },
        );
        assert_eq!(viewer.camera(), Point { x: 0, y: 0 });
    }

    #[test]
    fn levels_wrap_around() {
        let world = synthetic_world();
        let mut viewer = LevelViewer::new(&world, 0);
        viewer.change_level(&world, -1);
        assert_eq!(viewer.level(), 41);
        viewer.change_level(&world, 1);
        assert_eq!(viewer.level(), 0);
        assert_eq!(viewer.camera_at(0.3), viewer.camera());
    }
}
