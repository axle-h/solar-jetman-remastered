//! The planet surface (`docs/spec/scenes.md` 8, `docs/spec/game-flow.md` 7.3): the mothership
//! descends onto the planet's picture, the planet menu (EXPLORE, EQUIP POD, BLAST OFF) over it,
//! and the blast-off that wipes the menu and sends the mothership up and away. One call of
//! [`Surface::step`] is one pass of the original's handler.

use super::ppu::{Screen, Sprite};
use super::strings::{MENU_LAYOUTS, MENU_OPTIONS, MENU_RECORDS};
use super::{Clock, FONT_LOADS};
use crate::game::SCREEN_HEIGHT;
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::sprites::piece_left;
use crate::game::gameplay::sprites::{Piece, SpriteFile};
use crate::game::graphics::{
    ChrLoad, Graphics, MissingGraphics, TILE_BYTES, gameplay_background_loads, gameplay_palette,
};
use crate::game::level::Level;

/// The picture of each planet (scenes.md 8.1).
const PICTURE_OF_PLANET: [u8; 13] = [4, 6, 5, 0, 1, 2, 4, 1, 2, 5, 2, 0, 0];
/// Per picture 0-6 (3 is empty): resting x and y, start y and the ship's layout.
const REST_X: [u8; 7] = [83, 87, 153, 0, 23, 162, 51];
const REST_Y: [u8; 7] = [121, 156, 117, 0, 96, 88, 125];
const START_Y: [u8; 7] = [57, 0, 61, 0, 48, 0, 0];
const SHIP_LAYOUT: [u8; 7] = [0x80, 0x9A, 0x85, 0, 0x85, 0x85, 0xAB];
/// The picture's own tile streams (graphics.md 4.4): table 1, table 0 and the extra tiles.
const TABLE_1: [&str; 7] = [
    "tiles/surface/picture-0-table-1.png",
    "tiles/surface/picture-1-table-1.png",
    "tiles/surface/picture-2-table-1.png",
    "",
    "tiles/surface/picture-4-table-1.png",
    "tiles/surface/picture-5-table-1.png",
    "tiles/surface/picture-6-table-1.png",
];
const TABLE_0: [&str; 7] = [
    "tiles/surface/picture-0-table-0.png",
    "tiles/surface/picture-1-table-0.png",
    "tiles/surface/picture-2-table-0.png",
    "",
    "tiles/surface/picture-4-table-0.png",
    "tiles/surface/picture-5-table-0.png",
    "tiles/surface/picture-6-table-0.png",
];
const EXTRA: [&str; 7] = [
    "tiles/surface/extra-shared.png",
    "tiles/surface/extra-picture-1.png",
    "tiles/surface/extra-shared.png",
    "",
    "tiles/surface/extra-shared.png",
    "tiles/surface/extra-shared.png",
    "tiles/surface/extra-picture-6.png",
];
const COMMON: &str = "tiles/surface/common.png";
/// The first preset of the surface pictures; picture `p` uses preset `12 + p`.
const PRESET_FIRST: usize = 12;
/// The line at which the background switches to pattern table 0 (graphics.md 1.1).
const SPLIT_LINE: usize = 126;

const STATUS_PANEL: (&str, u16) = ("tiles/hud/status-panel.png", 0x1E40);
const STATUS_PANEL_1D60: (&str, u16) = ("tiles/hud/status-panel-1d60.png", 0x1D60);
const POD_FRAME: u8 = 0x9F;
const CURSOR_TILE: u8 = 0xA7;
const CURSOR_X: [i32; 2] = [90, 154];
const CURSOR_Y: [i32; 3] = [49, 65, 89];
/// The menu box's top-left cell and its twelve columns.
const BOX_COLUMN: usize = 10;
const BOX_ROW: usize = 4;
/// Passes the automatic departure waits before the mothership moves.
const AUTOMATIC_IDLE_PASSES: u32 = 1;
/// The background palette of the box's cells.
const BOX_PALETTE: u8 = 3;
/// Tile bases of the menu text: letters at code + $90, frame pieces at code + $BF.
const LETTER_BASE: u8 = 0x90;
const FRAME_BASE: u8 = 0xBF;

/// Picture 1's six fixed sprites: (x, y, tile), sprite palette 1, behind the background.
const FIXED_SPRITES: [(i32, i32, u8); 6] = [
    (84, 136, 0xA8),
    (92, 136, 0xA9),
    (84, 144, 0xAA),
    (92, 152, 0xAB),
    (84, 160, 0xAC),
    (92, 160, 0xAC),
];

/// Where the scene was entered and what it is doing (`game-flow.md` 7.3, RAM $A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    /// 0: the mothership flies in.
    Descent,
    /// 2 and 3: the planet menu.
    Menu,
    /// 4: the picture is redrawn row by row over the menu.
    Wipe,
    /// 1: the mothership flies off the top of the screen.
    Leaving,
}

/// What the player chose or what ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SurfaceOutcome {
    Running,
    /// To the launch scene and gameplay (EXPLORE, or the end of a descent with nothing to
    /// choose from).
    Launch,
    /// EQUIP POD.
    EquipPod,
    /// The mothership has gone: the next planet.
    Left,
}

/// What a menu option does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Option_ {
    Explore,
    Equip,
    Blast,
}

/// The mothership flying in or out: position and speed (16.8 and 8.8), and the exhaust.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Ship {
    x: i32,
    y: i32,
    x_speed: i32,
    y_speed: i32,
    visible: bool,
    exhaust: Option<u8>,
    /// Where the exhaust is drawn: the mothership's position before this pass's move.
    exhaust_at: (i32, i32),
}

/// The small object that rises from the mothership when it has landed with nothing aboard.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Pod {
    x: i32,
    y: i32,
    speed: i32,
    counter: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Surface {
    screen: Screen,
    mode: Mode,
    picture: usize,
    /// The picture's own nametable rows, to redraw over the menu box.
    page: Vec<Vec<u8>>,
    /// Passes in the current mode, from 1.
    pass: u32,
    ship: Ship,
    pod: Option<Pod>,
    /// Nothing owned or fitted: the descent includes the rising object and then goes straight
    /// to the launch.
    nothing_owned: bool,
    layout: usize,
    cursor: usize,
    /// Rows of the menu box drawn so far (it builds up one row per pass).
    rows_drawn: usize,
    /// The picture's attribute bytes, to restore under the menu box.
    picture_attributes: Vec<u8>,
    /// The mode-4 counter: the nametable row redrawn next.
    wipe_row: usize,
    /// Passes left before a skipped descent ends.
    skip: Option<u32>,
    /// A song to start, taken by the caller.
    song: Option<u8>,
    /// The mothership leaves at once on entering the menu mode (game-flow.md 7.3): the first
    /// passes only prepare it.
    automatic: bool,
    /// The validation mode (Q35, Q36 of `docs/deviations.md`): the drawer's stray pixel and the
    /// exhaust's lag stay.
    pub original_rules: bool,
}

impl Surface {
    /// The surface of planet `level` (0-12) with the picture's tiles loaded, over `previous`
    /// CHR memory and palette. `layout` is the menu's layout, built only in the menu mode.
    fn build(
        graphics: &Graphics,
        world_level: &Level,
        level: usize,
        previous: Option<&Screen>,
        nothing_owned: bool,
    ) -> Result<Surface, MissingGraphics> {
        let planet = level.min(12);
        let picture = usize::from(PICTURE_OF_PLANET[planet]);
        let mut screen = Screen::default();
        if let Some(previous) = previous {
            screen.chr = previous.chr.clone();
        }
        // The loads in the original's order (graphics.md 5.5): the level's gameplay background,
        // the font and the status panel again, then the picture's own tiles.
        let mut loads = gameplay_background_loads(world_level);
        let font = |stream: &'static str, address: u16, bytes: Option<usize>| ChrLoad {
            stream,
            range: bytes.map(|n| (0, n)),
            address,
        };
        loads.extend([
            // The letters A-Z, the cross and the hyphen (28 glyphs) and the font's extras.
            font(FONT_LOADS[1].0, 0x1020, Some(28 * TILE_BYTES)),
            font(FONT_LOADS[2].0, 0x11E0, None),
            font(STATUS_PANEL.0, STATUS_PANEL.1, None),
            font(STATUS_PANEL_1D60.0, STATUS_PANEL_1D60.1, None),
        ]);
        for (stream, address) in FONT_LOADS {
            loads.push(font(stream, address, None));
        }
        for (&(stream, _), address) in FONT_LOADS.iter().zip([0x1C00, 0x1D10, 0x1F00]) {
            loads.push(font(stream, address, None));
        }
        loads.push(font(FONT_LOADS[2].0, 0x1100, None));
        for (stream, address) in [
            (COMMON, 0x1A30),
            (TABLE_1[picture], 0x1000),
            (TABLE_0[picture], 0x0000),
            (EXTRA[picture], 0x1A80),
        ] {
            loads.push(font(stream, address, None));
        }
        screen.chr.apply(graphics, &loads)?;
        let path = format!("pictures/surface-{picture}.ron");
        let writes = graphics
            .pictures
            .get(&path)
            .ok_or_else(|| MissingGraphics(path.clone()))?;
        screen.load_picture(writes);
        let mut page = vec![vec![0u8; 32]; 30];
        let mut picture_attributes = vec![0u8; 64];
        for write in writes {
            if write.address >= 0x23C0 {
                let start = usize::from(write.address) - 0x23C0;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    if let Some(cell) = picture_attributes.get_mut(start + i) {
                        *cell = byte;
                    }
                }
                continue;
            }
            let start = usize::from(write.address).saturating_sub(0x2000);
            for (i, &byte) in write.bytes.iter().enumerate() {
                if start + i < 30 * 32 {
                    page[(start + i) / 32][(start + i) % 32] = byte;
                }
            }
        }
        let preset = graphics
            .presets
            .get(PRESET_FIRST + picture)
            .ok_or_else(|| MissingGraphics(format!("palette preset {}", PRESET_FIRST + picture)))?;
        // The gameplay palette of the level stays where the preset does not reach (the sprite
        // palettes 2 and 3 of picture 6), then the preset and the planet's own 12 colours.
        screen.palette = gameplay_palette(graphics, planet)?;
        screen.palette.apply(preset);
        if let Some(Some(colours)) = graphics.surface_colours.get(planet) {
            for (i, &colour) in colours.iter().enumerate() {
                screen.palette.0[i] = colour;
            }
        }
        screen.background_table = 1;
        screen.sprite_table = 1;
        screen.split = Some((SPLIT_LINE, 0));
        screen.scroll = (0, 2);
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen.refresh();
        let (tx, ty) = (i32::from(REST_X[picture]), i32::from(REST_Y[picture]));
        Ok(Surface {
            screen,
            mode: Mode::Descent,
            picture,
            page,
            picture_attributes,
            pass: 0,
            ship: Ship {
                x: tx << 8,
                y: ty << 8,
                x_speed: 0,
                y_speed: 0,
                visible: true,
                exhaust: None,
                exhaust_at: (0, 0),
            },
            pod: None,
            nothing_owned,
            layout: 0,
            cursor: 1,
            rows_drawn: 0,
            wipe_row: 0,
            skip: None,
            song: None,
            automatic: false,
            original_rules: false,
        })
    }

    /// The first arrival (mode 0): the mothership flies in from the right. `nothing_owned`:
    /// nothing owned or fitted (`game-flow.md` 7.3).
    pub fn landing(
        graphics: &Graphics,
        world_level: &Level,
        level: usize,
        previous: Option<&Screen>,
        nothing_owned: bool,
    ) -> Result<Surface, MissingGraphics> {
        let mut surface = Surface::build(graphics, world_level, level, previous, nothing_owned)?;
        let picture = surface.picture;
        surface.ship = Ship {
            x: 255 << 8,
            y: i32::from(START_Y[picture]) << 8,
            x_speed: 0xC0,
            y_speed: if picture == 1 { 0x80 } else { 0 },
            visible: true,
            exhaust: None,
            exhaust_at: (0, 0),
        };
        surface.song = Some(3);
        Ok(surface)
    }

    /// After docking (mode 3): the menu over the picture, or at once the mothership's departure
    /// when the planet's flag is set and nothing is left to collect (`leave_at_once`).
    pub fn docked(
        graphics: &Graphics,
        world_level: &Level,
        level: usize,
        previous: Option<&Screen>,
        nothing_owned: bool,
        flag_set: bool,
        leave_at_once: bool,
    ) -> Result<Surface, MissingGraphics> {
        let mut surface = Surface::build(graphics, world_level, level, previous, nothing_owned)?;
        surface.layout = usize::from(flag_set) | usize::from(nothing_owned) << 1;
        surface.cursor = 1;
        if flag_set && leave_at_once {
            surface.start_leaving();
            surface.automatic = true;
        } else {
            surface.mode = Mode::Menu;
        }
        Ok(surface)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Draws the first picture, before the first pass.
    pub fn show(&mut self, layouts: &SpriteFile) {
        self.place_sprites(layouts);
    }

    /// The song a mode starts, once (scenes.md 15): 3 with the descent, 5 with the departure.
    pub fn take_song(&mut self) -> Option<super::SoundRequest> {
        self.song.take().map(super::SoundRequest::SongIfNew)
    }

    fn tx(&self) -> i32 {
        i32::from(REST_X[self.picture])
    }

    fn ty(&self) -> i32 {
        i32::from(REST_Y[self.picture])
    }

    fn start_leaving(&mut self) {
        self.mode = Mode::Leaving;
        self.song = Some(5);
        self.pass = 0;
        self.ship = Ship {
            x: self.tx() << 8,
            y: self.ty() << 8,
            x_speed: 0,
            y_speed: if self.picture == 1 { 0x80 } else { 0 },
            visible: true,
            exhaust: None,
            exhaust_at: (0, 0),
        };
    }

    /// The options of the layout, in order.
    fn options(&self) -> Vec<Option_> {
        match self.layout {
            0 => vec![Option_::Explore, Option_::Equip],
            1 => vec![Option_::Explore, Option_::Equip, Option_::Blast],
            2 => vec![Option_::Explore],
            _ => vec![Option_::Explore, Option_::Blast],
        }
    }

    /// Draws row `r` of the menu box in the layout.
    fn draw_box_row(&mut self, r: usize) {
        let Some(&record) = MENU_LAYOUTS[self.layout].get(r) else {
            return;
        };
        let text = MENU_RECORDS[usize::from(record)];
        for column in 0..12 {
            let tile = match text.as_bytes().get(column) {
                None | Some(b' ') => 0,
                Some(&c @ b'1'..=b'6') => c + FRAME_BASE,
                Some(&c) => c + LETTER_BASE,
            };
            self.screen.set_tile(BOX_COLUMN + column, BOX_ROW + r, tile);
        }
        if !self.original_rules {
            // The remake colours the box's own cells alone: the original's 2 × 2 quadrants
            // recolour the picture next to a half-drawn box (`docs/deviations.md`).
            for column in 0..12 {
                self.screen
                    .set_own_palette(BOX_COLUMN + column, BOX_ROW + r, Some(BOX_PALETTE));
            }
            return;
        }
        // The attribute bytes of the group of rows are rewritten with the row: the box's rows
        // take background palette 3, except the blank row under it.
        let first = (BOX_ROW + r) / 4 * 4;
        for row in first..first + 4 {
            if row >= BOX_ROW && row - BOX_ROW + 1 < MENU_LAYOUTS[self.layout].len() {
                for pair in 0..6 {
                    self.screen
                        .set_cell_palette(BOX_COLUMN + 2 * pair, row, BOX_PALETTE);
                }
            }
        }
    }

    /// One pass.
    pub fn step(
        &mut self,
        layouts: &SpriteFile,
        clock: &mut Clock,
        pressed: Buttons,
    ) -> SurfaceOutcome {
        self.pass += 1;
        // The counter after this pass's increment, $5D as the handler reads it.
        clock.counter = (clock.counter + 1) & 0xFF_FFFF;
        let counter = clock.counter;
        let outcome = match self.mode {
            Mode::Descent => self.descent(counter, pressed),
            Mode::Menu => self.menu(pressed),
            Mode::Wipe => self.wipe(),
            Mode::Leaving if self.automatic && self.pass <= AUTOMATIC_IDLE_PASSES => {
                SurfaceOutcome::Running
            }
            Mode::Leaving => self.leaving(counter),
        };
        self.place_sprites(layouts);
        outcome
    }

    fn descent(&mut self, counter: u32, pressed: Buttons) -> SurfaceOutcome {
        let (tx, ty) = (self.tx(), self.ty());
        // A newly pressed A ends the descent at once when the launch follows; when the menu
        // follows it takes three more passes, the mothership moving on until then (measured).
        if pressed.has(Buttons::A) && self.skip.is_none() {
            self.skip = Some(if self.nothing_owned { 1 } else { 3 });
        }
        if let Some(left) = self.skip {
            if left == 1 {
                self.ship.x = tx << 8;
                self.ship.y = ty << 8;
                self.ship.exhaust = None;
                self.pod = None;
                return self.end_of_descent();
            }
            self.skip = Some(left - 1);
        }
        let ship = &mut self.ship;
        let before = (ship.x >> 8, ship.y >> 8);
        if let Some(pod) = &mut self.pod {
            // The object rises while its counter counts up; at 128 the descent ends.
            pod.counter += 1;
            pod.y -= pod.speed;
            if pod.counter >= 128 {
                return self.end_of_descent();
            }
            return SurfaceOutcome::Running;
        }
        let moves_down = |ship: &mut Ship| {
            ship.y += ship.y_speed;
            if ship.y >> 8 < 16 {
                // The line-16 rule keeps the fraction (visible with picture 1's start speed).
                ship.y = 16 << 8 | (ship.y & 0xFF);
                ship.y_speed = 0;
            }
        };
        if ship.x >> 8 != tx {
            let d = ((ship.x >> 8) - tx) & 0xFF;
            ship.x -= ship.x_speed;
            if d < 37 {
                let step = if counter & 15 == 0 { 3 } else { 2 };
                ship.x_speed = (ship.x_speed - step).max(0);
                if ship.x_speed == 0 {
                    ship.x = tx << 8 | (ship.x & 0xFF);
                }
                ship.y_speed = (ship.y_speed + 1).min(0x300);
                moves_down(ship);
            }
            ship.exhaust = None;
        } else if ship.y >> 8 == ty {
            ship.exhaust = None;
            if self.nothing_owned {
                let picture = self.picture;
                self.pod = Some(Pod {
                    x: (ship.x >> 8) + if picture == 6 { 4 } else { 7 },
                    y: (ship.y >> 8) << 8,
                    speed: if picture == 0 { 0x40 } else { 0x20 },
                    // Measured against the original frame by frame (planet 0): the scene ends 126
                    // passes after this one, so the object's counter, which ends it at 128,
                    // starts at 2 here. The spec's "arrival + 128" counts the arrival two passes
                    // earlier than the pass the ship is drawn stopped in.
                    counter: 2,
                });
                if picture == 1 {
                    ship.visible = false;
                }
            } else {
                return self.end_of_descent();
            }
        } else {
            let above = ty - (ship.y >> 8);
            if (1..=14).contains(&above) {
                ship.y_speed = (ship.y_speed - 1).max(0);
            }
            moves_down(ship);
            let base = i32::from(SHIP_LAYOUT[self.picture]);
            let b = ((counter >> 2) & 1) as i32;
            let above = ty - (ship.y >> 8);
            // Quirk Q36: the original places the flame before the move, one pass behind.
            ship.exhaust_at = if self.original_rules {
                before
            } else {
                (ship.x >> 8, ship.y >> 8)
            };
            ship.exhaust = Some(match (above > 14, base) {
                (true, _) => (base + 1 + b) as u8,
                (false, 0x9A) => (base + b) as u8,
                (false, 0xAB) => {
                    if b == 1 {
                        0xAE
                    } else {
                        0xFF
                    }
                }
                (false, _) => (base + 3 + b) as u8,
            });
            // With something owned the descent ends as the mothership stops: the pass that
            // moves it onto its place is the last (the menu starts in it).
            if !self.nothing_owned && ship.y >> 8 == ty {
                return self.end_of_descent();
            }
        }
        SurfaceOutcome::Running
    }

    /// The end of the descent: the launch when nothing is owned, else the menu (mode 2).
    fn end_of_descent(&mut self) -> SurfaceOutcome {
        self.ship.exhaust = None;
        self.ship.x = self.tx() << 8;
        self.ship.y = self.ty() << 8;
        if self.nothing_owned {
            self.layout = 2;
            self.cursor = 1;
            return SurfaceOutcome::Launch;
        }
        self.mode = Mode::Menu;
        self.layout = 0;
        self.cursor = 2;
        self.pass = 0;
        self.pod = None;
        // The menu's first row goes in the pass that ended the descent.
        self.menu(Buttons::default())
    }

    fn menu(&mut self, pressed: Buttons) -> SurfaceOutcome {
        let options = self.options();
        self.rows_drawn = (self.rows_drawn + 1).min(MENU_LAYOUTS[self.layout].len());
        for r in 0..self.rows_drawn {
            self.draw_box_row(r);
        }
        let count = usize::from(MENU_OPTIONS[self.layout]);
        if pressed.has(Buttons::UP) && self.cursor > 1 {
            self.cursor -= 1;
        }
        if pressed.has(Buttons::DOWN) && self.cursor < count {
            self.cursor += 1;
        }
        if pressed.has(Buttons::A) {
            match options[self.cursor - 1] {
                Option_::Explore => return SurfaceOutcome::Launch,
                Option_::Equip => return SurfaceOutcome::EquipPod,
                Option_::Blast => {
                    self.mode = Mode::Wipe;
                    self.pass = 0;
                    self.wipe_row = 4;
                    self.ship.y_speed = if self.picture == 1 { 0x80 } else { 0 };
                }
            }
        }
        SurfaceOutcome::Running
    }

    fn wipe(&mut self) -> SurfaceOutcome {
        // Rows 4 to 18 go back to the picture's own, one per pass but two in the first.
        let rows = 1;
        for _ in 0..rows {
            let row = self.wipe_row;
            if row > 18 {
                break;
            }
            for column in 0..32 {
                self.screen.set_tile(column, row, self.page[row][column]);
                self.screen.set_own_palette(column, row, None);
            }
            // The attribute bytes of the group of rows come back with the first of its rows.
            if row.is_multiple_of(4) {
                let group = row / 4;
                for i in 0..8 {
                    self.screen
                        .set_attribute(group * 8 + i, self.picture_attributes[group * 8 + i]);
                }
            }
            self.wipe_row += 1;
        }
        if self.pass >= 16 {
            self.start_leaving();
        }
        SurfaceOutcome::Running
    }

    fn leaving(&mut self, counter: u32) -> SurfaceOutcome {
        // This mode reads the counter as it was before the pass's increment.
        let counter = counter.wrapping_sub(1);
        let ship = &mut self.ship;
        if counter & 7 == 0 {
            ship.y_speed = (ship.y_speed + 1).min(0x300);
        }
        ship.y -= ship.y_speed;
        ship.exhaust_at = (ship.x >> 8, ship.y >> 8);
        let base = i32::from(SHIP_LAYOUT[self.picture]);
        ship.exhaust = Some((base + 1 + ((counter >> 1) & 1) as i32) as u8);
        if ship.y < 0 {
            return SurfaceOutcome::Left;
        }
        SurfaceOutcome::Running
    }

    /// The sprites of the scene's objects (the cursor, the picture's fixed sprites, the
    /// mothership, its exhaust and the rising object).
    fn place_sprites(&mut self, layouts: &SpriteFile) {
        let mut sprites = Vec::new();
        if self.mode == Mode::Menu || (self.mode == Mode::Wipe && self.pass == 0) {
            let y = CURSOR_Y[self.cursor.clamp(1, 3) - 1] + 1;
            for x in CURSOR_X {
                sprites.push(Sprite::new(x, y, CURSOR_TILE, 0));
            }
        }
        if self.picture == 1 && self.mode != Mode::Wipe {
            for (x, y, tile) in FIXED_SPRITES {
                let mut sprite = Sprite::new(x, y + 1, tile, 1);
                sprite.behind = true;
                sprites.push(sprite);
            }
        }
        let (x, y) = (self.ship.x >> 8, self.ship.y >> 8);
        let (tx, _) = (self.tx(), self.ty());
        let behind = match self.picture {
            0 => x != tx,
            4 => x == tx || x >= 216,
            _ => false,
        };
        let original = self.original_rules;
        let mut object = |frame: u8, at: (i32, i32), mirrored: bool, flipped: bool| {
            let Some(pieces) = layouts.table_b.get(usize::from(frame)) else {
                return;
            };
            for piece in pieces {
                sprites.push(piece_sprite(
                    piece,
                    at,
                    behind,
                    tile_base(frame),
                    mirrored,
                    flipped,
                    original,
                ));
            }
        };
        // The mothership and its exhaust face left; the small object does not.
        if self.ship.visible {
            object(SHIP_LAYOUT[self.picture], (x, y), true, false);
        }
        if let Some(frame) = self.ship.exhaust {
            object(frame, self.ship.exhaust_at, true, false);
        }
        if let Some(pod) = &self.pod {
            // The rising object moves up, so it is drawn flipped vertically (objects.md 6.3).
            object(POD_FRAME, (pod.x, pod.y >> 8), false, true);
        }
        self.screen.sprites = sprites;
        // The remake's: the mothership is hidden by the whole cliff it sinks behind, and the
        // two lines below the picture (the scroll of 2 shows its row 0 again there, which a
        // television's overscan hides) are blank (`docs/deviations.md`).
        self.screen.solid_behind = !original;
        self.screen.hide_from = (!original).then_some(SCREEN_HEIGHT - 2);
    }
}

/// Frames $80-$9E (the mothership and its exhaust on five of the pictures) take their tiles from
/// $A8 on; the others are absolute (objects.md 5.2).
fn tile_base(frame: u8) -> u8 {
    if (0x80..0x9F).contains(&frame) {
        0xA8
    } else {
        0
    }
}

/// A layout piece placed with its object point at `at`, mirrored when the object faces left
/// (objects.md 6.3); the console shows a sprite one line below its Y.
fn piece_sprite(
    piece: &Piece,
    at: (i32, i32),
    behind: bool,
    base: u8,
    mirrored: bool,
    flipped: bool,
    original: bool,
) -> Sprite {
    let dx = if mirrored {
        -i32::from(piece.dx)
    } else {
        i32::from(piece.dx)
    };
    let attributes = piece.attributes ^ if mirrored { 0x40 } else { 0 };
    // Quirk Q35: the original's drawer puts a layout without a vertical flip (all of this
    // scene's) one pixel right of X + dx; the remake keeps the mirror image's position.
    let mut sprite = Sprite::new(
        piece_left(at.0, dx, flipped, original),
        at.1 + i32::from(piece.dy) + 1,
        piece.tile.wrapping_add(base),
        attributes & 3,
    );
    sprite.behind = behind || attributes & 0x20 != 0;
    sprite.flip_x = attributes & 0x40 != 0;
    sprite.flip_y = attributes & 0x80 != 0;
    sprite
}
