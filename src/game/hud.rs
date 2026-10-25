//! The status panel under the playfield (`docs/spec/economy.md` 3.1, `docs/spec/graphics.md`
//! 1.1): the fuel and weapon gauges, jetpods, lives and the score, in background tiles of the
//! panel's tile set. The original refreshes the parts as rotating jobs; the remake draws all of
//! them every frame (`docs/deviations.md`).
//!
//! The layout (tile cells of the panel's nametable page, rows 4-6) was read from the original's
//! screen: labels in column 1, the 14 gauge cells from column 2, icons from column 16, the
//! score's six two-row digits from column 21 and the minus sign in column 20.

use crate::game::graphics::LevelGraphics;
use crate::game::playfield::Frame;
use crate::game::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Screen line of the panel's first tile row (graphics.md 1.1: the panel's content starts at
/// line 202) and the panel's horizontal scroll (252: tile column c starts at pixel 8c + 4).
const FIRST_LINE: usize = 202;
const LEFT: usize = 4;

const TILE_BLANK: u8 = 0x00;
const TILE_POD_TOP: u8 = 0xF4;
const TILE_POD_BOTTOM: u8 = 0xF5;
const TILE_JETMAN: u8 = 0xF6;
const TILE_FUEL_LABEL: u8 = 0xF7;
const TILE_WEAPON_LABEL: u8 = 0xF8;
const TILE_CELL_FULL: u8 = 0xF9;
const TILE_FUEL_PART: u8 = 0xFA;
const TILE_WEAPON_PART: u8 = 0xFB;
const TILE_MINUS: u8 = 0xE4;

/// Gauge cells and the units each holds.
const GAUGE_CELLS: usize = 14;
const CELL_UNITS: u8 = 16;
/// Icons drawn at most.
const MAX_ICONS: u8 = 4;

/// Top and bottom tiles of the digits 0-9.
pub(crate) const DIGIT_TOP: [u8; 10] = [0xE5, 0xE7, 0xE9, 0xEB, 0xED, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3];
pub(crate) const DIGIT_BOTTOM: [u8; 10] =
    [0xE6, 0xE8, 0xEA, 0xEC, 0xEE, 0xEC, 0xE6, 0xE8, 0xE6, 0xEC];

/// Attribute bytes of the panel's cell rows 4-7, by column / 4: each holds four 2 × 2-cell
/// quadrants (bits 0-1 top left, 2-3 top right, 4-5 bottom left, 6-7 bottom right).
const ATTRIBUTES: [u8; 8] = [0x44, 0x55, 0x55, 0x55, 0x05, 0x55, 0x55, 0x00];

/// What the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hud {
    /// The fuel gauge's display copy, RAM $05CE (0-224).
    pub fuel: u8,
    /// The weapon energy gauge, RAM $C9 (0-224).
    pub weapon: u8,
    /// Jetpods RAM $3D and Jetman lives RAM $3E.
    pub jetpods: u8,
    pub lives: u8,
    /// The score, 0-999999.
    pub score: u32,
    /// Whether the score is negative (RAM $CA bit 7): a minus sign in column 20.
    pub negative: bool,
}

/// One cell of the panel: tile and background palette.
type Cell = (u8, u8);

/// The tile of gauge cell `index` for a gauge showing `value`: full, the partly filled cell
/// that holds the remainder, or blank.
fn gauge_cell(index: usize, value: u8, partial: u8) -> u8 {
    let full = usize::from(value / CELL_UNITS);
    match index.cmp(&full) {
        std::cmp::Ordering::Less => TILE_CELL_FULL,
        std::cmp::Ordering::Equal => partial,
        std::cmp::Ordering::Greater => TILE_BLANK,
    }
}

impl Hud {
    /// The 3 × 32 cells of rows 4-6.
    fn cells(&self) -> [[Cell; 32]; 3] {
        let mut tiles = [[TILE_BLANK; 32]; 3];
        tiles[0][1] = TILE_FUEL_LABEL;
        tiles[2][1] = TILE_WEAPON_LABEL;
        for cell in 0..GAUGE_CELLS {
            tiles[0][2 + cell] = gauge_cell(cell, self.fuel, TILE_FUEL_PART);
            tiles[2][2 + cell] = gauge_cell(cell, self.weapon, TILE_WEAPON_PART);
        }
        for icon in 0..usize::from(MAX_ICONS) {
            if icon < usize::from(self.jetpods) {
                tiles[0][16 + icon] = TILE_POD_TOP;
                tiles[1][16 + icon] = TILE_POD_BOTTOM;
            }
            if icon < usize::from(self.lives) {
                tiles[2][16 + icon] = TILE_JETMAN;
            }
        }
        if self.negative {
            tiles[2][20] = TILE_MINUS;
        }
        let digits = format!("{:06}", self.score % 1_000_000);
        for (place, digit) in digits.bytes().enumerate() {
            let digit = usize::from(digit - b'0');
            tiles[1][21 + place] = DIGIT_TOP[digit];
            tiles[2][21 + place] = DIGIT_BOTTOM[digit];
        }
        let mut cells = [[(TILE_BLANK, 0); 32]; 3];
        for (row, line) in tiles.iter().enumerate() {
            for (column, &tile) in line.iter().enumerate() {
                let byte = ATTRIBUTES[column / 4];
                // Rows 4 and 5 are the top quadrants, row 6 the bottom ones.
                let quadrant = usize::from(row == 2) * 2 + (column / 2) % 2;
                cells[row][column] = (tile, (byte >> (quadrant * 2)) & 3);
            }
        }
        cells
    }

    /// Page 1 of the picture generator as the gameplay leaves it: 960 tile bytes (rows 4-6 hold
    /// the panel) and the 64 attribute bytes. The partly filled gauge cells are the tiles $FA
    /// and $FB, which the caller makes (graphics.md 3).
    pub fn panel_page(&self) -> Vec<u8> {
        let mut page = vec![0; 32 * 30 + 64];
        for (row, line) in self.cells().iter().enumerate() {
            for (column, &(tile, _)) in line.iter().enumerate() {
                page[(4 + row) * 32 + column] = tile;
            }
        }
        for (group, &byte) in ATTRIBUTES.iter().enumerate() {
            page[32 * 30 + 8 + group] = byte;
        }
        page
    }

    /// The panel's tiles without the two gauges (columns 0-15 blank): the approach screen
    /// shows only the pod icons and the score in the same places.
    pub fn icons_and_score(&self) -> [[u8; 32]; 3] {
        let mut tiles = [[TILE_BLANK; 32]; 3];
        for (row, line) in self.cells().iter().enumerate() {
            for (column, &(tile, _)) in line.iter().enumerate().skip(16) {
                tiles[row][column] = tile;
            }
        }
        tiles
    }

    /// Draws the panel into `frame` below the playfield, centred in a wide frame. The partly
    /// filled gauge cells are made of `graphics.gauge_rows` (graphics.md 3); without them such
    /// a cell is blank.
    pub fn draw(&self, frame: &mut Frame, graphics: &LevelGraphics) {
        let left = frame.centre();
        let cells = self.cells();
        let fill = |value: u8| usize::from(value % CELL_UNITS / 2) & 7;
        for (row, line) in cells.iter().enumerate() {
            for (column, &(tile, palette)) in line.iter().enumerate() {
                if tile == TILE_BLANK {
                    continue;
                }
                let gauge_row = match tile {
                    TILE_FUEL_PART => Some(graphics.gauge_rows.get(fill(self.fuel))),
                    TILE_WEAPON_PART => Some(graphics.gauge_rows.get(fill(self.weapon))),
                    _ => None,
                };
                for y in 0..8usize {
                    let line_y = FIRST_LINE + row * 8 + y;
                    if line_y >= SCREEN_HEIGHT {
                        continue;
                    }
                    let pixels: [u8; 8] = match gauge_row {
                        // Both planes hold the byte: every set bit is colour 3.
                        Some(Some(&byte)) => std::array::from_fn(|x| 3 * (byte >> (7 - x) & 1)),
                        Some(None) => [0; 8],
                        None => std::array::from_fn(|x| graphics.background.row(tile, y as u32)[x]),
                    };
                    for (x, &value) in pixels.iter().enumerate() {
                        let screen_x = LEFT + column * 8 + x;
                        if screen_x < SCREEN_WIDTH && value != 0 {
                            frame.pixels[line_y * frame.width + left + screen_x] =
                                graphics.palette.background(palette, value);
                        }
                    }
                }
            }
        }
    }
}

/// The bytes of one item picture (nine tiles).
const PICTURE_BYTES: usize = 9 * 16;

/// Draws the fitted special weapon's icon (graphics.md 1.2): item `item`'s picture as 3 x 3
/// background tiles at columns 28-30 of rows 4-6, palette 0. Nothing for a picture the import
/// lacks.
pub fn draw_weapon_icon(frame: &mut Frame, graphics: &LevelGraphics, item: u8) {
    let start = usize::from(item) * PICTURE_BYTES;
    let Some(picture) = graphics.item_pictures.get(start..start + PICTURE_BYTES) else {
        return;
    };
    let left = frame.centre();
    for (index, bytes) in picture.as_chunks::<16>().0.iter().enumerate() {
        let pixels = crate::game::graphics::tile_pixels(bytes);
        let (row, column) = (index / 3, 28 + index % 3);
        for y in 0..8usize {
            let line_y = FIRST_LINE + row * 8 + y;
            if line_y >= SCREEN_HEIGHT {
                continue;
            }
            for x in 0..8usize {
                let screen_x = LEFT + column * 8 + x;
                let value = pixels[y * 8 + x];
                if screen_x < SCREEN_WIDTH && value != 0 {
                    frame.pixels[line_y * frame.width + left + screen_x] =
                        graphics.palette.background(0, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hud() -> Hud {
        Hud {
            fuel: 0x50,
            weapon: 0xE0,
            jetpods: 2,
            lives: 4,
            score: 1000,
            negative: false,
        }
    }

    #[test]
    fn weapon_icon_draws_the_items_picture_in_columns_28_to_30() {
        let world = crate::game::level::tests::synthetic_world();
        let mut graphics = LevelGraphics::new(
            &crate::game::graphics::tests::synthetic_graphics(),
            &world.levels[0],
        )
        .unwrap();
        graphics.item_pictures = vec![0xFF; 2 * PICTURE_BYTES];
        graphics.item_pictures[PICTURE_BYTES..].fill(0);
        let mut frame = Frame::default();
        draw_weapon_icon(&mut frame, &graphics, 0);
        let at = |x: usize, y: usize| frame.pixels[y * SCREEN_WIDTH + x];
        let colour = graphics.palette.background(0, 3);
        assert_eq!(at(LEFT + 28 * 8, FIRST_LINE), colour);
        assert_ne!(at(LEFT + 27 * 8, FIRST_LINE), colour);
        let mut other = Frame::default();
        draw_weapon_icon(&mut other, &graphics, 1);
        assert_ne!(
            other.pixels[FIRST_LINE * SCREEN_WIDTH + LEFT + 28 * 8],
            colour
        );
    }

    #[test]
    fn gauges_fill_whole_cells_then_one_partial() {
        let cells = hud().cells();
        let row = |r: usize| cells[r].map(|(tile, _)| tile);
        // 0x50 = 5 full cells and a partial one, then blank.
        assert_eq!(&row(0)[2..9], &[0xF9, 0xF9, 0xF9, 0xF9, 0xF9, 0xFA, 0x00]);
        // A full tank shows 14 full cells and no partial one.
        assert!(row(2)[2..16].iter().all(|&t| t == 0xF9));
    }

    #[test]
    fn icons_and_score_digits() {
        let cells = hud().cells();
        let tile = |r: usize, c: usize| cells[r][c].0;
        assert_eq!((tile(0, 16), tile(0, 17), tile(0, 18)), (0xF4, 0xF4, 0));
        assert_eq!((tile(1, 16), tile(2, 19), tile(2, 20)), (0xF5, 0xF6, 0));
        // 001000: zeros, a one, zeros.
        let top: Vec<u8> = (21..27).map(|c| tile(1, c)).collect();
        assert_eq!(top, [0xE5, 0xE5, 0xE7, 0xE5, 0xE5, 0xE5]);
        let bottom: Vec<u8> = (21..27).map(|c| tile(2, c)).collect();
        assert_eq!(bottom, [0xE6, 0xE6, 0xE8, 0xE6, 0xE6, 0xE6]);
    }

    #[test]
    fn negative_score_shows_a_sign() {
        let hud = Hud {
            negative: true,
            ..hud()
        };
        assert_eq!(hud.cells()[2][20], (0xE4, 1));
    }

    #[test]
    fn palettes_follow_the_attribute_bytes() {
        let cells = hud().cells();
        assert_eq!(cells[0][1].1, 0, "the F label");
        assert_eq!(cells[0][2].1, 1, "the gauge");
        assert_eq!(cells[0][16].1, 1, "pod icons");
        assert_eq!(cells[2][16].1, 0, "Jetman icons");
        assert_eq!(cells[2][22].1, 1, "score digits");
    }
}
