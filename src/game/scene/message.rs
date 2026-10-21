//! The message screens (`docs/spec/scenes.md` 10, `docs/spec/game-flow.md` 7.4, `docs/spec/text.md`
//! 2-3): what the mothership says when an item is pulled in (kinds 1-5), the congratulations
//! (kind 7) and the warp-level introduction (kind 6, with its sprite lettering). The gameplay
//! status panel stays below the frame. One call of [`Message::step`] is one pass of the
//! original's handler.

use super::ppu::{Screen, Sprite};
use super::strings::{self, ANALYSIS_DIGITS, CARGO, Line};
use super::{Clock, FONT_LOADS};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{
    Chr, Graphics, MissingGraphics, Palette, PatternTable, gameplay_background_loads,
};
use crate::game::hud::Hud;
use crate::game::level::Level;

const FRAME_AND_FONT: (&str, u16) = ("tiles/menus/frame-and-font.png", 0x1000);
const STATUS_PANEL: (&str, u16) = ("tiles/hud/status-panel.png", 0x1E40);
const STATUS_PANEL_1D60: (&str, u16) = ("tiles/hud/status-panel-1d60.png", 0x1D60);
const PICTURE_B: &str = "pictures/menu-b.ron";
const PICTURE_C: &str = "pictures/menu-c.ron";
const PICTURE_D: &str = "pictures/menu-d.ron";
const PICTURE_COMET: &str = "pictures/comet.ron";
const COMET_BACKGROUND: (&str, u16) = ("tiles/comet/table-1.png", 0x1000);
const COMET_SPRITES: (&str, u16) = ("tiles/comet/table-0.png", 0x0000);
/// The item pictures (graphics.md 4.4) load at $1810.
const ITEM_PICTURES: [&str; 3] = [
    "tiles/equipment/picture-0.png",
    "tiles/equipment/picture-1.png",
    "tiles/equipment/picture-2.png",
];
/// The spaceship picture of the screen of bank 2 entry $8036 (graphics.md 4.2, 7.2): its tiles at
/// $1800 and its attribute bytes, which the warpship messages (47, 48) draw their text over.
/// The shop's sprite tiles (graphics.md 5.8), which the analysis screen loads into sprite table 0:
/// the container's art.
const SHOP_TILES: (&str, u16) = ("tiles/equipment/shop.png", 0x0800);
const SHOP_BLOCKS: &str = "tiles/equipment/shop-blocks.png";
const SHOP_BLOCK_BYTES: usize = 0x120;
const SPACESHIP_TILES: (&str, u16) = ("tiles/unidentified-a/tiles.png", 0x1800);
const SPACESHIP_ATTRIBUTES: &str = "pictures/unidentified-a.ron";
const PRESET_FRAME_B: usize = 24;
const PRESET_FRAME_C: usize = 25;
/// The analysis screen's frame is menu screen D's (the equipment frame, preset 26).
const PRESET_FRAME_D: usize = 26;
/// The warpship piece's box takes entries 8-15 from preset 6's data.
const PRESET_WARPSHIP: usize = 6;
const PRESET_COMET: usize = 21;
/// Frames the typewriter waits for each item.
const TYPE_EVERY: u32 = 4;
/// Message timers $DA at the start (game-flow.md 7.4).
pub const TIMER_ITEM: u8 = 240;
pub const TIMER_WARP: u8 = 192;
pub const TIMER_FINAL: u8 = 255;

/// The four kinds of picture a message screen can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Frame {
    /// The plain box (menu screen B).
    Plain,
    /// The decorated frame (menu screen C).
    Decorated,
    /// The analysis screen's frame (menu screen D).
    Analysis,
    /// Nothing but the cleared page (an empty message).
    Blank,
}

/// One tile of the item pictures: the cells written at once (scenes.md 10, "Tile pictures"):
/// (row, first column, count) of consecutive tiles from $81.
const SHIELDS: [(usize, usize, usize); 8] = [
    (9, 14, 4),
    (10, 12, 3),
    (10, 17, 3),
    (11, 12, 8),
    (12, 13, 6),
    (13, 14, 4),
    (0, 0, 0),
    (0, 0, 0),
];
const BOOSTERS: [(usize, usize, usize); 8] = [
    (8, 15, 2),
    (9, 14, 4),
    (10, 14, 4),
    (11, 14, 4),
    (12, 13, 6),
    (13, 13, 6),
    (14, 13, 6),
    (0, 0, 0),
];
const NAVIGATORS: [(usize, usize, usize); 8] = [
    (8, 15, 2),
    (9, 14, 4),
    (10, 14, 4),
    (11, 12, 8),
    (12, 12, 8),
    (13, 14, 4),
    (14, 15, 2),
    (15, 15, 2),
];

/// The picture of the completed warpship (message 48): rows of consecutive cells, (row, first
/// column, tiles). Tile $80 is blank.
const WARPSHIP: [(usize, usize, &[u8]); 6] = [
    (8, 12, &[0x83, 0x84, 0x85, 0x86, 0x80, 0x80]),
    (9, 12, &[0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E]),
    (
        10,
        10,
        &[0x8F, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98],
    ),
    (
        11,
        10,
        &[0x99, 0x9A, 0x9B, 0x9B, 0x9B, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F],
    ),
    (12, 12, &[0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5]),
    (13, 12, &[0x80; 6]),
];

/// The twelve pieces of the warpship picture (scenes.md 10, item 4): nametable row and column of
/// the 2 × 2 block, and its tiles (top-left, top-right, bottom-left, bottom-right).
const PIECES: [(usize, usize, [u8; 4]); 12] = [
    (8, 10, [0x81, 0x82, 0x87, 0x88]),
    (10, 14, [0x93, 0x94, 0x9B, 0x9B]),
    (8, 16, [0x80, 0x80, 0x8D, 0x8E]),
    (12, 12, [0xA0, 0xA1, 0x80, 0x80]),
    (10, 18, [0x97, 0x98, 0x9E, 0x9F]),
    (12, 16, [0xA4, 0xA5, 0x80, 0x80]),
    (10, 12, [0x91, 0x92, 0x9B, 0x9B]),
    (8, 12, [0x83, 0x84, 0x89, 0x8A]),
    (10, 16, [0x95, 0x96, 0x9C, 0x9D]),
    (8, 14, [0x85, 0x86, 0x8B, 0x8C]),
    (10, 10, [0x8F, 0x90, 0x99, 0x9A]),
    (12, 14, [0xA2, 0xA3, 0x80, 0x80]),
];
/// Passes from the last letter to the piece's lower pair of cells, and the effect that goes
/// with the lower pair (sound.md 8.3).
const PIECE_DELAY: u8 = 5;
pub const PIECE_SOUND: u8 = 0x40;

/// A sprite of an item picture: x, y of the top-left pixel, tile, palette and flips.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Spot {
    x: i32,
    y: i32,
    tile: u8,
    palette: u8,
    flip_x: bool,
    flip_y: bool,
}

const fn spot(x: i32, y: i32, tile: u8, palette: u8, flips: u8) -> Spot {
    Spot {
        x,
        y,
        tile,
        palette,
        flip_x: flips & 1 != 0,
        flip_y: flips & 2 != 0,
    }
}

const ALIEN: [Spot; 4] = [
    spot(127, 33, 0x00, 0, 0),
    spot(134, 33, 0x00, 0, 1),
    spot(127, 41, 0x01, 1, 0),
    spot(134, 41, 0x01, 1, 1),
];
const RADIOACTIVE: [Spot; 4] = [
    spot(127, 33, 0xD6, 1, 0),
    spot(135, 33, 0xD6, 1, 1),
    spot(127, 41, 0xD7, 1, 0),
    spot(135, 41, 0xD7, 1, 1),
];
const fn crystal(palette: u8) -> [Spot; 4] {
    [
        spot(119, 81, 0xDB, palette, 0),
        spot(127, 81, 0xDB, palette, 1),
        spot(119, 89, 0xDC, palette, 0),
        spot(127, 89, 0xDC, palette, 1),
    ]
}
const FUEL: [Spot; 10] = [
    spot(119, 69, 0x89, 3, 0),
    spot(127, 69, 0x89, 3, 0),
    spot(111, 73, 0x88, 3, 0),
    spot(135, 73, 0x88, 3, 1),
    spot(119, 77, 0x8A, 3, 0),
    spot(127, 77, 0x8B, 3, 0),
    spot(111, 81, 0x88, 3, 2),
    spot(135, 81, 0x88, 3, 3),
    spot(119, 85, 0x89, 3, 2),
    spot(127, 85, 0x89, 3, 2),
];
/// The analysis container's picture in its six stages (measured from the original's sprite memory):
/// (x, y of the top-left pixel, tile of the shop's sprite tiles, flipped horizontally), in front-to-back order, all in sprite palette 2.
const CONTAINER: [&[(i32, i32, u8, bool)]; 6] = [
    &[
        (155, 54, 0xC9, true),
        (147, 54, 0xCA, true),
        (139, 54, 0xCB, true),
        (155, 46, 0xC5, true),
        (147, 46, 0xC6, true),
        (139, 46, 0xC7, true),
        (131, 46, 0xC8, true),
        (155, 38, 0xC3, true),
        (147, 38, 0xC4, true),
        (155, 30, 0xC1, true),
        (147, 30, 0xC2, true),
        (109, 54, 0xC9, false),
        (117, 54, 0xCA, false),
        (125, 54, 0xCB, false),
        (109, 46, 0xC5, false),
        (117, 46, 0xC6, false),
        (125, 46, 0xC7, false),
        (133, 46, 0xC8, false),
        (109, 38, 0xC3, false),
        (117, 38, 0xC4, false),
        (109, 30, 0xC1, false),
        (117, 30, 0xC2, false),
        (120, 38, 0x99, false),
        (128, 38, 0x9A, false),
        (136, 38, 0x9A, true),
        (144, 38, 0x99, true),
        (120, 30, 0x97, false),
        (128, 30, 0x98, false),
        (136, 30, 0x98, true),
        (144, 30, 0x97, true),
        (122, 22, 0x96, false),
        (126, 22, 0x96, false),
        (136, 22, 0x96, false),
        (142, 22, 0x96, false),
    ],
    &[
        (120, 38, 0x94, false),
        (128, 38, 0x95, false),
        (136, 38, 0x95, true),
        (144, 38, 0x94, true),
        (120, 30, 0x92, false),
        (128, 30, 0x93, false),
        (136, 30, 0x93, true),
        (144, 30, 0x92, true),
        (120, 22, 0x90, false),
        (128, 22, 0x91, false),
        (136, 22, 0x91, true),
        (144, 22, 0x90, true),
        (108, 54, 0xBE, false),
        (116, 54, 0xBF, false),
        (124, 54, 0xC0, false),
        (108, 46, 0xBB, false),
        (116, 46, 0xBC, false),
        (124, 46, 0xBD, false),
        (108, 38, 0xB9, false),
        (116, 38, 0xBA, false),
        (108, 30, 0xB7, false),
        (116, 30, 0xB8, false),
        (156, 54, 0xBE, true),
        (148, 54, 0xBF, true),
        (140, 54, 0xC0, true),
        (156, 46, 0xBB, true),
        (148, 46, 0xBC, true),
        (140, 46, 0xBD, true),
        (156, 38, 0xB9, true),
        (148, 38, 0xBA, true),
        (156, 30, 0xB7, true),
        (148, 30, 0xB8, true),
    ],
    &[
        (120, 35, 0x8E, false),
        (128, 35, 0x8F, false),
        (136, 35, 0x8F, true),
        (144, 35, 0x8E, true),
        (120, 27, 0x8C, false),
        (128, 27, 0x8D, false),
        (136, 27, 0x8D, true),
        (144, 27, 0x8C, true),
        (107, 54, 0xB4, false),
        (115, 54, 0xB5, false),
        (123, 54, 0xB6, false),
        (107, 46, 0xB1, false),
        (115, 46, 0xB2, false),
        (123, 46, 0xB3, false),
        (107, 38, 0xAF, false),
        (115, 38, 0xB0, false),
        (107, 30, 0xAD, false),
        (115, 30, 0xAE, false),
        (157, 54, 0xB4, true),
        (149, 54, 0xB5, true),
        (141, 54, 0xB6, true),
        (157, 46, 0xB1, true),
        (149, 46, 0xB2, true),
        (141, 46, 0xB3, true),
        (157, 38, 0xAF, true),
        (149, 38, 0xB0, true),
        (157, 30, 0xAD, true),
        (149, 30, 0xAE, true),
    ],
    &[
        (120, 26, 0x8A, false),
        (128, 26, 0x8B, false),
        (136, 26, 0x8B, true),
        (144, 26, 0x8A, true),
        (120, 22, 0x88, false),
        (128, 22, 0x89, false),
        (136, 22, 0x89, true),
        (144, 22, 0x88, true),
        (106, 54, 0xAB, false),
        (114, 54, 0xAC, false),
        (106, 46, 0xA9, false),
        (114, 46, 0xAA, false),
        (106, 38, 0xA7, false),
        (114, 38, 0xA8, false),
        (106, 30, 0xA6, false),
        (158, 54, 0xAB, true),
        (150, 54, 0xAC, true),
        (158, 46, 0xA9, true),
        (150, 46, 0xAA, true),
        (158, 38, 0xA7, true),
        (150, 38, 0xA8, true),
        (158, 30, 0xA6, true),
    ],
    &[
        (120, 23, 0x86, false),
        (128, 23, 0x87, false),
        (136, 23, 0x87, true),
        (144, 23, 0x86, true),
        (105, 54, 0xA5, false),
        (105, 46, 0xA4, false),
        (105, 38, 0xA3, false),
        (105, 30, 0xA2, false),
        (159, 54, 0xA5, true),
        (159, 46, 0xA4, true),
        (159, 38, 0xA3, true),
        (159, 30, 0xA2, true),
    ],
    &[
        (171, 54, 0xA0, true),
        (163, 54, 0xA1, true),
        (171, 46, 0x9E, true),
        (163, 46, 0x9F, true),
        (171, 38, 0x9C, true),
        (163, 38, 0x9D, true),
        (163, 30, 0x9B, true),
        (93, 54, 0xA0, false),
        (101, 54, 0xA1, false),
        (93, 46, 0x9E, false),
        (101, 46, 0x9F, false),
        (93, 38, 0x9C, false),
        (101, 38, 0x9D, false),
        (101, 30, 0x9B, false),
        (120, 23, 0x82, false),
        (128, 23, 0x83, false),
        (136, 23, 0x84, false),
        (144, 23, 0x85, false),
        (120, 15, 0x80, false),
        (128, 15, 0x81, false),
        (136, 15, 0x81, true),
        (144, 15, 0x80, true),
    ],
];

/// The container opens from pass 97 on, a stage every 8 passes (the digit windows end at 96).
const CONTAINER_FIRST_OPEN: u32 = 97;
const CONTAINER_STAGE_PASSES: u32 = 8;

/// The container's sprites in stage `stage` (0-5).
fn container_spots(stage: usize) -> Vec<Spot> {
    CONTAINER[stage.min(5)]
        .iter()
        .map(|&(x, y, tile, flip)| spot(x, y, tile, 2, u8::from(flip)))
        .collect()
}

/// The lettering of the warp-level screen: (x of the first sprite, y, first tile, count,
/// palette); the sprites are 8 pixels apart (scenes.md 10.1).
const LETTERING: [(i32, i32, u8, u8, u8); 6] = [
    (105, 137, 0x00, 7, 0),
    (113, 153, 0x07, 5, 1),
    (129, 161, 0x0C, 1, 1),
    (121, 169, 0x0D, 3, 1),
    (105, 177, 0x10, 7, 1),
    (105, 185, 0x17, 7, 1),
];

/// What the screen is about, as gameplay's delivery says (economy.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subject {
    /// The message kind RAM $DB: 1-5 a delivery, 6 the warp-level introduction, 7 the end.
    pub kind: u8,
    /// The printer message id (text.md 3).
    pub text: u8,
    /// The delivered item's kind, which picks the picture.
    pub item: u8,
    /// The analysis cargo number 0-16 (kind 3).
    pub cargo: u8,
}

/// What a pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MessageOutcome {
    Running,
    Ended,
}

/// One step of typed text: a position change or a character.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum Typed {
    Move(usize, usize),
    Char(u8),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Message {
    screen: Screen,
    subject: Subject,
    /// The timer RAM $DA.
    timer: u8,
    typed: Vec<Typed>,
    next: usize,
    cursor: (usize, usize),
    /// The item picture's sprites, drawn over the screen with the playfield's sprite tiles.
    spots: Vec<Spot>,
    /// The analysis screen's value, shown when the timer reaches 64.
    value: u32,
    digits_seen: [bool; 3],
    hud: Hud,
    #[serde(with = "crate::game::state::gauge_rows")]
    gauge_rows: Vec<u8>,
    /// Sprite tiles of the screen's own (the analysis container), instead of the level's.
    own_sprite_tiles: bool,
    /// The gameplay's sprite palette and tiles are the caller's: the palette is ours.
    pass: u32,
    /// A character was typed this pass: the typing click (text.md 2.3).
    click: bool,
    /// The warpship picture's piece that appears when the typing ends, and the passes left
    /// before its lower pair (then the upper pair one pass later).
    piece: Option<usize>,
    piece_wait: Option<u8>,
    /// The effect asked for in this pass.
    sound: Option<u8>,
    /// Quirk Q35: the item pictures' sprites sit one pixel further right in the original.
    pub original_rules: bool,
}

impl Message {
    /// Builds the screen of `subject` over the gameplay's palette (`palette`) with the panel
    /// of `hud`.
    pub fn new(
        graphics: &Graphics,
        level: &Level,
        palette: &Palette,
        subject: Subject,
        hud: Hud,
        clock: &mut Clock,
    ) -> Result<Message, MissingGraphics> {
        let mut screen = Screen::default();
        screen.palette = *palette;
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let mut message = Message {
            screen,
            subject,
            timer: match subject.kind {
                6 => TIMER_WARP,
                7 => TIMER_FINAL,
                _ => TIMER_ITEM,
            },
            typed: Vec::new(),
            next: 0,
            cursor: (0, 0),
            spots: Vec::new(),
            value: 0,
            digits_seen: [false; 3],
            hud,
            gauge_rows: graphics.gauge_rows.clone(),
            own_sprite_tiles: false,
            pass: 0,
            click: false,
            piece: None,
            piece_wait: None,
            sound: None,
            original_rules: false,
        };
        if subject.kind == 6 {
            message.build_warp(graphics)?;
        } else {
            message.build(graphics, level)?;
            // The kinds that start the counter themselves keep it; the others begin at 0.
            if subject.kind != 7 {
                clock.counter = 0;
            }
        }
        message.screen.refresh();
        Ok(message)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The sprite tiles the item picture is drawn with when they are not the level's.
    pub fn sprite_tiles(&self) -> Option<&PatternTable> {
        self.own_sprite_tiles.then(|| self.screen.sprite_tiles())
    }

    /// The item picture's sprites as (x, y, tile, attributes) for the caller to draw with the
    /// gameplay's sprite tiles.
    pub fn item_sprites(&self) -> Vec<(i32, i32, u8, u8)> {
        self.spots
            .iter()
            .map(|s| {
                let attributes = s.palette | u8::from(s.flip_x) << 6 | u8::from(s.flip_y) << 7;
                (
                    s.x - i32::from(!self.original_rules),
                    s.y,
                    s.tile,
                    attributes,
                )
            })
            .collect()
    }

    fn frame_of(subject: Subject) -> Frame {
        match subject.text {
            6..=9 | 21 | 22 | 25 | 47 | 48 | 50 => Frame::Plain,
            45 | 46 => Frame::Decorated,
            27 => Frame::Analysis,
            _ => Frame::Blank,
        }
    }

    fn build(&mut self, graphics: &Graphics, level: &Level) -> Result<(), MissingGraphics> {
        let subject = self.subject;
        let frame = Message::frame_of(subject);
        // The level's tiles stay in video memory under what the screen loads.
        self.screen
            .chr
            .apply(graphics, &gameplay_background_loads(level))?;
        let mut loads = vec![FRAME_AND_FONT];
        loads.extend(FONT_LOADS);
        let picture = match subject.text {
            6 | 7 => Some((0usize, SHIELDS)),
            8 => Some((1, BOOSTERS)),
            9 => Some((2, NAVIGATORS)),
            _ => None,
        };
        if let Some((index, _)) = picture {
            loads.push((ITEM_PICTURES[index], 0x1810));
        }
        if matches!(subject.text, 47 | 48) {
            loads.push(SPACESHIP_TILES);
        }
        loads.extend([STATUS_PANEL, STATUS_PANEL_1D60]);
        if subject.text == 27 {
            loads.push(SHOP_TILES);
        }
        self.screen.load_tiles(graphics, &loads)?;
        if subject.text == 27 {
            // The equipment blocks: block 0, and the one of the cargo at $1930 (graphics.md 5.8).
            let block = |k: usize, address: u16| crate::game::graphics::ChrLoad {
                stream: SHOP_BLOCKS,
                range: Some((SHOP_BLOCK_BYTES * k, SHOP_BLOCK_BYTES * (k + 1))),
                address,
            };
            let cargo = usize::from(subject.cargo) % 17;
            self.screen
                .chr
                .apply(graphics, &[block(0, 0x1810), block(cargo + 1, 0x1930)])?;
            self.screen.refresh();
            self.own_sprite_tiles = true;
        }
        self.load_panel_tiles();
        let preset = |index: usize| {
            graphics
                .presets
                .get(index)
                .ok_or_else(|| MissingGraphics(format!("palette preset {index}")))
        };
        match frame {
            Frame::Plain | Frame::Decorated | Frame::Analysis => {
                let path = match frame {
                    Frame::Plain => PICTURE_B,
                    Frame::Decorated => PICTURE_C,
                    _ => PICTURE_D,
                };
                let writes = graphics
                    .pictures
                    .get(path)
                    .ok_or_else(|| MissingGraphics(path.to_owned()))?;
                self.screen.load_picture(writes);
                let first = match frame {
                    Frame::Plain => PRESET_FRAME_B,
                    Frame::Decorated => PRESET_FRAME_C,
                    _ => PRESET_FRAME_D,
                };
                self.screen.palette.apply(preset(first)?);
                if matches!(subject.text, 47 | 48) {
                    self.screen.palette.apply(preset(PRESET_WARPSHIP)?);
                }
                if subject.text == 27 {
                    // Entries 9 and 10 are the pair of the analysis list, entry 11 a copy of
                    // entry 26, the level's own colour (graphics.md 7.4).
                    let list = usize::from(subject.cargo) % 17;
                    if let Some(pair) = graphics.analysis_pairs.get(list) {
                        self.screen.palette.0[9..11].copy_from_slice(&pair[..2]);
                    }
                    self.screen.palette.0[11] = self.screen.palette.0[26];
                }
            }
            Frame::Blank => {}
        }
        if let Some((_, cells)) = picture {
            let mut tile = 0x81u16;
            for (row, column, count) in cells {
                for i in 0..count {
                    self.screen.set_tile(column + i, row, tile as u8);
                    tile += 1;
                }
            }
        }
        if matches!(subject.text, 47 | 48) {
            let writes = graphics
                .pictures
                .get(SPACESHIP_ATTRIBUTES)
                .ok_or_else(|| MissingGraphics(SPACESHIP_ATTRIBUTES.to_owned()))?;
            self.screen.load_picture(writes);
        }
        if subject.text == 48 {
            for (row, column, tiles) in WARPSHIP {
                for (i, &tile) in tiles.iter().enumerate() {
                    self.screen.set_tile(column + i, row, tile);
                }
            }
        }
        // The text: the immediate lines now, the typed ones as the passes go.
        if let Some(text) = strings::message(subject.text) {
            for line in text.immediate {
                self.print_line(&line);
            }
            for line in text.typed {
                self.typed
                    .push(Typed::Move(usize::from(line.column), usize::from(line.row)));
                self.typed
                    .extend(line.text.bytes().map(|c| Typed::Char(char_tile(c))));
            }
        }
        if subject.text == 27 {
            self.value =
                crate::game::gameplay::economy::CARGO_VALUES[usize::from(subject.cargo) % 17];
            // The three element names, the same list as the cargo's number.
            if let Some(list) = strings::message(28 + subject.cargo % 17) {
                for line in list.immediate {
                    self.print_line(&line);
                }
            }
        }
        self.spots = match subject.item {
            5 => ALIEN.to_vec(),
            12 => RADIOACTIVE.to_vec(),
            15 => crystal(1).to_vec(),
            16 => crystal(0).to_vec(),
            10 => FUEL.to_vec(),
            9 => container_spots(0),
            _ => Vec::new(),
        };
        Ok(())
    }

    /// The panel's tiles on page 1 (the gameplay's panel stays) and its two gauge tiles.
    fn load_panel_tiles(&mut self) {
        write_gauge_tile(&mut self.screen, &self.gauge_rows, 0x1FA0, self.hud.fuel);
        write_gauge_tile(&mut self.screen, &self.gauge_rows, 0x1FB0, self.hud.weapon);
        self.screen.panel = Some(self.hud.panel_page());
    }

    fn print_line(&mut self, line: &Line) {
        for (column, byte) in (usize::from(line.column)..).zip(line.text.bytes()) {
            self.screen
                .set_tile(column, usize::from(line.row), char_tile(byte));
        }
    }

    /// Kind 6: the full-screen picture with the lettering in sprites.
    fn build_warp(&mut self, graphics: &Graphics) -> Result<(), MissingGraphics> {
        let mut chr = Chr::default();
        chr.apply(
            graphics,
            &[COMET_BACKGROUND, COMET_SPRITES]
                .iter()
                .map(|&(stream, address)| crate::game::graphics::ChrLoad {
                    stream,
                    range: None,
                    address,
                })
                .collect::<Vec<_>>(),
        )?;
        self.screen.chr = chr;
        self.screen.sprite_table = 0;
        let writes = graphics
            .pictures
            .get(PICTURE_COMET)
            .ok_or_else(|| MissingGraphics(PICTURE_COMET.to_owned()))?;
        self.screen.load_picture(writes);
        self.screen.palette.apply(
            graphics
                .presets
                .get(PRESET_COMET)
                .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET_COMET}")))?,
        );
        let mut sprites = Vec::new();
        for (x, y, first, count, palette) in LETTERING {
            for i in 0..count {
                sprites.push(Sprite::new(x + 8 * i32::from(i), y, first + i, palette));
            }
        }
        self.screen.sprites = sprites;
        Ok(())
    }

    /// One pass. `clock` holds $5D: kinds 1-5 and 7 step it with the generator, kind 6 only
    /// adds one (game-flow.md 7.4).
    pub fn step(&mut self, clock: &mut Clock, held: Buttons, pressed: Buttons) -> MessageOutcome {
        self.pass += 1;
        if self.subject.kind == 6 {
            clock.counter = (clock.counter + 1) & 0xFF_FFFF;
            // The picture shakes: the vertical scroll alternates between 0 and 2 every frame.
            self.screen.scroll.1 = if clock.counter & 1 == 0 { 2 } else { 0 };
        } else {
            clock.advance_split(held, pressed);
        }
        let counter = clock.counter;
        if self.subject.kind != 6 {
            self.piece_step();
            self.typewriter(counter);
            if self.subject.text == 27 {
                self.analysis(clock, counter);
                let stage = counter
                    .checked_sub(CONTAINER_FIRST_OPEN)
                    .map_or(0, |n| (n / CONTAINER_STAGE_PASSES + 1) as usize);
                self.spots = container_spots(stage);
            }
        }
        // $DA counts down on the frames whose $5D (low byte) is 64 or more; the button is read
        // then too.
        // The final message always lasts 319 passes (docs/deviations.md, Q33): 64 idle, then 255.
        // The warp-level introduction lasts 256 passes (64 idle, then 192) unless the original's
        // rules are asked for, which make its length depend on $5D (docs/deviations.md).
        let counting = if self.subject.kind == 7 || (self.subject.kind == 6 && !self.original_rules)
        {
            self.pass > 64
        } else {
            counter & 0xFF >= 64
        };
        if counting {
            self.timer = self.timer.saturating_sub(1);
            if self.timer == 0 || (self.timer < 64 && pressed.0 != 0) {
                return MessageOutcome::Ended;
            }
            if self.subject.text == 27 {
                if self.timer == 128 {
                    self.show_cargo_name();
                }
                if self.timer == 64 {
                    self.show_value();
                }
            }
        }
        MessageOutcome::Running
    }

    /// Shows the warpship picture of a delivered piece `piece` (0-11) with the pieces of `flags`
    /// (bit p = piece p) already in it; the delivered one appears when the typing ends.
    pub fn set_warpship(&mut self, flags: u16, piece: usize) {
        if !matches!(self.subject.text, 47 | 48) || piece >= PIECES.len() {
            return;
        }
        for (p, &(row, column, tiles)) in PIECES.iter().enumerate() {
            let shown = flags & (1 << p) != 0 && p != piece;
            for (i, &tile) in tiles.iter().enumerate() {
                let tile = if shown { tile } else { 0x80 };
                self.screen.set_tile(column + i % 2, row + i / 2, tile);
            }
        }
        self.piece = Some(piece);
    }

    /// The effect this pass asked for on both pulses, once.
    pub fn take_sound(&mut self) -> Option<u8> {
        self.sound.take()
    }

    /// The piece's pairs of cells: the lower pair, then the upper one a pass later.
    fn piece_step(&mut self) {
        let Some(wait) = self.piece_wait else { return };
        let Some(piece) = self.piece else { return };
        let (row, column, tiles) = PIECES[piece];
        match wait {
            0 => {
                for (i, &tile) in tiles.iter().enumerate().skip(2) {
                    self.screen.set_tile(column + i % 2, row + i / 2, tile);
                }
                self.piece_wait = Some(u8::MAX);
            }
            u8::MAX => {
                for (i, &tile) in tiles.iter().enumerate().take(2) {
                    self.screen.set_tile(column + i % 2, row + i / 2, tile);
                }
                self.piece_wait = None;
            }
            n => {
                // The effect is requested a pass before the lower pair shows (Mesen,
                // `sj-tools scene --compare`).
                if n == 1 {
                    self.sound = Some(PIECE_SOUND);
                }
                self.piece_wait = Some(n - 1);
            }
        }
    }

    /// Whether this pass typed a character that clicks, once.
    pub fn take_click(&mut self) -> bool {
        std::mem::take(&mut self.click)
    }

    fn typewriter(&mut self, counter: u32) {
        // The printer clicks when it prints a character, a pass before the character shows
        // (Mesen, `sj-tools scene --compare`): every character but a blank clicks (text.md 2.3).
        if (counter + 1).is_multiple_of(TYPE_EVERY)
            && let Some(&Typed::Char(tile)) = self.typed.get(self.next)
        {
            self.click = tile != 0;
        }
        if !counter.is_multiple_of(TYPE_EVERY) {
            return;
        }
        let Some(&item) = self.typed.get(self.next) else {
            return;
        };
        self.next += 1;
        if self.next == self.typed.len() && matches!(self.subject.text, 47 | 48) {
            // The warpship messages change four cells at the right of the box when the text
            // is complete (measured; unexplained in the spec).
            for (column, row, tile) in [(30, 8, 0x40), (31, 8, 0x20), (30, 15, 0x81), (31, 15, 0)] {
                self.screen.set_tile(column, row, tile);
            }
            if self.piece.is_some() {
                self.piece_wait = Some(PIECE_DELAY - 1);
            }
        }
        match item {
            Typed::Move(column, row) => self.cursor = (column, row),
            Typed::Char(tile) => {
                self.screen.set_tile(self.cursor.0, self.cursor.1, tile);
                self.cursor.0 += 1;
            }
        }
    }

    /// The three flickering digits of each element's amount (text.md 4.4).
    fn analysis(&mut self, clock: &Clock, counter: u32) {
        let window = match counter {
            0..=31 => Some(0),
            32..=63 => Some(1),
            64..=95 => Some(2),
            _ => None,
        };
        let Some(row) = window else { return };
        let bytes = [clock.rng.out, clock.rng.high, clock.rng.low];
        for (i, byte) in bytes.iter().enumerate() {
            let digit = ANALYSIS_DIGITS[usize::from(byte & 15)];
            self.screen.set_tile(24 + i, 10 + 2 * row, digit);
        }
        self.digits_seen[row] = true;
    }

    fn show_cargo_name(&mut self) {
        let name = CARGO[usize::from(self.subject.cargo) % 17];
        // Right-aligned: the last character is in column 26 of row 16.
        let start = 27 - name.len();
        for (i, byte) in name.bytes().enumerate() {
            self.screen.set_tile(start + i, 16, char_tile(byte));
        }
    }

    fn show_value(&mut self) {
        let digits = self.value.to_string();
        for (i, byte) in digits.bytes().enumerate() {
            self.screen.set_tile(16 + i, 18, byte);
        }
    }

    /// The cells of the Hud panel are the caller's: the message keeps showing the state from
    /// before the delivery.
    pub fn hud(&self) -> &Hud {
        &self.hud
    }
}

/// The partly filled gauge tile at `address` for a gauge showing `level`.
fn write_gauge_tile(screen: &mut Screen, gauge_rows: &[u8], address: u16, level: u8) {
    let fill = usize::from(level % 16 / 2) & 7;
    screen.chr.write_gauge(address, gauge_rows, fill);
}

/// Shows the gameplay's status panel on `screen`: the panel's tiles, its two partly filled
/// gauge cells and the page (the screens that keep the panel, as the messages do).
pub(super) fn load_panel(
    screen: &mut Screen,
    graphics: &Graphics,
    hud: Hud,
) -> Result<(), MissingGraphics> {
    screen.load_tiles(graphics, &[STATUS_PANEL, STATUS_PANEL_1D60])?;
    write_gauge_tile(screen, &graphics.gauge_rows, 0x1FA0, hud.fuel);
    write_gauge_tile(screen, &graphics.gauge_rows, 0x1FB0, hud.weapon);
    screen.panel = Some(hud.panel_page());
    Ok(())
}

/// Tile of one byte of message text (the game's character codes, text.md 1).
fn char_tile(byte: u8) -> u8 {
    super::ppu::char_tile(char::from(byte))
}
