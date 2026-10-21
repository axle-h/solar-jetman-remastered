//! The password entry screen (`docs/spec/scenes.md` 4, `docs/spec/password.md` 9, 6).

use super::FONT_LOADS;
use super::ppu::{Screen, Sprite};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{Graphics, MissingGraphics};
use crate::game::password::Saved;

const PICTURE: &str = "pictures/menu-b.ron";
const PRESET_FRAME: usize = 24;
const PRESET_AFTER: usize = 9;
/// The cursor: tile $5C (a hyphen glyph used as an underline) at x = 80 + 8 × place, line 101.
const CURSOR_TILE: u8 = 0x5C;
const CURSOR_LEFT: i32 = 80;
const CURSOR_LINE: i32 = 101;
/// Frames a held Up or Down waits before repeating, and between repeats (password.md 9).
const REPEAT_DELAY: u32 = 32;
const REPEAT_RATE: u32 = 14;
const PLACES: usize = 12;

/// What pressing A, B or Start did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EntryOutcome {
    Running,
    /// A valid password: the saved state to start from.
    Valid(Saved),
    /// Start on an invalid password: a new game begins.
    NewGame,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    screen: Screen,
    letters: [u8; PLACES],
    cursor: usize,
    /// Frames Up or Down has been held since it was last pressed or repeated.
    held_for: u32,
}

impl Entry {
    /// Builds the screen on top of what the previous scene left: CHR memory (only the menu
    /// frame and the font are rewritten, graphics.md 5.9) and the palette (the presets below
    /// cover the background entries 1-11 and 9-15; the sprite colours stay).
    pub fn new(graphics: &Graphics, previous: Option<&Screen>) -> Result<Entry, MissingGraphics> {
        let mut screen = Screen::default();
        if let Some(previous) = previous {
            screen.chr = previous.chr.clone();
            screen.palette = previous.palette;
        }
        screen.sprite_table = 1;
        let mut loads = vec![("tiles/menus/frame-and-font.png", 0x1000)];
        loads.extend(FONT_LOADS);
        screen.load_tiles(graphics, &loads)?;
        let picture = graphics
            .pictures
            .get(PICTURE)
            .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?;
        screen.load_picture(picture);
        screen.print(9, 9, "ENTER PASSWORD");
        screen.print(5, 15, "OR PRESS START TO PLAY");
        for preset in [PRESET_FRAME, PRESET_AFTER] {
            let preset = graphics
                .presets
                .get(preset)
                .ok_or_else(|| MissingGraphics(format!("palette preset {preset}")))?;
            screen.palette.apply(preset);
        }
        screen.scroll = (0, 2);
        // The leftmost 8 pixels of the background are masked on this screen.
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let mut entry = Entry {
            screen,
            letters: [b'A'; PLACES],
            cursor: 0,
            held_for: 0,
        };
        entry.refresh();
        Ok(entry)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn letters(&self) -> &[u8; PLACES] {
        &self.letters
    }

    /// The place the cursor is on (0-11).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Puts letters in the places, for tests.
    #[cfg(test)]
    pub(crate) fn set_letters(&mut self, letters: &[u8; PLACES]) {
        self.letters = *letters;
        self.refresh();
    }

    fn refresh(&mut self) {
        for (i, &letter) in self.letters.iter().enumerate() {
            self.screen.set_tile(10 + i, 12, letter);
        }
        let x = CURSOR_LEFT + 8 * self.cursor as i32;
        let mut cursor = Sprite::new(x, CURSOR_LINE, CURSOR_TILE, 1);
        cursor.behind = false;
        self.screen.sprites = vec![cursor];
    }

    fn step_letter(&mut self, forward: bool) {
        let letter = &mut self.letters[self.cursor];
        *letter = match (forward, *letter) {
            (true, b'Z') => b'A',
            (true, l) => l + 1,
            (false, b'A') => b'Z',
            (false, l) => l - 1,
        };
    }

    /// One pass: the cursor and the letters, or the validation.
    pub fn step(&mut self, held: Buttons, pressed: Buttons) -> EntryOutcome {
        let validate = Buttons::A | Buttons::B | Buttons::START;
        if pressed.0 & validate != 0 {
            return match Saved::decode(&self.letters).filter(Saved::is_playable) {
                Some(saved) => EntryOutcome::Valid(saved),
                None if pressed.has(Buttons::START) => EntryOutcome::NewGame,
                None => EntryOutcome::Running,
            };
        }
        let vertical = Buttons::UP | Buttons::DOWN;
        if held.0 & vertical != 0 {
            let mut step = false;
            if pressed.0 & vertical != 0 {
                self.held_for = 0;
                step = true;
            } else {
                self.held_for += 1;
                if self.held_for >= REPEAT_DELAY {
                    self.held_for = REPEAT_DELAY - REPEAT_RATE;
                    step = true;
                }
            }
            if step {
                // With both held, Up wins.
                self.step_letter(!held.has(Buttons::UP));
            }
        }
        if pressed.has(Buttons::LEFT) && self.cursor > 0 {
            self.cursor -= 1;
        }
        if pressed.has(Buttons::RIGHT) && self.cursor + 1 < PLACES {
            self.cursor += 1;
        }
        self.refresh();
        EntryOutcome::Running
    }
}
