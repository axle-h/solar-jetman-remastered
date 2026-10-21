//! The badge intermission (`docs/spec/scenes.md` 11.2): the Federation of Space Loonies badge
//! rises from below the screen, lands, and counts the gems collected in the backdrop interlude
//! (or on a warp level), one every 16 frames.

use super::message::load_panel;
use super::ppu::Screen;
use super::title::badge_sprites;
use super::{Clock, SoundRequest};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::sprites::COMMON_SPRITES;
use crate::game::graphics::{ChrLoad, Graphics, MissingGraphics, Palette};
use crate::game::hud::Hud;

/// The badge's first row before its first pass, and where it rests (2 lines a pass).
const START_Y: i32 = 231;
const REST_Y: i32 = 21;
/// The pass in which the badge counts as landed: the first row is on line 23, two lines above
/// its place (the counter is reset there).
const LANDING_Y: i32 = 23;
/// Palette preset of the badge (graphics.md 7.2) and its sprite tiles (graphics.md 5.3).
const PRESET: usize = 20;
const LOGO: (&str, u16) = ("tiles/attract/space-loonies-logo.png", 0x0C00);
/// The scene ends when the counter reaches this, counted from the landing.
const END_COUNTER: u32 = 254;
/// A crystal is taken every 16 frames from the landing on, the first after 16.
const COUNT_EVERY: u32 = 16;
/// The gem picture: 2 × 2 background tiles, the rows and columns they go to (the first five in
/// the first row), and the background palette of the cells (attributes $AA).
const GEM_TILES: [u8; 4] = [0xA0, 0xA1, 0xA2, 0xA3];
/// The sprite tiles the gem's upper and lower halves are made from.
const GEM_SPRITE_TILES: [usize; 2] = [0xDB, 0xDC];
const GEM_ROWS: [usize; 2] = [16, 19];
const GEMS_IN_FIRST_ROW: usize = 5;

/// What a pass of the intermission leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BadgeOutcome {
    Running,
    /// The scene is over; `gain` points were counted in all.
    Ended {
        gain: u32,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Badge {
    screen: Screen,
    /// The first row's line.
    y: i32,
    landed: bool,
    /// Crystals not counted yet, and counted.
    gems: u8,
    counted: u8,
    gain: u32,
    hud: Hud,
    /// The sprites built in the last pass: the console's sprite transfer comes a frame later.
    pending_sprites: Vec<super::ppu::Sprite>,
    /// Tiles written in the last pass: the video buffer is flushed a pass later.
    pending_tiles: Vec<(usize, usize, u8)>,
    /// The effect of the crystal counted in this pass.
    sound: Option<u8>,
}

impl Badge {
    /// The intermission after a level, over the gameplay palette `palette` and the status
    /// panel `hud`, with `gems` crystals to count.
    pub fn new(
        graphics: &Graphics,
        background: &[ChrLoad],
        palette: &Palette,
        hud: Hud,
        gems: u8,
    ) -> Result<Badge, MissingGraphics> {
        let mut screen = Screen::default();
        screen.chr.apply(graphics, background)?;
        screen.load_tiles(graphics, &[LOGO])?;
        // The gem's four background tiles are the item's two sprite tiles ($DB, $DC of the
        // common sprites) and their mirror images (measured: $A0-$A3 at $1A00).
        let loads: Vec<_> = COMMON_SPRITES
            .iter()
            .map(|&(stream, address)| crate::game::graphics::ChrLoad {
                stream,
                range: None,
                address,
            })
            .collect();
        for (i, (tile, mirror)) in [
            (GEM_SPRITE_TILES[0], false),
            (GEM_SPRITE_TILES[0], true),
            (GEM_SPRITE_TILES[1], false),
            (GEM_SPRITE_TILES[1], true),
        ]
        .into_iter()
        .enumerate()
        {
            screen.chr.copy_derived(
                graphics,
                &loads,
                tile * 16,
                16,
                mirror,
                0x1A00 + 16 * i as u16,
            )?;
        }
        screen.refresh();
        load_panel(&mut screen, graphics, hud)?;
        screen.palette = *palette;
        let preset = graphics
            .presets
            .get(PRESET)
            .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET}")))?;
        screen.palette.apply(preset);
        for index in 0..64 {
            screen.set_attribute(index, 0xAA);
        }
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let mut badge = Badge {
            screen,
            y: START_Y,
            landed: false,
            gems,
            counted: 0,
            gain: 0,
            hud,
            pending_sprites: Vec::new(),
            pending_tiles: Vec::new(),
            sound: None,
        };
        badge.place_sprites();
        Ok(badge)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The sound the scene starts with: silence (scenes.md 15).
    pub fn song(&self) -> SoundRequest {
        SoundRequest::SongIfNew(0)
    }

    /// The effect asked for in the last pass, on both pulses: $1E, $20, ..., $30 for the first
    /// to the tenth crystal counted (sound.md 8.3).
    pub fn take_sound(&mut self) -> Option<u8> {
        self.sound.take()
    }

    fn place_sprites(&mut self) {
        let mut sprites = Vec::new();
        badge_sprites(self.y, &mut sprites);
        self.pending_sprites = sprites;
    }

    /// One pass.
    pub fn step(&mut self, clock: &mut Clock, held: Buttons, pressed: Buttons) -> BadgeOutcome {
        self.screen.sprites = std::mem::take(&mut self.pending_sprites);
        for (column, row, tile) in std::mem::take(&mut self.pending_tiles) {
            self.screen.set_tile(column, row, tile);
        }
        clock.advance_split(held, pressed);
        let seen = clock.counter;
        if self.y > REST_Y {
            self.y -= 2;
        }
        if !self.landed && self.y == LANDING_Y {
            // The scene resets the counter itself (to 0: the next pass's increment makes it 1).
            self.landed = true;
            clock.counter = 0;
        } else if self.landed && seen != 0 && seen.is_multiple_of(COUNT_EVERY) && self.gems != 0 {
            self.take_one();
        }
        self.place_sprites();
        if self.landed && clock.counter & 0xFF == END_COUNTER {
            return BadgeOutcome::Ended { gain: self.gain };
        }
        BadgeOutcome::Running
    }

    /// One crystal is counted: 100 points (1,100 from the tenth), a gem on the screen.
    fn take_one(&mut self) {
        let index = usize::from(self.counted);
        // The series stops at the tenth step ($30).
        self.sound = Some(0x1E + 2 * self.counted.min(9));
        self.gems -= 1;
        self.counted += 1;
        let points = if self.counted >= 10 { 1100 } else { 100 };
        self.gain += points;
        self.hud.score += points;
        self.screen.panel = Some(self.hud.panel_page());
        let (row, column) = if index < GEMS_IN_FIRST_ROW {
            (GEM_ROWS[0], 8 + 4 * index)
        } else {
            (GEM_ROWS[1], 8 + 4 * (index - GEMS_IN_FIRST_ROW))
        };
        for (i, &tile) in GEM_TILES.iter().enumerate() {
            self.pending_tiles.push((column + i % 2, row + i / 2, tile));
        }
    }
}
