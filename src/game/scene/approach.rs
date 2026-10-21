//! The approach screen (`docs/spec/scenes.md` 7): "APPROACHING PLANET" over a star field with
//! the planet's globe, its gravity and diameter, the password for the state, and the status panel
//! with the system, the lives and the score. Any button lands.

use super::ppu::{Screen, Sprite};
use super::strings::{PLANETS, SYSTEMS, signed_number};
use super::{Clock, FONT_LOADS};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{Graphics, MissingGraphics};
use crate::game::hud::Hud;

const PICTURE: &str = "pictures/approach.ron";
const PRESET: usize = 0;
const BACKGROUND: (&str, u16) = ("tiles/approach/background.png", 0x1800);
const SPRITES: (&str, u16) = ("tiles/approach/sprites.png", 0x0010);
const STATUS_PANEL: (&str, u16) = ("tiles/hud/status-panel.png", 0x1E40);
const STATUS_PANEL_1D60: (&str, u16) = ("tiles/hud/status-panel-1d60.png", 0x1D60);
/// The globe: 15 background tiles from $81 in rows 8-11, columns 14-17 (the corner stays blank),
/// in background palette 2.
const GLOBE_FIRST_TILE: u8 = 0x81;
/// Attribute bytes of the globe's cells (rows 8-11 are attribute row 2; columns 14-17 straddle
/// attribute columns 3 and 4).
const GLOBE_ATTRIBUTES: [(usize, u8); 2] = [(19, 0x88), (20, 0x22)];
/// The nine sprites over the globe: (x, y, tile), palette 2.
const GLOBE_SPRITES: [(i32, i32, u8); 9] = [
    (120, 65, 1),
    (128, 65, 2),
    (136, 65, 3),
    (112, 73, 4),
    (128, 73, 5),
    (136, 73, 6),
    (112, 81, 7),
    (128, 81, 8),
    (136, 81, 9),
];
/// The status panel's attribute bytes in attribute row 1 (rows 4-7), by column group.
const PANEL_ATTRIBUTES: [u8; 8] = [0x00, 0x00, 0x0C, 0xFF, 0x05, 0x55, 0x55, 0x00];

/// What the screen shows besides the planet.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    /// Level index, 0-13.
    pub level: usize,
    /// Gravity (a signed multiple of 8) and diameter of the planet's level.
    pub gravity: i32,
    pub diameter: i32,
    /// The password for the current state.
    pub password: &'a [u8; 12],
    pub lives: u8,
    pub jetpods: u8,
    pub score: u32,
}

/// What a pass of the screen did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApproachOutcome {
    Running,
    /// A button was pressed: land.
    Land,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Approach {
    screen: Screen,
}

impl Approach {
    /// Builds the screen on `previous` CHR memory and palette (the scene before leaves tiles
    /// the picture does not overwrite).
    pub fn new(
        graphics: &Graphics,
        facts: &Facts,
        previous: Option<&Screen>,
        original_rules: bool,
    ) -> Result<Approach, MissingGraphics> {
        let mut screen = Screen::default();
        if let Some(previous) = previous {
            screen.chr = previous.chr.clone();
        }
        let mut loads = FONT_LOADS.to_vec();
        loads.extend([BACKGROUND, SPRITES, STATUS_PANEL, STATUS_PANEL_1D60]);
        screen.load_tiles(graphics, &loads)?;
        let picture = graphics
            .pictures
            .get(PICTURE)
            .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?;
        screen.load_picture(picture);
        let preset = graphics
            .presets
            .get(PRESET)
            .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET}")))?;
        screen.palette.apply(preset);
        if let Some(Some(globe)) = graphics.approach_globe.get(facts.level) {
            // Quirk Q34: on the surface pictures 1 and 5 (planets 2, 4, 7, 9) the original writes
            // the colours to the unused entries 21-23 and leaves preset 0's in the globe's.
            let wrong_entries = original_rules && matches!(facts.level, 2 | 4 | 7 | 9);
            let first = if wrong_entries { 21 } else { 9 };
            for (i, &colour) in globe.iter().enumerate() {
                screen.palette.0[first + i] = colour;
            }
        }
        for (i, tile) in (GLOBE_FIRST_TILE..).take(15).enumerate() {
            // The block's top-left to bottom-right, row by row, without the bottom row's first.
            let slot = if i < 12 { i } else { i + 1 };
            screen.set_tile(14 + slot % 4, 8 + slot / 4, tile);
        }
        for (index, byte) in GLOBE_ATTRIBUTES {
            screen.set_attribute(index, byte);
        }
        screen.print(7, 2, "APPROACHING PLANET");
        let name = PLANETS[facts.level.min(13)];
        let column = 15 - (name.len() - 1) / 2;
        screen.print(column, 5, name);
        screen.print(3, 15, "GRAVITY");
        screen.print(11, 15, &signed_number(facts.gravity));
        screen.print(17, 15, "DIAMETER");
        screen.print(26, 15, &signed_number(facts.diameter));
        screen.print(5, 18, "PASSWORD");
        for (i, &letter) in facts.password.iter().enumerate() {
            screen.set_tile(14 + i, 18, letter);
        }
        screen.print(6, 21, "PRESS START TO LAND");
        screen.sprite_table = 0;
        screen.refresh();
        screen.sprites = GLOBE_SPRITES
            .iter()
            .map(|&(x, y, tile)| Sprite::new(x, y, tile, 2))
            .collect();
        // The status panel on page 1.
        screen.open_panel();
        screen.print_panel(1, 4, "SYSTEM");
        let system = facts.level / 3;
        screen.set_panel_tile(8, 4, b'1' + system as u8);
        screen.print_panel(2, 6, SYSTEMS[system.min(4)]);
        let hud = Hud {
            fuel: 0,
            weapon: 0,
            jetpods: facts.jetpods,
            lives: facts.lives,
            score: facts.score,
            negative: false,
        };
        for (row, tiles) in hud.icons_and_score().iter().enumerate() {
            for (column, &tile) in tiles.iter().enumerate().skip(16) {
                screen.set_panel_tile(column, 4 + row, tile);
            }
        }
        for (group, &byte) in PANEL_ATTRIBUTES.iter().enumerate() {
            screen.set_panel_attribute(8 + group, byte);
        }
        Ok(Approach { screen })
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// One pass: any newly pressed button lands.
    pub fn step(&mut self, clock: &mut Clock, held: Buttons, pressed: Buttons) -> ApproachOutcome {
        clock.advance(held, pressed);
        if pressed.0 != 0 {
            ApproachOutcome::Land
        } else {
            ApproachOutcome::Running
        }
    }
}
