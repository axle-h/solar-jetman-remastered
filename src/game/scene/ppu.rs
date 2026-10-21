//! A small picture generator for the screens outside gameplay: a nametable page of tiles with
//! attributes, a scroll, two pattern tables, a palette and a list of 8 × 8 sprites, drawn into a
//! [`Frame`] the way the console shows them (`docs/spec/graphics.md` 1, `docs/spec/scenes.md` 0).

use crate::game::graphics::{Chr, Graphics, MissingGraphics, Palette, PatternTable, PictureWrite};
use crate::game::playfield::Frame;
use crate::game::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::cell::OnceCell;

/// Tile columns and rows of a nametable page.
pub const COLUMNS: usize = 32;
pub const ROWS: usize = 30;
/// Height of the page in pixels: a vertical scroll wraps at this line.
const PAGE_HEIGHT: usize = ROWS * 8;
const ATTRIBUTE_BASE: usize = COLUMNS * ROWS;

/// One 8 × 8 sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sprite {
    /// Screen position of the top-left pixel (not the sprite memory's y, which is one less).
    pub x: i32,
    pub y: i32,
    pub tile: u8,
    /// Sprite palette 0-3.
    pub palette: u8,
    /// Drawn behind background pixels that are not transparent.
    pub behind: bool,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Sprite {
    pub fn new(x: i32, y: i32, tile: u8, palette: u8) -> Sprite {
        Sprite {
            x,
            y,
            tile,
            palette,
            behind: false,
            flip_x: false,
            flip_y: false,
        }
    }
}

/// A band of lines whose tiles come from a world wider than the page, scrolled on its own (the
/// shop's strip of item frames: the console rewrites the page's tiles as they scroll into view,
/// the remake reads the world directly).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Band {
    /// Screen lines `first_line` up to (not including) `end_line`.
    pub first_line: usize,
    pub end_line: usize,
    /// Horizontal scroll in pixels (no wrap).
    pub scroll_x: usize,
    /// Tile numbers and background palettes (0-3) of the world, `columns` per row, from page
    /// row `first_row` on, one row per 8 lines of the page (the vertical scroll applies).
    pub first_row: usize,
    pub columns: usize,
    pub tiles: Vec<u8>,
    pub palettes: Vec<u8>,
}

/// Everything a still screen is made of.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Screen {
    pub chr: Chr,
    /// The pattern tables the screen shows, made from `chr` (not saved: made again on first
    /// use after a load).
    #[serde(skip)]
    background: OnceCell<PatternTable>,
    #[serde(skip)]
    sprite_tiles: OnceCell<PatternTable>,
    /// Pattern table the background and the sprites use (0 or 1).
    pub background_table: usize,
    pub sprite_table: usize,
    /// Tile numbers (960) then attribute bytes (64) of the page.
    page: Vec<u8>,
    pub palette: Palette,
    /// Scroll of the top-left pixel; x wraps at 256, y at 240.
    pub scroll: (usize, usize),
    /// The background is not shown in the leftmost 8 pixels (the picture generator's mask).
    pub blank_left: bool,
    /// Sprites are not shown in the leftmost 8 pixels either.
    pub blank_left_sprites: bool,
    /// Background rendering is on; when off only the backdrop colour (palette entry 16) and the
    /// sprites show.
    pub show_background: bool,
    /// In front-to-back order: the first sprite wins where two overlap.
    pub sprites: Vec<Sprite>,
    /// From this screen line on the background comes from the pattern table `.1` (a mid-screen
    /// switch; sprites keep theirs).
    pub split: Option<(usize, usize)>,
    #[serde(skip)]
    split_tiles: OnceCell<PatternTable>,
    /// Page 1 (tiles then attributes), shown from line 196 with the scroll (252, 26): the
    /// status panel. Lines 193-195 show only the backdrop colour and nothing below line 192 has
    /// sprites (`graphics.md` 1.1).
    pub panel: Option<Vec<u8>>,
    /// Lines the panel starts earlier than usual (0; the original's planet map: 1, Q37).
    pub panel_lift: usize,
    /// A band with a world of its own, over the page.
    pub band: Option<Band>,
    /// From this line on rendering is off: only the backdrop colour shows, no sprites either
    /// (the credits' split).
    pub hide_from: Option<usize>,
    /// One line drawn only up to a column (the credits' split switches rendering off partway
    /// along it): (line, first hidden column).
    pub hide_line_from: Option<(usize, usize)>,
    /// Sprites behind the background are hidden by the scenery's small transparent holes too,
    /// not only by its coloured pixels: the remake's (`docs/deviations.md`, "Sprites behind
    /// the scenery").
    #[serde(default)]
    pub solid_behind: bool,
    /// Background palettes of single cells of the page (row-major, 32 × 30), over the
    /// attribute table's 2 × 2 quadrants where set: the remake's finer colouring
    /// (`docs/deviations.md`, "Colour cells"). Empty when unused.
    #[serde(default)]
    pub cell_palettes: Vec<Option<u8>>,
}

/// First line of the blank gap above the status panel and the panel's first line.
const PANEL_GAP: usize = 193;
const PANEL_FIRST: usize = 196;
const PANEL_SCROLL: (usize, usize) = (252, 26);

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Screen(..)")
    }
}

impl Default for Screen {
    fn default() -> Screen {
        let chr = Chr::default();
        Screen {
            background: OnceCell::from(chr.pattern_table(1)),
            sprite_tiles: OnceCell::from(chr.pattern_table(0)),
            split_tiles: OnceCell::from(chr.pattern_table(0)),
            chr,
            background_table: 1,
            sprite_table: 0,
            page: vec![0; ATTRIBUTE_BASE + 64],
            palette: Palette([0x0F; 32]),
            scroll: (0, 0),
            blank_left: false,
            blank_left_sprites: false,
            show_background: true,
            sprites: Vec::new(),
            split: None,
            panel: None,
            panel_lift: 0,
            band: None,
            hide_from: None,
            hide_line_from: None,
            solid_behind: false,
            cell_palettes: Vec::new(),
        }
    }
}

impl Screen {
    /// Rebuilds the pattern tables after the CHR memory changed.
    pub fn refresh(&mut self) {
        self.background = OnceCell::from(self.chr.pattern_table(self.background_table));
        self.sprite_tiles = OnceCell::from(self.chr.pattern_table(self.sprite_table));
        if let Some((_, table)) = self.split {
            self.split_tiles = OnceCell::from(self.chr.pattern_table(table));
        }
    }

    fn background_tiles(&self) -> &PatternTable {
        self.background
            .get_or_init(|| self.chr.pattern_table(self.background_table))
    }

    /// The pattern table the sprites are drawn with.
    pub fn sprite_tiles(&self) -> &PatternTable {
        self.sprite_tiles
            .get_or_init(|| self.chr.pattern_table(self.sprite_table))
    }

    fn split_background_tiles(&self) -> &PatternTable {
        self.split_tiles.get_or_init(|| {
            self.chr
                .pattern_table(self.split.map_or(0, |(_, table)| table))
        })
    }

    /// Replays tile loads of the asset pack into CHR memory: (stream, PPU address).
    pub fn load_tiles(
        &mut self,
        graphics: &Graphics,
        loads: &[(&'static str, u16)],
    ) -> Result<(), MissingGraphics> {
        let loads: Vec<_> = loads
            .iter()
            .map(|&(stream, address)| crate::game::graphics::ChrLoad {
                stream,
                range: None,
                address,
            })
            .collect();
        self.chr.apply(graphics, &loads)?;
        self.refresh();
        Ok(())
    }

    /// The page's 960 tile bytes then its 64 attribute bytes.
    pub fn page_bytes(&self) -> &[u8] {
        &self.page
    }

    /// Replaces the page (tiles then attributes) with `page`, as the console's video memory
    /// takes the writes it was sent a frame ago.
    pub fn set_page(&mut self, page: &[u8]) {
        if page.len() == self.page.len() {
            self.page.copy_from_slice(page);
        }
    }

    /// Writes a picture's nametable and attribute bytes.
    pub fn load_picture(&mut self, writes: &[PictureWrite]) {
        for write in writes {
            let start = usize::from(write.address).saturating_sub(0x2000);
            for (i, &byte) in write.bytes.iter().enumerate() {
                if let Some(cell) = self.page.get_mut(start + i) {
                    *cell = byte;
                }
            }
        }
    }

    /// Writes the four attribute bytes of attribute row `group` (rows 4 × group to 4 × group + 3,
    /// attribute columns 0 to 3).
    pub fn set_attributes(&mut self, group: usize, bytes: &[u8; 4]) {
        let at = ATTRIBUTE_BASE + group * 8;
        if let Some(cells) = self.page.get_mut(at..at + 4) {
            cells.copy_from_slice(bytes);
        }
    }

    /// Gives the 2 × 2 cells that hold (`column`, `row`) background palette `palette`.
    pub fn set_cell_palette(&mut self, column: usize, row: usize, palette: u8) {
        if column < COLUMNS && row < ROWS {
            let at = ATTRIBUTE_BASE + (row / 4) * 8 + column / 4;
            let shift = ((row / 2) & 1) * 4 + ((column / 2) & 1) * 2;
            self.page[at] = self.page[at] & !(3 << shift) | (palette & 3) << shift;
        }
    }

    /// Gives the one cell (`column`, `row`) its own background palette, or takes it away
    /// (`None`), over the attribute table ([`Screen::cell_palettes`]).
    pub fn set_own_palette(&mut self, column: usize, row: usize, palette: Option<u8>) {
        if column < COLUMNS && row < ROWS {
            if self.cell_palettes.is_empty() {
                self.cell_palettes = vec![None; COLUMNS * ROWS];
            }
            self.cell_palettes[row * COLUMNS + column] = palette.map(|p| p & 3);
        }
    }

    /// Writes attribute byte `index` (0-63) of the page.
    pub fn set_attribute(&mut self, index: usize, byte: u8) {
        if index < 64 {
            self.page[ATTRIBUTE_BASE + index] = byte;
        }
    }

    pub fn clear_page(&mut self) {
        self.page.fill(0);
    }

    /// Starts the status panel (page 1) blank.
    pub fn open_panel(&mut self) {
        self.panel = Some(vec![0; ATTRIBUTE_BASE + 64]);
    }

    /// Sets a tile of the status panel (page 1).
    pub fn set_panel_tile(&mut self, column: usize, row: usize, tile: u8) {
        if let Some(panel) = &mut self.panel
            && column < COLUMNS
            && row < ROWS
        {
            panel[row * COLUMNS + column] = tile;
        }
    }

    /// Writes the attribute byte `index` (0-63) of the status panel.
    pub fn set_panel_attribute(&mut self, index: usize, byte: u8) {
        if let Some(panel) = &mut self.panel
            && index < 64
        {
            panel[ATTRIBUTE_BASE + index] = byte;
        }
    }

    /// Writes text into the status panel like [`Screen::print`].
    pub fn print_panel(&mut self, column: usize, row: usize, text: &str) {
        for (at, ch) in (column..).zip(text.chars()) {
            self.set_panel_tile(at, row, char_tile(ch));
        }
    }

    pub fn set_tile(&mut self, column: usize, row: usize, tile: u8) {
        if column < COLUMNS && row < ROWS {
            self.page[row * COLUMNS + column] = tile;
        }
    }

    pub fn tile(&self, column: usize, row: usize) -> u8 {
        self.page[row * COLUMNS + column]
    }

    /// Writes text from (`column`, `row`) with the game's character codes (`text.md` 1, 2.1):
    /// a space is the blank tile, `©` and `™` are the codes $3A and $3B, and a space right after
    /// a comma or a full stop is not written, because those two glyphs carry their own space.
    pub fn print(&mut self, column: usize, row: usize, text: &str) {
        let mut at = column;
        let mut previous = ' ';
        for ch in text.chars() {
            if ch == ' ' && matches!(previous, ',' | '.') {
                previous = ch;
                continue;
            }
            previous = ch;
            self.set_tile(at, row, char_tile(ch));
            at += 1;
        }
    }

    /// Draws the screen into `frame`.
    pub fn draw(&self, frame: &mut Frame) {
        let mut sprite_layer = vec![0u16; SCREEN_WIDTH * SCREEN_HEIGHT];
        for sprite in &self.sprites {
            self.stamp(sprite, &mut sprite_layer);
        }
        // The background's colour numbers (0-3) where sprites may show, for the solid scenery.
        let mut values = self
            .solid_behind
            .then(|| vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT]);
        for y in 0..SCREEN_HEIGHT {
            if self.hide_from.is_some_and(|line| y >= line) {
                for x in 0..SCREEN_WIDTH {
                    frame.pixels[y * SCREEN_WIDTH + x] = self.palette.universal_background();
                }
                sprite_layer[y * SCREEN_WIDTH..(y + 1) * SCREEN_WIDTH].fill(0);
                continue;
            }
            let (gap, first) = (PANEL_GAP - self.panel_lift, PANEL_FIRST - self.panel_lift);
            if self.panel.is_some() && (gap..first).contains(&y) {
                // Rendering is off: only the backdrop colour shows.
                for x in 0..SCREEN_WIDTH {
                    frame.pixels[y * SCREEN_WIDTH + x] = self.palette.universal_background();
                }
                sprite_layer[y * SCREEN_WIDTH..(y + 1) * SCREEN_WIDTH].fill(0);
                continue;
            }
            let in_panel = self.panel.is_some() && y >= first;
            let (page, scroll, tiles) = match &self.panel {
                Some(panel) if in_panel => (
                    panel.as_slice(),
                    (PANEL_SCROLL.0, PANEL_SCROLL.1 + y - first),
                    self.background_tiles(),
                ),
                _ => (
                    self.page.as_slice(),
                    (self.scroll.0, self.scroll.1 + y),
                    match self.split {
                        Some((line, _)) if y >= line => self.split_background_tiles(),
                        _ => self.background_tiles(),
                    },
                ),
            };
            let page_y = scroll.1 % PAGE_HEIGHT;
            let (tile_row, fine_y) = (page_y / 8, page_y % 8);
            let band = self
                .band
                .as_ref()
                .filter(|b| !in_panel && (b.first_line..b.end_line).contains(&y));
            for x in 0..SCREEN_WIDTH {
                let (tile, palette, fine_x, fine_y) = if let Some(band) = band {
                    let row = tile_row.saturating_sub(band.first_row);
                    let world_x = x + band.scroll_x;
                    let at = row * band.columns + world_x / 8;
                    let in_world = world_x / 8 < band.columns && tile_row >= band.first_row;
                    (
                        band.tiles
                            .get(at)
                            .copied()
                            .filter(|_| in_world)
                            .unwrap_or(0),
                        band.palettes
                            .get(at)
                            .copied()
                            .filter(|_| in_world)
                            .unwrap_or(0),
                        world_x % 8,
                        fine_y,
                    )
                } else {
                    let page_x = (x + scroll.0) % SCREEN_WIDTH;
                    let (tile_column, fine_x) = (page_x / 8, page_x % 8);
                    let attribute = page[ATTRIBUTE_BASE + (tile_row / 4) * 8 + tile_column / 4];
                    let shift = ((tile_row / 2) & 1) * 4 + ((tile_column / 2) & 1) * 2;
                    let own = (!in_panel)
                        .then(|| self.cell_palettes.get(tile_row * COLUMNS + tile_column))
                        .flatten()
                        .copied()
                        .flatten();
                    (
                        page[tile_row * COLUMNS + tile_column],
                        own.unwrap_or((attribute >> shift) & 3),
                        fine_x,
                        fine_y,
                    )
                };
                let value = tiles.row(tile, fine_y as u32)[fine_x];
                let value = if (self.blank_left && x < 8) || !self.show_background {
                    0
                } else {
                    value
                };
                frame.pixels[y * SCREEN_WIDTH + x] = self.palette.background(palette, value);
                let i = y * SCREEN_WIDTH + x;
                if let Some(values) = values.as_mut() {
                    values[i] = value;
                }
                let sprite = sprite_layer[i];
                if in_panel || (sprite & 0x100 != 0 && value != 0) {
                    sprite_layer[i] = 0;
                }
            }
        }
        for (i, &sprite) in sprite_layer.iter().enumerate() {
            let hidden = |values: &[u8]| sprite & 0x100 != 0 && enclosed(values, i);
            if sprite != 0 && !values.as_deref().is_some_and(hidden) {
                frame.pixels[i] = (sprite & 0xFF) as u8;
            }
        }
        if let Some((line, column)) = self
            .hide_line_from
            .filter(|&(line, _)| line < SCREEN_HEIGHT)
        {
            for x in column..SCREEN_WIDTH {
                frame.pixels[line * SCREEN_WIDTH + x] = self.palette.universal_background();
            }
        }
    }

    /// Adds the opaque pixels of a sprite that no earlier sprite has taken: colour in the low
    /// byte, bit 8 for "behind the background", and the whole value never 0 where drawn.
    fn stamp(&self, sprite: &Sprite, layer: &mut [u16]) {
        for row in 0..8i32 {
            let y = sprite.y + row;
            if !(0..SCREEN_HEIGHT as i32).contains(&y) {
                continue;
            }
            let source_row = if sprite.flip_y { 7 - row } else { row } as u32;
            let pixels = self.sprite_tiles().row(sprite.tile, source_row);
            for column in 0..8i32 {
                let x = sprite.x + column;
                if !(0..SCREEN_WIDTH as i32).contains(&x) || (self.blank_left_sprites && x < 8) {
                    continue;
                }
                let source = if sprite.flip_x { 7 - column } else { column };
                let value = pixels[source as usize];
                if value == 0 {
                    continue;
                }
                let cell = &mut layer[y as usize * SCREEN_WIDTH + x as usize];
                if *cell != 0 {
                    continue;
                }
                let entry = 16 + usize::from(sprite.palette & 3) * 4 + usize::from(value);
                *cell = u16::from(self.palette.0[entry]) | u16::from(sprite.behind) << 8 | 0x200;
            }
        }
    }
}

/// Reach of [`enclosed`]: holes in the scenery up to this many pixels across in either
/// direction count as scenery.
const HOLE_REACH: usize = 4;

/// Whether the background pixel `i`, transparent (colour 0), is part of the scenery rather
/// than open sky: its outline (a coloured pixel next to it, diagonals included) or a small hole
/// (a coloured pixel within [`HOLE_REACH`] on each of its four sides, on its own line and
/// column).
fn enclosed(values: &[u8], i: usize) -> bool {
    if values[i] != 0 {
        return true;
    }
    let (x, y) = (i % SCREEN_WIDTH, i / SCREEN_WIDTH);
    let at = |x: usize, y: usize| values[y * SCREEN_WIDTH + x] != 0;
    let near = |n: usize, size: usize| n.saturating_sub(1)..=(n + 1).min(size - 1);
    if near(y, SCREEN_HEIGHT).any(|y| near(x, SCREEN_WIDTH).any(|x| at(x, y))) {
        return true;
    }
    let reach = 1..=HOLE_REACH;
    reach.clone().any(|d| x >= d && at(x - d, y))
        && reach.clone().any(|d| x + d < SCREEN_WIDTH && at(x + d, y))
        && reach.clone().any(|d| y >= d && at(x, y - d))
        && reach.clone().any(|d| y + d < SCREEN_HEIGHT && at(x, y + d))
}

/// The tile of a character of the game's text (`text.md` 1).
pub fn char_tile(ch: char) -> u8 {
    match ch {
        ' ' => 0x00,
        '0'..='9' | 'A'..='Z' => ch as u8,
        '©' => 0x3A,
        '™' => 0x3B,
        '×' => 0x5B,
        '-' => 0x5C,
        '.' => 0x5D,
        ',' => 0x5E,
        _ => 0x00,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen whose background tile 1 is solid colour 1 and sprite tile 1 solid colour 2.
    fn screen() -> Screen {
        let mut screen = Screen::default();
        screen.chr.write(0x1000 + 16, &[0xFF; 8]);
        screen.chr.write(16, &[0; 8]);
        screen.chr.write(16 + 8, &[0xFF; 8]);
        screen.refresh();
        for (i, entry) in screen.palette.0.iter_mut().enumerate() {
            *entry = 0x10 + i as u8;
        }
        screen.palette.0[16] = 0x0F;
        screen
    }

    #[test]
    fn background_scrolls_and_wraps() {
        let mut screen = screen();
        screen.set_tile(0, 29, 1);
        screen.scroll = (4, 224);
        let mut frame = Frame::default();
        screen.draw(&mut frame);
        // Page row 29 is screen lines 8-15 with a scroll of 224; x 0 of the page is at -4.
        assert_eq!(frame.pixel(3, 8), 0x11);
        assert_eq!(frame.pixel(4, 8), 0x0F);
        assert_eq!(frame.pixel(3, 16), 0x0F);
        // Column 0 wraps round to the right edge: pixels 252-255 show its first four.
        assert_eq!(frame.pixel(252, 8), 0x11);
    }

    #[test]
    fn attributes_pick_the_palette_per_quadrant() {
        let mut screen = screen();
        screen.set_tile(2, 2, 1);
        screen.set_tile(0, 0, 1);
        // Attribute byte 0: top-left 0, top-right 1, bottom-left 2, bottom-right 3.
        screen.load_picture(&[PictureWrite {
            address: 0x23C0,
            bytes: vec![0b11_10_01_00],
        }]);
        let mut frame = Frame::default();
        screen.draw(&mut frame);
        assert_eq!(frame.pixel(0, 0), 0x11); // palette 0, colour 1
        // Tile (2, 2) is in the bottom-right quadrant: palette 3, colour 1.
        assert_eq!(frame.pixel(16, 16), 0x10 + 13);
    }

    #[test]
    fn sprites_overlap_and_hide_behind() {
        let mut screen = screen();
        screen.set_tile(1, 0, 1);
        let mut back = Sprite::new(8, 0, 1, 1);
        back.behind = true;
        let front = Sprite::new(12, 0, 1, 2);
        let mut over_backdrop = Sprite::new(40, 0, 1, 1);
        over_backdrop.behind = true;
        screen.sprites = vec![front, back, over_backdrop];
        let mut frame = Frame::default();
        screen.draw(&mut frame);
        // The front sprite overlaps the other from x 12; its colour 2 of palette 2.
        assert_eq!(frame.pixel(12, 0), 0x10 + 16 + 8 + 2);
        // The behind sprite shows nowhere over the solid background tile at x 8-11.
        assert_eq!(frame.pixel(9, 0), 0x11);
        // Where the background is transparent a behind sprite shows.
        assert_eq!(frame.pixel(41, 0), 0x10 + 16 + 4 + 2);
    }

    #[test]
    fn a_cell_of_its_own_palette_leaves_its_quadrant() {
        let mut screen = screen();
        screen.set_tile(0, 0, 1);
        screen.set_tile(1, 0, 1);
        screen.set_own_palette(1, 0, Some(3));
        let mut frame = Frame::default();
        screen.draw(&mut frame);
        // Cell 0 keeps the quadrant's palette 0, cell 1 shows palette 3.
        assert_eq!(frame.pixel(0, 0), 0x11);
        assert_eq!(frame.pixel(8, 0), 0x10 + 13);
        screen.set_own_palette(1, 0, None);
        screen.draw(&mut frame);
        assert_eq!(frame.pixel(8, 0), 0x11);
    }

    #[test]
    fn solid_scenery_hides_its_holes_and_outline() {
        // Scenery filling x 20-39, y 20-39, with a 3 x 3 hole at (28-30, 28-30).
        let mut values = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT];
        for y in 20..40 {
            for x in 20..40 {
                values[y * SCREEN_WIDTH + x] =
                    u8::from(!((28..31).contains(&x) && (28..31).contains(&y)));
            }
        }
        let at = |x: usize, y: usize| enclosed(&values, y * SCREEN_WIDTH + x);
        assert!(at(29, 29) && at(25, 25));
        // Just outside the edge (its outline) counts; two pixels out is sky.
        assert!(at(19, 30) && at(40, 40));
        assert!(!at(18, 30) && !at(100, 100));
        // With the switch on, a behind sprite over a hole is hidden.
        let mut screen = screen();
        screen.set_tile(1, 0, 1);
        let mut back = Sprite::new(16, 0, 1, 1);
        back.behind = true;
        screen.sprites = vec![back];
        let mut frame = Frame::default();
        screen.draw(&mut frame);
        let (next, further) = (frame.pixel(16, 0), frame.pixel(18, 0));
        assert!(next != 0x0F && further != 0x0F);
        // Next to the tile at x 8-15 the sprite is hidden, three pixels off it is not.
        screen.solid_behind = true;
        screen.draw(&mut frame);
        assert_eq!((frame.pixel(16, 0), frame.pixel(18, 0)), (0x0F, further));
    }

    #[test]
    fn text_uses_the_games_codes() {
        let mut screen = screen();
        screen.print(1, 1, "A-B, C. © 1");
        let row: Vec<u8> = (1..11).map(|c| screen.tile(c, 1)).collect();
        // "," and "." carry their own space.
        assert_eq!(
            row,
            [0x41, 0x5C, 0x42, 0x5E, 0x43, 0x5D, 0x3A, 0x00, 0x31, 0x00]
        );
    }
}
