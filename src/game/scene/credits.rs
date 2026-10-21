//! The credits (`docs/spec/scenes.md` 14.3, `docs/spec/text.md` 4.7): the lines scroll up the
//! page one pixel every two frames over the Space Loonies badge, lines 233 on hidden,
//! for 2,274 frames; then the program restarts. No input in the original; song 32 plays on.
//! In the remake any button ends them (`Game::set_skip_scenes`).

use super::FONT_LOADS;
use super::ppu::{Screen, Sprite};
use super::title::badge_sprites_at;
use crate::game::graphics::{Graphics, MissingGraphics};

const PRESET: usize = 20;
const LOGO: (&str, u16) = ("tiles/attract/space-loonies-logo.png", 0x0C00);
/// The lines and the blank rows after each (text.md 4.7); 11 blank rows come first.
const LINES: [(&str, usize); 22] = [
    ("SOLAR JETMAN CREDITS", 5),
    ("CONCEPT BY", 2),
    ("RARE", 5),
    ("SOFTWARE  BY", 2),
    ("STEVE HUGHES", 5),
    ("GRAPHICS  BY", 2),
    ("LYNDON  BROOKE", 2),
    ("STE PICKFORD", 5),
    ("SOUND EFFECTS BY", 2),
    ("MARK  BETTERIDGE", 5),
    ("MUSIC BY", 2),
    ("DAVE  WISE", 5),
    ("CRITIQUING BY", 2),
    ("SIMON FARMER", 2),
    ("GREGG MAYLES", 2),
    ("STEPHEN STAMPER", 2),
    ("HUW WARD", 5),
    ("ADDITIONAL MATERIAL BY", 2),
    ("JOHN  PICKFORD", 2),
    ("TIM  STAMPER", 5),
    ("PRODUCED BY RARE LTD", 5),
    ("A TRADE WEST PRODUCTION", 39),
];
const FIRST_ROW: usize = 11;
/// Rows before the end marker.
const ROWS: usize = 141;
/// Frames the credits show.
pub const FRAMES: u32 = 2274;
/// The screen line from which rendering is off.
const HIDDEN_FROM: usize = 233;
/// With the original's rules line 232 is drawn only up to x = 120 (scenes.md 14.3).
const PARTIAL_LINE: (usize, usize) = (232, 121);
/// The line at which the warm reset cuts the last frame (Q43), and the badge sprites the first
/// lit frame lacks (Q42).
const CUT_LINE: usize = 82;
const MISSING_AT_FIRST: [usize; 2] = [24, 25];
/// The badge's first row in the credits, and how far left of the title's it sits.
const BADGE_Y: i32 = 69;
const BADGE_DX: i32 = -8;

/// What a pass of the credits leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CreditsOutcome {
    Running,
    /// The end marker was read and its last frames shown: the program restarts.
    Ended,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Credits {
    screen: Screen,
    /// Frames shown so far; the scroll is half of it.
    frame: u32,
    #[serde(with = "crate::game::state::static_str_rows")]
    rows: Vec<Option<&'static str>>,
    /// The badge, shown from the third frame.
    sprites: Vec<Sprite>,
    /// The original's rules: line 232 cut at x = 120 (scenes.md 14.3), the first lit frame
    /// short of two sprites (Q42) and the last cut at line 82 (Q43).
    original_rules: bool,
    /// Sprites held back from the first lit frame, with their places.
    skipped: Vec<(usize, Sprite)>,
}

impl Credits {
    pub fn new(graphics: &Graphics) -> Result<Credits, MissingGraphics> {
        let mut screen = Screen::default();
        let mut loads = FONT_LOADS.to_vec();
        loads.push(LOGO);
        screen.load_tiles(graphics, &loads)?;
        screen.palette.apply(
            graphics
                .presets
                .get(PRESET)
                .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET}")))?,
        );
        let mut sprites = Vec::new();
        badge_sprites_at(BADGE_Y, BADGE_DX, &mut sprites);
        for sprite in &mut sprites {
            sprite.behind = true;
        }
        // The console's sprite transfer comes two frames in: the badge is there from the second.
        screen.hide_from = Some(HIDDEN_FROM);
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let mut rows = vec![None; FIRST_ROW];
        for (text, blanks) in LINES {
            rows.push(Some(text));
            rows.extend(std::iter::repeat_n(None, blanks));
        }
        debug_assert_eq!(rows.len(), ROWS);
        Ok(Credits {
            screen,
            frame: 0,
            rows,
            sprites,
            original_rules: false,
            skipped: Vec::new(),
        })
    }

    /// Chooses the original's rules (`Game::set_original_rules`) over the remake's: line 232
    /// drawn to x = 120 only, and the first and last lit frames incomplete.
    pub fn set_original_rules(&mut self, on: bool) {
        self.original_rules = on;
        self.screen.hide_line_from = on.then_some(PARTIAL_LINE);
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// One pass: the scroll goes up a pixel every second frame, and each 8 pixels the next row
    /// of the data is written at the bottom.
    pub fn step(&mut self) -> CreditsOutcome {
        self.frame += 1;
        if self.frame >= FRAMES {
            return CreditsOutcome::Ended;
        }
        // Measured: the scroll goes up on the odd frames, from the third.
        if self.frame == 1 {
            self.screen.sprites = std::mem::take(&mut self.sprites);
            if self.original_rules {
                // Q42: the buffer is still part old in the first lit frame.
                for &i in MISSING_AT_FIRST.iter().rev() {
                    if i < self.screen.sprites.len() {
                        self.skipped.push((i, self.screen.sprites.remove(i)));
                    }
                }
            }
        } else if self.frame == 2 {
            while let Some((i, sprite)) = self.skipped.pop() {
                self.screen.sprites.insert(i, sprite);
            }
        }
        if self.original_rules && self.frame == FRAMES - 1 {
            // Q43: the warm reset cuts the last frame.
            self.screen.hide_from = Some(CUT_LINE);
        }

        let steps = (self.frame.saturating_sub(1) / 2) as usize;
        self.screen.scroll.1 = steps % 240;
        if self.frame.is_multiple_of(16) {
            let l = (self.frame / 16) as usize - 1;
            let row = l % 30;
            for column in 0..32 {
                self.screen.set_tile(column, row, 0);
            }
            if let Some(Some(text)) = self.rows.get(l) {
                let start = (32 - text.len()) / 2;
                self.screen.print(start, row, text);
            }
        }
        CreditsOutcome::Running
    }
}
