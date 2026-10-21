//! Drawing the playfield: the map window under the camera, in the top of the screen
//! (`docs/spec/levels.md` section 5, `docs/spec/graphics.md` 1.1).

use crate::game::graphics::LevelGraphics;
use crate::game::level::{Level, Point, TILE_SIZE, World};
use crate::game::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Lines of the screen the playfield covers, from the top. Lines 192-195 are blank on the
/// original (rendering is off while the status panel is set up), and the panel follows.
pub const PLAYFIELD_LINES: usize = 193;

/// The leftmost columns of the playfield are blanked.
pub const BLANK_LEFT_COLUMNS: usize = 8;

/// A screen of NES colour numbers (0-63), row by row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Frame {
    pub pixels: Vec<u8>,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            pixels: vec![0x0F; SCREEN_WIDTH * SCREEN_HEIGHT],
        }
    }
}

impl Frame {
    pub fn pixel(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * SCREEN_WIDTH + x]
    }
}

/// Draws the whole screen for a level seen from `camera` (world pixels of the playfield's
/// top-left corner): the map window in the playfield, the universal background colour in the
/// blanked left columns and below the playfield (the status panel is not drawn yet).
pub fn draw_playfield(
    frame: &mut Frame,
    world: &World,
    level: &Level,
    graphics: &LevelGraphics,
    camera: Point,
) {
    let palette = &graphics.palette;
    frame.pixels.fill(palette.universal_background());
    for sy in 0..PLAYFIELD_LINES {
        let wy = u32::from(camera.y) + sy as u32;
        let (ty, fine_y) = (wy / TILE_SIZE, wy % TILE_SIZE);
        let line = &mut frame.pixels[sy * SCREEN_WIDTH..(sy + 1) * SCREEN_WIDTH];
        let mut sx = BLANK_LEFT_COLUMNS;
        while sx < SCREEN_WIDTH {
            let wx = u32::from(camera.x) + sx as u32;
            let (tx, fine_x) = (wx / TILE_SIZE, wx % TILE_SIZE);
            let tile = world.tile_at(level, tx, ty);
            let row = graphics.background.row(tile.tile, fine_y);
            let span = (TILE_SIZE - fine_x) as usize;
            let span = span.min(SCREEN_WIDTH - sx);
            for (i, &value) in row[fine_x as usize..fine_x as usize + span]
                .iter()
                .enumerate()
            {
                line[sx + i] = palette.background(tile.palette, value);
            }
            sx += span;
        }
    }
}

/// The colour index (0-3) of the background pixel at screen (`x`, `y`) of the playfield,
/// drawn with `camera` as [`draw_playfield`] draws it; 0 in the blanked left columns.
pub fn background_value(
    world: &World,
    level: &Level,
    graphics: &LevelGraphics,
    camera: Point,
    (x, y): (usize, usize),
) -> u8 {
    if x < BLANK_LEFT_COLUMNS {
        return 0;
    }
    let wx = u32::from(camera.x) + x as u32;
    let wy = u32::from(camera.y) + y as u32;
    let tile = world.tile_at(level, wx / TILE_SIZE, wy / TILE_SIZE);
    graphics.background.row(tile.tile, wy % TILE_SIZE)[(wx % TILE_SIZE) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::graphics::tests::synthetic_graphics;
    use crate::game::graphics::{Chr, Palette};
    use crate::game::level::tests::synthetic_world;

    /// Graphics where tile n is filled with pixel value n mod 4, except tile 8, whose top row
    /// is 0 1 2 3 0 1 2 3; palette entry i is colour 0x20 + i, entry 16 is 0x0F.
    fn test_graphics() -> LevelGraphics {
        let mut chr = Chr::default();
        for tile in 0..256usize {
            let value = (tile % 4) as u8;
            let plane0 = if value & 1 != 0 { 0xFF } else { 0 };
            let plane1 = if value & 2 != 0 { 0xFF } else { 0 };
            let mut bytes = [plane0; 16];
            bytes[8..].fill(plane1);
            if tile == 8 {
                bytes[0] = 0b0101_0101;
                bytes[8] = 0b0011_0011;
            }
            chr.write(0x1000 + (tile * 16) as u16, &bytes);
        }
        let mut entries = [0u8; 32];
        for (i, entry) in entries.iter_mut().enumerate() {
            *entry = 0x20 + i as u8;
        }
        entries[16] = 0x0F;
        LevelGraphics {
            number: 0,
            background: chr.pattern_table(1),
            palette: Palette(entries),
            gauge_rows: Vec::new(),
            item_pictures: Vec::new(),
            beam_tiles: Vec::new(),
            sign_tiles: Vec::new(),
        }
    }

    #[test]
    fn draws_the_map_window() {
        let world = synthetic_world();
        let level = &world.levels[0];
        let graphics = test_graphics();
        let mut frame = Frame::default();
        // Camera at world (120, 0): screen x 8 shows world x 128, the start of chunk (1, 0),
        // whose top-left tile is tile 8 with palette 2.
        draw_playfield(&mut frame, &world, level, &graphics, Point { x: 120, y: 0 });
        let expected_row = [0x0F, 0x29, 0x2A, 0x2B, 0x0F, 0x29, 0x2A, 0x2B];
        for (i, &colour) in expected_row.iter().enumerate() {
            assert_eq!(frame.pixel(8 + i, 0), colour, "x {}", 8 + i);
        }
        // Only the top row of tile 8 has pixels set.
        assert_eq!(frame.pixel(9, 1), 0x0F);
        // Screen x 16 is world x 136: tile 9 of block 2 (value 1, palette 2).
        assert_eq!(frame.pixel(16, 0), 0x29);
        // Left of x 8 and below the playfield: universal background.
        assert!((0..8).all(|x| frame.pixel(x, 10) == 0x0F));
        assert!((PLAYFIELD_LINES..SCREEN_HEIGHT).all(|y| frame.pixel(100, y) == 0x0F));
        // Chunk (0, 0) is sky: tile 0, value 0.
        draw_playfield(&mut frame, &world, level, &graphics, Point { x: 0, y: 0 });
        assert_eq!(frame.pixel(50, 50), 0x0F);
    }

    #[test]
    fn fine_scroll_shifts_pixels() {
        let world = synthetic_world();
        let level = &world.levels[0];
        let graphics = test_graphics();
        let mut frame = Frame::default();
        // Camera x 123: screen x 8 is world x 131, pixel 3 of tile 8's top row (value 3).
        draw_playfield(&mut frame, &world, level, &graphics, Point { x: 123, y: 0 });
        assert_eq!(frame.pixel(8, 0), 0x2B);
        assert_eq!(frame.pixel(9, 0), 0x0F);
        // Camera y 1 moves tile 8's top row out of view.
        draw_playfield(&mut frame, &world, level, &graphics, Point { x: 120, y: 1 });
        assert_eq!(frame.pixel(9, 0), 0x0F);
        // Block 1 (tiles 4-7 of set 0) below: world y 24 is its tile 6 (value 2, palette 2).
        assert_eq!(frame.pixel(8, 23), 0x2A);
    }

    #[test]
    fn real_layout_from_synthetic_imports() {
        // The whole path from imported graphics to a frame runs on synthetic data.
        let world = synthetic_world();
        let level = &world.levels[3];
        let graphics = LevelGraphics::new(&synthetic_graphics(), level).unwrap();
        let mut frame = Frame::default();
        draw_playfield(&mut frame, &world, level, &graphics, level.camera_start);
        assert!(frame.pixels.iter().all(|&c| c < 0x40));
    }
}
