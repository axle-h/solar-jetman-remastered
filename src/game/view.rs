//! The field of view (`docs/widescreen.md`): the original's screen, or a wider one centred on
//! it. Every rule that measures against the screen measures against a [`Viewport`], which at
//! [`ORIGINAL_WIDTH`] is exactly the camera's screen.

use crate::game::level::{Level, Point};

/// The original screen's width in pixels.
pub const ORIGINAL_WIDTH: u16 = 256;

/// [`ORIGINAL_WIDTH`], for serde defaults.
pub fn original_width() -> u16 {
    ORIGINAL_WIDTH
}

/// The widest view: 32:9 at 240 lines.
pub const MAX_WIDTH: u16 = 854;

/// A view width held to the supported range.
pub fn clamp_width(width: u16) -> u16 {
    width.clamp(ORIGINAL_WIDTH, MAX_WIDTH)
}

/// The width of a view `lines` high with `aspect` (width over height), held to the range.
pub fn width_for(aspect: f64, lines: u32) -> u16 {
    let width = (f64::from(lines) * aspect).round();
    clamp_width(width.clamp(0.0, f64::from(u16::MAX)) as u16)
}

/// The world rectangle the screen shows: `left` and `top` world pixels (`left` wraps round
/// below 0 as the 16-bit positions do), `width` pixels across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub left: u16,
    pub top: u16,
    pub width: u16,
}

impl Viewport {
    /// The original's screen: the camera's.
    pub fn original(camera: Point) -> Viewport {
        Viewport {
            left: camera.x,
            top: camera.y,
            width: ORIGINAL_WIDTH,
        }
    }

    /// A view `width` wide around `camera`'s screen: centred on it, pushed back inside the
    /// columns the camera can show (0 to its limit plus one screen), or centred on those
    /// columns where they are narrower than the view. It always holds the camera's screen.
    pub fn around(level: &Level, camera: Point, width: u16) -> Viewport {
        let width = clamp_width(width);
        let camera_x = i32::from(camera.x);
        let (w, screen) = (i32::from(width), i32::from(ORIGINAL_WIDTH));
        let reach = i32::from(level.camera_limit().x) + screen;
        let mut left = camera_x - (w - screen) / 2;
        if reach >= w {
            left = left.clamp(0, reach - w);
        } else {
            left = (reach - w) / 2;
        }
        let left = left.clamp(camera_x + screen - w, camera_x);
        Viewport {
            left: left as u16,
            top: camera.y,
            width,
        }
    }

    /// The view's width as a signed number, for the window tests.
    pub fn w(&self) -> i32 {
        i32::from(self.width)
    }

    /// `x` relative to the view's left edge, as a signed 16-bit offset.
    pub fn rx(&self, x: u16) -> i32 {
        i32::from(x.wrapping_sub(self.left) as i16)
    }

    /// `y` relative to the view's top edge, as a signed 16-bit offset.
    pub fn ry(&self, y: u16) -> i32 {
        i32::from(y.wrapping_sub(self.top) as i16)
    }

    /// How far right of the view's left edge the camera's screen starts.
    pub fn shift(&self, camera: Point) -> i32 {
        i32::from(camera.x.wrapping_sub(self.left) as i16)
    }

    /// The view's left edge as a signed world x.
    pub fn left_x(&self) -> i32 {
        i32::from(self.left as i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    fn level(width: u8) -> Level {
        let mut level = synthetic_world().levels[0].clone();
        level.width = width;
        level.height = 8;
        level
    }

    #[test]
    fn original_width_is_the_camera() {
        let level = level(8);
        for x in [0, 100, 768] {
            let camera = Point { x, y: 40 };
            assert_eq!(
                Viewport::around(&level, camera, ORIGINAL_WIDTH),
                Viewport::original(camera)
            );
        }
    }

    #[test]
    fn wide_view_is_centred_and_kept_inside() {
        // Width 8 (half-chunks): the camera reaches x 768, so columns 0-1023 can be seen.
        let level = level(8);
        let view = Viewport::around(&level, Point { x: 400, y: 0 }, 456);
        assert_eq!((view.left, view.width), (300, 456));
        assert_eq!(view.shift(Point { x: 400, y: 0 }), 100);
        let view = Viewport::around(&level, Point { x: 20, y: 0 }, 456);
        assert_eq!(view.left, 0);
        let view = Viewport::around(&level, Point { x: 768, y: 0 }, 456);
        assert_eq!(view.left, 1024 - 456);
    }

    #[test]
    fn narrow_map_is_centred_but_the_screen_stays_in_view() {
        // Width 2: one screen. A 456-wide view is centred on it.
        let level = level(2);
        let view = Viewport::around(&level, Point { x: 0, y: 0 }, 456);
        assert_eq!(view.left_x(), -100);
        // A camera past its limit (the final level's route has none) is still in view.
        let view = Viewport::around(&level, Point { x: 2000, y: 0 }, 456);
        assert!(view.rx(2000) >= 0 && view.rx(2000) + 256 <= 456);
    }

    #[test]
    fn widths_follow_the_aspect() {
        assert_eq!(width_for(16.0 / 9.0, 240), 427);
        assert_eq!(width_for(4.0 / 3.0, 240), 320);
        assert_eq!(width_for(1.0, 240), 256);
        assert_eq!(width_for(10.0, 240), MAX_WIDTH);
    }
}
