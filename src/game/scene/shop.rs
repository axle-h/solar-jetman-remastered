//! The shop and the equipment list (`docs/spec/scenes.md` 6, `docs/spec/economy.md` 6): a strip
//! of item frames that scrolls 64 pixels, 32 frames, from one item to the next; the item's
//! name, price and count; buying (the price taken digit by digit) and fitting (EQUIP POD).
//!
//! The two screens are one scene with two modes. The shop is the screen of `graphics.md` 5.10
//! with its tiles in the alternative variant and a layout the program writes; the equip list
//! is the same screen's other variant, whose picture comes from the ROM.

use super::ppu::{Band, Screen, Sprite, char_tile};
use super::{Clock, FONT_LOADS};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::inventory::{self, Inventory, NIPPON_JETPOD};
use crate::game::gameplay::objects::Score;
use crate::game::graphics::{ChrLoad, Graphics, MissingGraphics, PalettePreset};
use crate::game::hud::{DIGIT_BOTTOM, DIGIT_TOP};

const TILES_ALT: &str = "tiles/frames-screen/tiles-alt.png";
const TILES: &str = "tiles/frames-screen/tiles.png";
const SQUARES: &str = "tiles/frames-screen/squares.png";
const EXTRA_DEVICES: &str = "tiles/frames-screen/squares-extra-a.png";
const EXTRA_PODS: &str = "tiles/frames-screen/squares-extra-b.png";
const STATUS_PANEL: (&str, u16) = ("tiles/hud/status-panel.png", 0x1E40);
const STATUS_PANEL_1D60: (&str, u16) = ("tiles/hud/status-panel-1d60.png", 0x1D60);
const ALT_ATTRIBUTES: &str = "pictures/frames-screen-alt-attributes.ron";
const EQUIP_PICTURE: &str = "pictures/frames-screen.ron";
/// Preset 4 gives colours 1-3 (graphics.md 7.4).
const PRESET_SCREEN: usize = 4;
/// The effect of a payment step and of a fit or un-fit (sound.md 8.3).
const MENU_SOUND: u8 = 0x06;

/// The tile of the first of an item's 3 × 3 picture tiles in the background table, and of its
/// sprites in the sprite table: nine to an item, the squares first, then the extras.
const PICTURE_TILE: u8 = 0x60;
const PICTURE_SPRITE: u8 = 0x80;
/// The strip: cell `k` starts at column 4 + 8k of the world, is 8 columns wide, and shows its
/// picture at columns 2-4 of the cell, rows 9-11.
const STRIP_FIRST_LINE: usize = 48;
const STRIP_END_LINE: usize = 104;
const STRIP_ROWS: usize = 8;
const STRIP_FIRST_ROW: usize = 6;
/// Frames of a move and its speed.
const MOVE_FRAMES: u32 = 32;
const MOVE_SPEED: usize = 2;
/// The item box's sprites: top-left corner in the shop and in the equip list.
const BOX_SHOP: (i32, i32) = (112, 167);
const BOX_EQUIP: (i32, i32) = (144, 167);
/// Fitted-item icons: first tile of slot 0, the rows and the x of the first, second, third.
const ICON_TILE: u8 = 0x13;
const ICON_Y: i32 = 213;
const ICON_X: [i32; 3] = [100, 124, 148];

/// Strip frame of the shop: tiles of an 8-column cell, rows 7-12 (the picture's cells are 0).
const SHOP_CELL: [[u8; 8]; 6] = [
    [0x19, 0x1A, 0x1A, 0x1A, 0x1A, 0x1A, 0x1B, 0x00],
    [0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1D, 0x00],
    [0x1E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F, 0x1A],
    [0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x25],
    [0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23, 0x00],
    [0x24, 0x25, 0x25, 0x25, 0x25, 0x25, 0x26, 0x00],
];
/// The same for the equip list: its columns 4-10 hold the frame, as the picture has it.
const EQUIP_CELL: [[u8; 8]; 6] = [
    [0x0A, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0B, 0x12],
    [0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0E, 0x12],
    [0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0E, 0x12],
    [0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0E, 0x12],
    [0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0E, 0x12],
    [0x0F, 0x10, 0x10, 0x10, 0x10, 0x10, 0x11, 0x12],
];

/// The item box of the shop (rows 19-24, columns 12-18).
const SHOP_BOX: [[u8; 7]; 6] = [
    [0x19, 0x1A, 0x1A, 0x1A, 0x1A, 0x1A, 0x1B],
    [0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1D],
    [0x1E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
    [0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21],
    [0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23],
    [0x24, 0x25, 0x25, 0x25, 0x25, 0x25, 0x26],
];
/// The logo's tiles: (row, first column, first tile, count); row 2 is split in two.
const LOGO: [(usize, usize, u8, usize); 5] = [
    (2, 12, 0x01, 1),
    (2, 15, 0x02, 3),
    (3, 12, 0x05, 6),
    (4, 12, 0x0B, 7),
    (5, 12, 0x12, 7),
];

/// Which screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Shop,
    Equip,
}

/// What a pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShopOutcome {
    Running,
    /// B: to the approach screen (shop) or the launch scene (equip list).
    Leave,
}

/// A purchase under way: the price's five digits (ten thousands first) and what is bought.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Payment {
    item: usize,
    digits: [u8; 5],
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Shop {
    /// What is on show: the strip's scroll as of the last pass, everything else as of the one
    /// before (the console's video memory takes the writes a frame after the pass makes them).
    screen: Screen,
    /// The screen as the last pass left it.
    work: Screen,
    /// The page and the picture's sprites of the pass before.
    behind: (Vec<u8>, Vec<Sprite>),
    mode: Mode,
    /// The items in cell order (cell `k + 1` holds `list[k]`).
    list: Vec<usize>,
    sel: usize,
    /// Pixels per frame of the move under way (0, 2 or -2) and the frames done.
    dir: i32,
    moved: u32,
    scroll: usize,
    payment: Option<Payment>,
    /// An A in the equip list is acted on in the next pass.
    fit_pending: bool,
    box_sprites: Vec<Sprite>,
    icon_sprites: Vec<Sprite>,
    /// Sound effects asked for in the last pass, each on both pulses (sound.md 8.3).
    sounds: Vec<u8>,
}

impl Shop {
    /// Builds the screen. `previous` is the scene before (its sprite palettes and tiles stay);
    /// `planet` the level index; `inventory` and `score` as they stand when it opens, with
    /// what was fitted already returned.
    pub fn new(
        graphics: &Graphics,
        mode: Mode,
        planet: usize,
        inventory: &Inventory,
        score: Score,
        previous: Option<&Screen>,
    ) -> Result<Shop, MissingGraphics> {
        let mut screen = Screen::default();
        if let Some(previous) = previous {
            screen.chr = previous.chr.clone();
            screen.palette = previous.palette;
        }
        let pods_on_offer = planet & 3 == 3;
        let mut loads = FONT_LOADS[..2].to_vec();
        loads.extend([STATUS_PANEL, STATUS_PANEL_1D60]);
        screen.load_tiles(graphics, &loads)?;
        let stream = |path: &'static str| {
            graphics
                .tile_streams
                .get(path)
                .ok_or_else(|| MissingGraphics(format!("tile stream {path}")))
        };
        let whole = |stream, address| ChrLoad {
            stream,
            range: None,
            address,
        };
        // Table 0 gets the same stream in both variants; table 1 differs (graphics.md 5.10).
        screen.chr.apply(graphics, &[whole(TILES, 0x0000)])?;
        screen.chr.apply(
            graphics,
            &[whole(
                match mode {
                    Mode::Shop => TILES_ALT,
                    Mode::Equip => TILES,
                },
                0x1000,
            )],
        )?;
        // Tile 1 of table 0 is sprite 0's one-pixel marker.
        let mut marker = [0u8; 16];
        marker[0] = 0x80;
        screen.chr.write(0x0010, &marker);
        let squares_len = stream(SQUARES)?.len();
        let extras = if pods_on_offer {
            EXTRA_PODS
        } else {
            EXTRA_DEVICES
        };
        for base in [0x0800u16, 0x1600] {
            screen.chr.apply(graphics, &[whole(SQUARES, base)])?;
            screen.chr.apply(
                graphics,
                &[whole(extras, base + 0x10 * (squares_len / 16) as u16)],
            )?;
        }
        // The solid tile of table 0 (graphics.md 3).
        screen.chr.write(0x0FE0, &[0xFF; 16]);
        screen.sprite_table = 0;
        screen.background_table = 1;
        screen.scroll = (0, 2);
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen.refresh();

        // The picture and the palette.
        let sets = &graphics.frames_screen;
        let variant = match mode {
            Mode::Shop => 0,
            Mode::Equip => 1,
        };
        let table = sets
            .get(variant)
            .ok_or_else(|| MissingGraphics("the colours of the shop screen".to_owned()))?;
        for (i, &colour) in table.iter().enumerate().skip(1) {
            screen.palette.0[1 + i] = colour;
        }
        let preset: &PalettePreset = graphics
            .presets
            .get(PRESET_SCREEN)
            .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET_SCREEN}")))?;
        screen.palette.apply(preset);
        screen.palette.0[16] = 0x0F;
        let picture = match mode {
            Mode::Shop => ALT_ATTRIBUTES,
            Mode::Equip => EQUIP_PICTURE,
        };
        screen.load_picture(
            graphics
                .pictures
                .get(picture)
                .ok_or_else(|| MissingGraphics(picture.to_owned()))?,
        );

        let list = match mode {
            Mode::Shop => inventory.stock(planet),
            Mode::Equip => inventory.equip_list(),
        };
        let mut shop = Shop {
            screen: screen.clone(),
            work: screen,
            behind: (Vec::new(), Vec::new()),
            mode,
            list,
            sel: 0,
            dir: 0,
            moved: 0,
            scroll: 0,
            payment: None,
            fit_pending: false,
            box_sprites: Vec::new(),
            icon_sprites: Vec::new(),
            sounds: Vec::new(),
        };
        shop.lay_out(score);
        shop.build_strip();
        shop.show_item(inventory);
        shop.show_icons(inventory);
        shop.screen = shop.work.clone();
        shop.screen.sprites = shop
            .box_sprites
            .iter()
            .chain(&shop.icon_sprites)
            .copied()
            .collect();
        shop.behind = (shop.work.page_bytes().to_vec(), shop.box_sprites.clone());
        Ok(shop)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The effects the passes since the last call asked for, each on both pulses.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// Whether the strip is moving (tests).
    pub fn moving(&self) -> bool {
        self.dir != 0
    }

    /// Whether nothing is moving or being paid (tests).
    pub fn idle(&self) -> bool {
        self.dir == 0 && self.payment.is_none()
    }

    /// The items the screen lists, in order.
    pub fn list(&self) -> &[usize] {
        &self.list
    }

    /// The cell on show and the strip's scroll in pixels (tests).
    pub fn position(&self) -> (usize, usize) {
        (self.sel, self.scroll)
    }

    /// The item on show, if the list has any.
    fn current(&self) -> Option<usize> {
        self.list.get(self.sel).copied()
    }

    /// The static parts of the page.
    fn lay_out(&mut self, score: Score) {
        let screen = &mut self.work;
        if self.mode == Mode::Shop {
            for (row, column, first, count) in LOGO {
                for i in 0..count {
                    screen.set_tile(column + i, row, first + i as u8);
                }
            }
            for (r, cells) in SHOP_BOX.iter().enumerate() {
                for (c, &tile) in cells.iter().enumerate() {
                    screen.set_tile(12 + c, 19 + r, tile);
                }
            }
            screen.print(21, 22, "IN YOUR");
            screen.print(21, 23, "ACCOUNT");
            self.show_score(score);
        }
        let screen = &mut self.work;
        screen.print(3, 20, "YOU OWN");
        screen.print(3, 23, "OF THEM");
        screen.print(2, 26, "FIRE B");
        screen.print(2, 27, "TO EXIT");
        screen.print(23, 26, "FIRE A");
        screen.print(22, 27, "TO SELECT");
    }

    fn show_score(&mut self, score: Score) {
        let digits = format!("{:06}", score.0 % 1_000_000);
        for (i, d) in digits.bytes().enumerate() {
            let d = usize::from(d - b'0');
            self.work.set_tile(21 + i, 20, DIGIT_TOP[d]);
            self.work.set_tile(21 + i, 21, DIGIT_BOTTOM[d]);
        }
    }

    /// The tile of an item's picture: items 14 and 15 show the pictures of 12 and 13.
    fn picture_of(item: usize) -> usize {
        if item >= NIPPON_JETPOD {
            item - 2
        } else {
            item
        }
    }

    /// Writes the world of the strip: `columns` wide for every cell and the screen's width
    /// past the last.
    fn build_strip(&mut self) {
        let cells = self.list.len() + 1;
        let columns = 8 * cells + 36;
        let template = match self.mode {
            Mode::Shop => &SHOP_CELL,
            Mode::Equip => &EQUIP_CELL,
        };
        let mut tiles = vec![0u8; columns * STRIP_ROWS];
        // Rows 7-12 are the frames; the equip list's picture has a pipe above and below.
        for row in 0..STRIP_ROWS {
            for column in 0..columns {
                tiles[row * columns + column] = match (row, self.mode) {
                    (0, Mode::Equip) => 0x08,
                    (7, Mode::Equip) => 0x09,
                    (1..=6, _) => template[row - 1][(column + 4) % 8],
                    _ => 0,
                };
            }
        }
        for (k, &item) in self.list.iter().enumerate() {
            let first = Self::picture_of(item);
            let x0 = 4 + 8 * (k + 1);
            for r in 0..3 {
                for c in 0..3 {
                    tiles[(3 + r) * columns + x0 + 2 + c] =
                        PICTURE_TILE + (9 * first + 3 * r + c) as u8;
                }
            }
        }
        // The page's attributes repeat every 32 columns.
        let page = self.work.page_bytes();
        let mut palettes = vec![0u8; columns * STRIP_ROWS];
        for row in 0..STRIP_ROWS {
            for column in 0..columns {
                let (r, c) = (STRIP_FIRST_ROW + row, column % 32);
                let byte = page[960 + (r / 4) * 8 + c / 4];
                let shift = ((r / 2) & 1) * 4 + ((c / 2) & 1) * 2;
                palettes[row * columns + column] = (byte >> shift) & 3;
            }
        }
        self.work.band = Some(Band {
            first_line: STRIP_FIRST_LINE,
            end_line: STRIP_END_LINE,
            first_row: STRIP_FIRST_ROW,
            scroll_x: self.scroll,
            columns,
            tiles,
            palettes,
        });
    }

    /// The item's name, price, count and picture.
    fn show_item(&mut self, inventory: &Inventory) {
        let Some(item) = self.current() else {
            self.clear_item();
            return;
        };
        let name = inventory::NAMES[item];
        let start = (32 - name.len()) / 2;
        for column in 0..28 {
            self.work.set_tile(column, 15, 0);
        }
        for (i, ch) in name.chars().enumerate() {
            if start + i < 28 {
                self.work.set_tile(start + i, 15, char_tile(ch));
            }
        }
        if self.mode == Mode::Shop {
            let digits = format!("{:05}", inventory::price(item));
            for (i, d) in digits.bytes().enumerate() {
                let d = usize::from(d - b'0');
                self.work.set_tile(13 + i, 17, DIGIT_TOP[d]);
                self.work.set_tile(13 + i, 18, DIGIT_BOTTOM[d]);
            }
        }
        self.show_count(inventory);
    }

    /// The count (one digit) and, with one or more, the box's picture.
    fn show_count(&mut self, inventory: &Inventory) {
        let Some(item) = self.current() else { return };
        let count = inventory.owned(item).min(9);
        let digit = usize::from(count);
        self.work.set_tile(6, 21, DIGIT_TOP[digit]);
        self.work.set_tile(6, 22, DIGIT_BOTTOM[digit]);
        self.box_sprites.clear();
        if count >= 1 && self.dir == 0 {
            let (x, y) = match self.mode {
                Mode::Shop => BOX_SHOP,
                Mode::Equip => BOX_EQUIP,
            };
            let first = PICTURE_SPRITE + (9 * Self::picture_of(item)) as u8;
            for r in 0..3 {
                for c in 0..3 {
                    self.box_sprites.push(Sprite::new(
                        x + 8 * c,
                        y + 8 * r,
                        first + (3 * r + c) as u8,
                        0,
                    ));
                }
            }
        }
    }

    /// Blanks the name, the count and the picture (a move starts).
    fn clear_item(&mut self) {
        for column in 0..28 {
            self.work.set_tile(column, 15, 0);
        }
        self.work.set_tile(6, 21, 0);
        self.work.set_tile(6, 22, 0);
        self.box_sprites.clear();
    }

    /// The icons of the fittings (equip list only).
    fn show_icons(&mut self, inventory: &Inventory) {
        self.icon_sprites.clear();
        if self.mode == Mode::Equip {
            let fitted = [inventory.bullets, inventory.weapon, inventory.device];
            let mut at = 0;
            for (slot, &value) in fitted.iter().enumerate() {
                if value == 0 {
                    continue;
                }
                let first = ICON_TILE + 4 * slot as u8;
                for (i, &(dx, dy)) in [(0, 0), (8, 0), (0, 8), (8, 8)].iter().enumerate() {
                    self.icon_sprites.push(Sprite::new(
                        ICON_X[at] + dx,
                        ICON_Y + dy,
                        first + i as u8,
                        0,
                    ));
                }
                at += 1;
            }
        }
    }

    /// One pass. `inventory` and `score` are the player's; the shop changes them.
    pub fn step(
        &mut self,
        clock: &mut Clock,
        pressed: Buttons,
        inventory: &mut Inventory,
        score: &mut Score,
    ) -> ShopOutcome {
        let outcome = self.pass(clock, pressed, inventory, score);
        // The page and the item's picture show what the pass before made; the strip's scroll
        // and the fitted-item icons (sprite memory, copied before the page is) this pass's own.
        self.screen.set_page(&self.behind.0);
        self.screen.sprites = std::mem::take(&mut self.behind.1);
        self.screen
            .sprites
            .extend(self.icon_sprites.iter().copied());
        self.behind = (self.work.page_bytes().to_vec(), self.box_sprites.clone());
        if let (Some(shown), Some(work)) = (&mut self.screen.band, &self.work.band) {
            shown.scroll_x = work.scroll_x;
        }
        outcome
    }

    fn pass(
        &mut self,
        clock: &mut Clock,
        pressed: Buttons,
        inventory: &mut Inventory,
        score: &mut Score,
    ) -> ShopOutcome {
        clock.counter = (clock.counter + 1) & 0xFF_FFFF;
        clock.job = (clock.job + 1) & 7;
        if self.payment.is_some() {
            if clock.counter.is_multiple_of(8) {
                self.pay(inventory, score);
            }
            return ShopOutcome::Running;
        }
        if std::mem::take(&mut self.fit_pending)
            && self.dir == 0
            && let Some(item) = self.current()
        {
            inventory.fit(item);
            self.show_count(inventory);
            self.show_icons(inventory);
        }
        // The strip scrolls.
        if self.dir != 0 {
            if self.moved == 0 && self.dir < 0 {
                self.sel -= 1;
            }
            if self.moved == 0 {
                self.clear_item();
            }
            self.scroll = self.scroll.wrapping_add_signed(self.dir as isize);
            self.moved += 1;
            if let Some(band) = &mut self.work.band {
                band.scroll_x = self.scroll;
            }
            if self.moved == MOVE_FRAMES {
                if self.dir > 0 {
                    self.sel += 1;
                }
                self.dir = 0;
                self.moved = 0;
                self.show_item(inventory);
            }
        }
        if pressed.has(Buttons::B) {
            return ShopOutcome::Leave;
        }
        if self.dir != 0 {
            return ShopOutcome::Running;
        }
        if pressed.has(Buttons::LEFT) && self.sel > 0 {
            self.dir = -(MOVE_SPEED as i32);
        } else if pressed.has(Buttons::RIGHT) && self.sel + 1 < self.list.len() {
            self.dir = MOVE_SPEED as i32;
        } else if pressed.has(Buttons::A) {
            if self.mode == Mode::Equip {
                self.fit_pending = true;
                // The effect is requested in the pass that reads the button, one before the
                // fit shows (Mesen, `sj-tools scene --compare`).
                if let Some(item) = self.current()
                    && inventory.owned(item) >= 1
                {
                    self.sounds.push(MENU_SOUND);
                }
            } else {
                self.press_a(inventory, *score);
            }
        }
        ShopOutcome::Running
    }

    /// A: buy (shop) or fit (equip list).
    fn press_a(&mut self, inventory: &mut Inventory, score: Score) {
        let Some(item) = self.current() else { return };
        match self.mode {
            Mode::Equip => inventory.fit(item),
            Mode::Shop => {
                if inventory.check(item, score).is_ok() {
                    let price = inventory::price(item);
                    let mut digits = [0u8; 5];
                    for (i, d) in format!("{price:05}").bytes().enumerate() {
                        digits[i] = d - b'0';
                    }
                    self.payment = Some(Payment { item, digits });
                }
            }
        }
    }

    /// A frame of the payment in which the counter is a multiple of 8: the highest digit of the
    /// price goes down by one and the score loses that power of ten; with nothing left to pay
    /// the item is owned.
    fn pay(&mut self, inventory: &mut Inventory, score: &mut Score) {
        let Some(payment) = &mut self.payment else {
            return;
        };
        if let Some(at) = payment.digits.iter().position(|&d| d != 0) {
            payment.digits[at] -= 1;
            score.0 = score.0.saturating_sub(10u32.pow(4 - at as u32));
            self.show_score(*score);
            self.sounds.push(MENU_SOUND);
        } else {
            let item = payment.item;
            self.payment = None;
            inventory.grant(item);
            self.show_item(inventory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::graphics::PictureWrite;

    fn graphics() -> Graphics {
        let mut graphics = Graphics::default();
        for (path, tiles) in [
            ("tiles/font/digits.png", 12),
            ("tiles/font/letters.png", 31),
            (STATUS_PANEL.0, 27),
            (STATUS_PANEL_1D60.0, 5),
            (TILES_ALT, 48),
            (TILES, 32),
            (SQUARES, 108),
            (EXTRA_DEVICES, 18),
            (EXTRA_PODS, 18),
        ] {
            graphics
                .tile_streams
                .insert(path.to_owned(), vec![0x5A; tiles * 16]);
        }
        graphics.presets = vec![
            PalettePreset {
                first_entry: 0,
                colours: vec![1, 2, 3],
            };
            5
        ];
        graphics.presets[4] = PalettePreset {
            first_entry: 1,
            colours: vec![0x38, 0x22, 0x02],
        };
        graphics.frames_screen = vec![vec![0x11; 15], vec![0x22; 15]];
        graphics.pictures.insert(
            ALT_ATTRIBUTES.to_owned(),
            vec![PictureWrite {
                address: 0x23C0,
                bytes: vec![0; 64],
            }],
        );
        graphics.pictures.insert(
            EQUIP_PICTURE.to_owned(),
            vec![PictureWrite {
                address: 0x2000,
                bytes: vec![0; 832],
            }],
        );
        graphics
    }

    fn shop(mode: Mode, planet: usize, inventory: &Inventory, score: u32) -> Shop {
        Shop::new(&graphics(), mode, planet, inventory, Score(score), None).unwrap()
    }

    /// One pass with `buttons` newly pressed.
    fn pass(
        shop: &mut Shop,
        clock: &mut Clock,
        inventory: &mut Inventory,
        score: &mut Score,
        buttons: u8,
    ) -> ShopOutcome {
        shop.step(clock, Buttons(buttons), inventory, score)
    }

    #[test]
    fn a_move_takes_32_frames_and_never_wraps() {
        let mut inventory = Inventory::default();
        let mut shop = shop(Mode::Shop, 1, &inventory, 1000);
        let (mut clock, mut score) = (Clock::default(), Score(1000));
        assert_eq!(shop.list(), [0, 3, 6, 10, 12]);
        // Left at the start does nothing.
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::LEFT,
        );
        assert!(!shop.moving());
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::RIGHT,
        );
        assert!(shop.moving());
        for _ in 0..31 {
            pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        }
        // The press was seen in the first pass; 32 passes of motion follow it.
        assert!(shop.moving());
        pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        assert_eq!(shop.position(), (1, 64));
        // The last item: further presses are ignored.
        for _ in 0..3 {
            pass(
                &mut shop,
                &mut clock,
                &mut inventory,
                &mut score,
                Buttons::RIGHT,
            );
            for _ in 0..32 {
                pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
            }
        }
        assert_eq!(shop.position(), (4, 256));
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::RIGHT,
        );
        assert!(!shop.moving());
    }

    #[test]
    fn left_lowers_the_item_at_once_and_b_leaves_in_the_middle_of_a_move() {
        let mut inventory = Inventory::default();
        let mut shop = shop(Mode::Shop, 1, &inventory, 0);
        let (mut clock, mut score) = (Clock::default(), Score(0));
        for _ in 0..2 {
            pass(
                &mut shop,
                &mut clock,
                &mut inventory,
                &mut score,
                Buttons::RIGHT,
            );
            for _ in 0..32 {
                pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
            }
        }
        assert_eq!(shop.position(), (2, 128));
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::LEFT,
        );
        pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        assert_eq!(shop.position(), (1, 126));
        assert_eq!(
            pass(
                &mut shop,
                &mut clock,
                &mut inventory,
                &mut score,
                Buttons::B
            ),
            ShopOutcome::Leave
        );
    }

    #[test]
    fn buying_takes_the_price_digit_by_digit_on_multiples_of_8() {
        let mut inventory = Inventory::default();
        let mut shop = shop(Mode::Shop, 1, &inventory, 700);
        let (mut clock, mut score) = (Clock::default(), Score(700));
        clock.counter = 3;
        // Item 0 costs 400: A, then a step at every counter multiple of 8, four of them, and
        // the item the step after.
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::A,
        );
        let mut seen = Vec::new();
        for _ in 0..60 {
            pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
            if seen.last() != Some(&(score.0, inventory.counts[0])) {
                seen.push((score.0, inventory.counts[0]));
            }
        }
        assert_eq!(
            seen,
            [(700, 0), (600, 0), (500, 0), (400, 0), (300, 0), (300, 1)]
        );
    }

    #[test]
    fn a_refused_purchase_changes_nothing() {
        let mut inventory = Inventory::default();
        let mut shop = shop(Mode::Shop, 1, &inventory, 399);
        let (mut clock, mut score) = (Clock::default(), Score(399));
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::A,
        );
        for _ in 0..40 {
            pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        }
        assert_eq!((score.0, inventory.counts[0]), (399, 0));
        assert!(shop.idle());
    }

    #[test]
    fn the_equip_list_fits_on_the_pass_after_a_and_shows_icons() {
        let mut inventory = Inventory::default();
        inventory.counts[0] = 2;
        inventory.counts[8] = 1;
        let mut shop = shop(Mode::Equip, 5, &inventory, 0);
        let (mut clock, mut score) = (Clock::default(), Score(0));
        assert_eq!(shop.list(), [0, 8]);
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::A,
        );
        assert_eq!(inventory.weapon, 0);
        pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        assert_eq!((inventory.weapon, inventory.counts[0]), (1, 1));
        // The icon of the weapon slot (slot 1: tiles $17-$1A) at the first place.
        let icons: Vec<_> = shop
            .screen()
            .sprites
            .iter()
            .filter(|s| s.y >= 200)
            .map(|s| (s.x, s.y, s.tile))
            .collect();
        assert_eq!(
            icons,
            [
                (100, 213, 0x17),
                (108, 213, 0x18),
                (100, 221, 0x19),
                (108, 221, 0x1A)
            ]
        );
        // A second press removes it again (it is already fitted).
        pass(
            &mut shop,
            &mut clock,
            &mut inventory,
            &mut score,
            Buttons::A,
        );
        pass(&mut shop, &mut clock, &mut inventory, &mut score, 0);
        assert_eq!((inventory.weapon, inventory.counts[0]), (0, 2));
    }
}
