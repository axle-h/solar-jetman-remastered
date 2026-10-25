//! The title sequence (`docs/spec/scenes.md` 2, `docs/spec/game-flow.md` 7.1): the sun and the
//! logo among 56 stars, the logo's fade-in and slide, the menu with a Jetman walking along the
//! bottom, and the "Federation of Space Loonies" badge.
//!
//! One call of [`Title::step`] is one pass of the original's title handler, so the frame
//! numbers in the comments are passes counted by the frame counter ($5D/$5E), which the title
//! sets to 1 before its first pass (the original's loading frame).

use super::ppu::{Screen, Sprite};
use super::{Clock, FONT_LOADS, Select, SoundRequest};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::sprites::COMMON_SPRITES;
use crate::game::graphics::{Graphics, MissingGraphics};

/// The 56 stars: sprite memory y, x and shape (scenes.md 2.1). The original reads them from its
/// own code; the numbers are the ones seen in its sprite memory.
const STARS: [(u8, u8, u8); 56] = [
    (16, 189, 0),
    (133, 17, 1),
    (247, 132, 0),
    (16, 24, 1),
    (1, 136, 0),
    (189, 227, 0),
    (86, 169, 3),
    (85, 169, 0),
    (83, 76, 1),
    (230, 12, 3),
    (133, 13, 1),
    (133, 201, 1),
    (125, 164, 3),
    (13, 192, 1),
    (41, 169, 3),
    (83, 185, 1),
    (141, 2, 3),
    (3, 1, 1),
    (201, 20, 3),
    (201, 23, 2),
    (202, 134, 0),
    (237, 247, 0),
    (1, 133, 1),
    (34, 133, 1),
    (119, 133, 1),
    (169, 35, 1),
    (169, 36, 1),
    (160, 26, 1),
    (185, 50, 0),
    (1, 1, 1),
    (247, 136, 0),
    (160, 7, 2),
    (247, 153, 2),
    (136, 16, 1),
    (0, 32, 2),
    (169, 64, 3),
    (2, 169, 1),
    (31, 2, 3),
    (2, 141, 1),
    (169, 128, 3),
    (2, 169, 2),
    (23, 3, 3),
    (141, 116, 2),
    (119, 133, 0),
    (240, 3, 1),
    (132, 165, 2),
    (165, 13, 2),
    (14, 72, 1),
    (169, 1, 2),
    (32, 172, 1),
    (1, 160, 2),
    (248, 32, 0),
    (104, 133, 1),
    (133, 13, 1),
    (12, 162, 0),
    (92, 176, 0),
];

/// A sprite line below the screen, for side stars that are not shown.
const SCREEN_OFF: i32 = 0x100;

/// Sprite-memory y at and above which the slide hides a star (2.1).
const HIDE_FROM: u8 = 0x90;
const HIDDEN: u8 = 0xF8;

/// The sun's camera: where it starts, where it settles (2.2).
const CAMERA_START: (i32, i32) = (88, 236);
const CAMERA_TARGET: (i32, i32) = (4, 176);
/// The menu Jetman's target lines (2.3): the low one except for passes 194 to 705.
const FIGURE_LOW: i32 = 192;
const FIGURE_HIGH: i32 = 128;
const FIGURE_HIGH_FROM: u32 = 194;
const FIGURE_HIGH_TO: u32 = 705;
/// Scroll once the title is on screen (2.3).
const MENU_SCROLL: (usize, usize) = (4, 224);

const SUBSTATE_SUN: u8 = 0;
const SUBSTATE_FADE: u8 = 1;
const SUBSTATE_SLIDE: u8 = 2;
const SUBSTATE_WRITE: u8 = 3;
const SUBSTATE_BOX: u8 = 4;
const SUBSTATE_MENU: u8 = 5;
const SUBSTATE_BADGE: u8 = 6;

/// Frames of the fade, the slide, the menu's walker and the badge (the sun scene ends when the
/// counter's high byte reaches 2).
const FADE_FRAMES: u32 = 64;
const SLIDE_FRAMES: u32 = 48;
const MENU_FRAMES: u32 = 994;
const BADGE_FRAMES: u32 = 256;

/// Tiles of the menu box (text.md 4.6, the title's copy of the frame pieces at $10).
const BOX_TOP_LEFT: u8 = 0x10;
const BOX_TOP_RIGHT: u8 = 0x11;
const BOX_BOTTOM_LEFT: u8 = 0x12;
const BOX_BOTTOM_RIGHT: u8 = 0x13;
const BOX_HORIZONTAL: u8 = 0x14;
const BOX_VERTICAL: u8 = 0x15;

const PRESET_TITLE: usize = 5;
const PRESET_BADGE: usize = 20;
const PICTURE: &str = "pictures/title.ron";

/// What the title asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TitleOutcome {
    Running,
    /// START GAME chosen.
    NewGame,
    /// CONTINUE chosen: the password screen.
    Continue,
    /// The badge is over: the attract cycle goes on.
    Timeout,
}

/// A write to the title's nametable.
#[derive(Debug, Clone, Copy)]
enum Write {
    Text(usize, usize, &'static str),
    Tile(usize, usize, u8),
}

/// A [`Write`] as saved: the text owned.
#[derive(serde::Serialize, serde::Deserialize)]
enum WriteSaved {
    Text(usize, usize, String),
    Tile(usize, usize, u8),
}

/// `Vec<Write>` as saved.
mod writes_saved {
    use super::{Write, WriteSaved};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(writes: &[Write], s: S) -> Result<S::Ok, S::Error> {
        let saved: Vec<WriteSaved> = writes.iter().map(|&w| w.into()).collect();
        saved.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Write>, D::Error> {
        Ok(Vec::<WriteSaved>::deserialize(d)?
            .into_iter()
            .map(Write::from)
            .collect())
    }
}

impl From<Write> for WriteSaved {
    fn from(write: Write) -> WriteSaved {
        match write {
            Write::Text(c, r, text) => WriteSaved::Text(c, r, text.to_owned()),
            Write::Tile(c, r, tile) => WriteSaved::Tile(c, r, tile),
        }
    }
}

impl From<WriteSaved> for Write {
    fn from(write: WriteSaved) -> Write {
        match write {
            WriteSaved::Text(c, r, text) => Write::Text(c, r, crate::game::state::intern(&text)),
            WriteSaved::Tile(c, r, tile) => Write::Tile(c, r, tile),
        }
    }
}

/// A star's own state.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Star {
    y: u8,
    x: u8,
    shape: u8,
    own_palette: u8,
    behind: bool,
    /// Removed for the walking Jetman's sprites (2.1, "Shared slots").
    removed: bool,
}

/// A helper object that homes on a target along one axis (`scenes.md` 2.2, 2.3, with the
/// scheme of `behaviour.md` 2.3): an 8.8 position, a speed magnitude and a direction flag.
/// The sun's camera and the menu's Jetman use it with different constants.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Homing {
    position: i32,
    speed: i32,
    /// Moving toward larger values.
    positive: bool,
    target: i32,
    accelerate: i32,
    cap: i32,
    brake: i32,
    /// Below this speed the brake stops the object and turns it.
    snap: i32,
    /// Added to the speed of every move that is made.
    drift: i32,
}

impl Homing {
    /// One axis of the sun's camera: accelerate 12, brake 20, capped at $02FC, plus 4/256.
    fn camera(start: i32, target: i32, speed: i32, positive: bool) -> Homing {
        Homing {
            position: start << 8,
            speed,
            positive,
            target,
            accelerate: 12,
            cap: 0x2FC,
            brake: 12 + 8,
            snap: 20,
            drift: 4,
        }
    }

    /// The menu's Jetman, vertically: accelerate and brake 12, capped at 1.00.
    fn figure(start: i32) -> Homing {
        Homing {
            position: start << 8,
            speed: 0,
            positive: true,
            target: FIGURE_LOW,
            accelerate: 12,
            cap: 0x100,
            brake: 12,
            snap: 13,
            drift: 0,
        }
    }

    fn moves(&mut self, amount: i32) {
        self.position += if self.positive { amount } else { -amount };
    }

    fn step(&mut self) {
        let toward = self.positive == (self.target >= self.position >> 8);
        if toward {
            self.speed = (self.speed + self.accelerate).min(self.cap);
            self.moves(self.speed + self.drift);
        } else if self.speed == 0 {
            self.positive = !self.positive;
        } else if self.speed < self.snap {
            self.speed = 0;
            self.positive = !self.positive;
        } else {
            self.speed -= self.brake;
            self.moves(self.speed + self.drift);
        }
    }

    fn pixel(&self) -> i32 {
        self.position >> 8
    }
}

/// The title and its attract badge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Title {
    screen: Screen,
    substate: u8,
    /// Frame counter value when the substate started.
    entered: u32,
    /// The fade's next step (0-6).
    fade_step: u8,
    #[serde(with = "crate::game::state::array")]
    stars: [Star; 56],
    /// The star in the flash palette.
    flashing: Option<usize>,
    camera: [Homing; 2],
    /// The scroll the next pass puts on the screen.
    scroll_next: (usize, usize),
    /// Nametable writes made by the last pass, which reach the picture one pass later.
    #[serde(with = "writes_saved")]
    writes: Vec<Write>,
    selection: Select,
    /// Passes since the menu started (`s` of scenes.md 2.3).
    walked: u32,
    /// The menu Jetman's height, whether he was rising when the pass began (his pose follows
    /// one pass behind), and the exhaust flame's offset d while its slot is in use
    /// (`player.md` 4.8).
    figure: Homing,
    figure_rising: bool,
    figure_before: i32,
    flame: Option<u8>,
    /// Preset 5 as loaded, for the sun's flicker.
    sun: (u8, u8),
    /// Colours of the fade (graphics.md 7.4).
    #[serde(with = "crate::game::state::title_fade")]
    fade: Vec<Vec<u8>>,
    #[serde(with = "crate::game::state::title_fade_end")]
    fade_end: Vec<u8>,
    white: u8,
    /// Song to start with the first pass.
    song_pending: bool,
    /// The engine sound the menu Jetman asks for while he rises (player.md 4.8).
    engine: bool,
}

impl Title {
    /// Builds the title screen (graphics.md 5.2) and sets the clock as its first pass finds it.
    pub fn new(graphics: &Graphics, clock: &mut Clock) -> Result<Title, MissingGraphics> {
        let mut screen = Screen::default();
        let mut loads: Vec<(&'static str, u16)> = FONT_LOADS.to_vec();
        loads.push(("tiles/font/digits.png", 0x1C00));
        loads.push(("tiles/font/letters.png", 0x1D10));
        loads.push(("tiles/font/frame.png", 0x1F00));
        loads.push(("tiles/font/frame.png", 0x1100));
        loads.push(("tiles/title/logo.png", 0x1640));
        loads.push(("tiles/title/sprites-0800.png", 0x0800));
        loads.push(("tiles/title/sprites-0860.png", 0x0860));
        loads.push(("tiles/title/sprites-0b80.png", 0x0B80));
        loads.push(("tiles/sprites/title.png", 0x0010));
        loads.extend(COMMON_SPRITES.iter().copied());
        screen.load_tiles(graphics, &loads)?;
        let picture = graphics
            .pictures
            .get(PICTURE)
            .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?;
        screen.load_picture(picture);

        let preset = graphics
            .presets
            .get(PRESET_TITLE)
            .ok_or_else(|| MissingGraphics("palette preset 5".to_owned()))?;
        screen.palette.apply(preset);
        let sun = (screen.palette.0[13], screen.palette.0[15]);
        // The logo starts black (the fade brings it in).
        for entry in [1, 2, 3, 5, 6, 7, 9, 10, 11] {
            screen.palette.0[entry] = 0x0F;
        }
        screen.scroll = (CAMERA_START.0 as usize, CAMERA_START.1 as usize);
        // The left-column mask hides background and sprites (graphics.md 1.3).
        screen.blank_left = true;
        screen.blank_left_sprites = true;

        let stars = STARS.map(|(y, x, shape)| Star {
            y,
            x,
            shape,
            own_palette: 0,
            behind: false,
            removed: false,
        });
        let mut stars = stars;
        for (n, star) in stars.iter_mut().enumerate() {
            star.own_palette = match n % 4 {
                3 => 0,
                other => other as u8,
            };
        }
        clock.counter = 1;
        clock.job = 0;
        let mut title = Title {
            screen,
            substate: SUBSTATE_SUN,
            entered: 1,
            fade_step: 0,
            stars,
            flashing: None,
            scroll_next: (CAMERA_START.0 as usize, CAMERA_START.1 as usize),
            writes: Vec::new(),
            camera: [
                Homing::camera(CAMERA_START.0, CAMERA_TARGET.0, 0, true),
                Homing::camera(CAMERA_START.1, CAMERA_TARGET.1, 0x200, false),
            ],
            selection: Select::StartGame,
            walked: 0,
            figure: Homing::figure(128),
            figure_rising: true,
            figure_before: 128,
            flame: None,
            sun,
            fade: graphics.title_fade.clone(),
            fade_end: graphics.title_fade_end.clone(),
            white: graphics.title_white,
            song_pending: true,
            engine: false,
        };
        title.place_sprites();
        Ok(title)
    }

    /// The song, if it is yet to be requested (until the first pass). The original plays it
    /// once (scenes.md 15); here it loops through the title, the story and the badge.
    pub fn take_song_request(&mut self) -> Option<SoundRequest> {
        std::mem::take(&mut self.song_pending).then_some(SoundRequest::SongLooped(1))
    }

    /// Whether the menu Jetman asked for the engine sound in this pass, once.
    pub fn take_engine_sound(&mut self) -> bool {
        std::mem::take(&mut self.engine)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn substate(&self) -> u8 {
        self.substate
    }

    fn sprite_attributes(star: &Star) -> u8 {
        star.own_palette
    }

    /// Rebuilds the sprite list: cursor, walking Jetman, then the stars in their slots.
    fn place_sprites(&mut self) {
        let mut sprites = Vec::new();
        let mut flame = None;
        match self.substate {
            SUBSTATE_MENU if self.walked >= 1 => {
                let y = 162 + 16 * self.selection.row();
                for x in [86, 173] {
                    sprites.push(Sprite::new(x, y, 0x85, 1));
                }
                flame = self.walker_sprites(&mut sprites);
            }
            SUBSTATE_BADGE => {
                self.badge_sprites(&mut sprites);
            }
            _ => {}
        }
        if self.substate != SUBSTATE_BADGE {
            for (n, star) in self.stars.iter().enumerate() {
                if star.removed || star.y >= 0xEF {
                    continue;
                }
                let mut sprite = Sprite::new(
                    i32::from(star.x),
                    i32::from(star.y) + 1,
                    0x80 + star.shape,
                    if self.flashing == Some(n) {
                        3
                    } else {
                        Self::sprite_attributes(star)
                    },
                );
                sprite.behind = star.behind;
                sprites.push(sprite);
            }
        }
        sprites.extend(flame);
        self.screen.sprites = sprites;
        self.screen.side_sprites = self.side_stars();
    }

    /// The stars again either side of a wider view (`docs/widescreen.md`).
    fn side_stars(&self) -> Vec<Sprite> {
        if self.substate == SUBSTATE_BADGE {
            return Vec::new();
        }
        let stars: Vec<Sprite> = self
            .stars
            .iter()
            .enumerate()
            .map(|(n, star)| {
                let mut sprite = Sprite::new(
                    i32::from(star.x),
                    i32::from(star.y) + 1,
                    0x80 + star.shape,
                    if self.flashing == Some(n) {
                        3
                    } else {
                        Self::sprite_attributes(star)
                    },
                );
                sprite.behind = star.behind;
                // A removed or hidden star stays out of view with its partners.
                if star.removed || star.y >= 0xEF {
                    sprite.y = SCREEN_OFF;
                }
                sprite
            })
            .collect();
        super::ppu::side_stars(&stars)
    }

    /// The menu's Jetman (2.3): a 2 × 2 group along the bottom, 1 pixel every 4 passes, in the
    /// thrust or the fall pose, with the exhaust flame while its slot is in use. Before pass 18
    /// only the right-hand column is shown, as a pair.
    fn walker_sprites(&self, sprites: &mut Vec<Sprite>) -> Option<Sprite> {
        let s = self.walked as i32;
        if s < 2 {
            return None;
        }
        // Sprite-memory y of the bottom row is the height + 4; the screen line is one more.
        let bottom = self.figure.pixel() + 5;
        let feet = if self.figure_rising {
            (0x0C, 0x0D)
        } else {
            (0x0E, 0x0F)
        };
        if s < 18 {
            let x = 4 + (s - 2) / 4;
            sprites.push(Sprite::new(x, bottom, feet.1, 0));
            sprites.push(Sprite::new(x, bottom - 8, 0x0B, 0));
            return None;
        }
        let x = (s - 18).div_euclid(4);
        sprites.push(Sprite::new(x, bottom, feet.0, 0));
        sprites.push(Sprite::new(x + 8, bottom, feet.1, 0));
        sprites.push(Sprite::new(x, bottom - 8, 0x01, 0));
        sprites.push(Sprite::new(x + 8, bottom - 8, 0x0B, 0));
        // The flame is drawn from where the Jetman was before this pass's move.
        self.flame
            .map(|d| Sprite::new(x - 1, self.figure_before + 6, 0x10 + d, 1))
    }

    /// One pass of the menu Jetman: the figure's move (his pose is the direction he had when the
    /// pass began) and the flame (`player.md` 4.8: d is 2 or 1 while rising by the counter's
    /// bit 1, then grows at every multiple of 8 and the flame goes at 3).
    fn walk(&mut self, counter: u32) {
        let s = self.walked;
        // The counter value the figure's logic sees (fitted: s + 652 at pass s, where this pass's
        // counter is s + 654).
        let c = counter.wrapping_sub(2);
        if s == 3 {
            self.figure = Homing::figure(128);
        }
        self.figure_before = self.figure.pixel();
        if s >= 4 {
            self.figure_rising = !self.figure.positive;
            self.figure.target = if (FIGURE_HIGH_FROM..=FIGURE_HIGH_TO).contains(&s) {
                FIGURE_HIGH
            } else {
                FIGURE_LOW
            };
            self.figure.step();
            if self.figure_rising {
                self.flame = Some(if c & 2 == 0 { 2 } else { 1 });
            } else if let Some(d) = self.flame
                && c & 7 == 0
            {
                self.flame = (d + 1 < 3).then_some(d + 1);
            }
        }
        // The engine sound goes out a pass before the flame that shows it (Mesen, `sj-tools
        // scene --compare`): this pass decides for the next one.
        if s >= 3 {
            self.engine = !self.figure.positive && c.wrapping_add(1) & 2 == 0;
        }
    }

    /// The badge's 62 sprites (2.4).
    fn badge_sprites(&self, sprites: &mut Vec<Sprite>) {
        badge_sprites(69, sprites);
    }

    /// One pass of the title handler.
    pub fn step(
        &mut self,
        graphics: &Graphics,
        clock: &mut Clock,
        buttons: Buttons,
        pressed: Buttons,
    ) -> Result<TitleOutcome, MissingGraphics> {
        clock.advance(buttons, pressed);
        let c = clock.counter;
        // The scroll registers are written at the start of a frame's handler, before its logic:
        // what a pass computes is shown one pass later.
        self.screen.scroll = self.scroll_next;
        for write in std::mem::take(&mut self.writes) {
            match write {
                Write::Text(column, row, text) => self.screen.print(column, row, text),
                Write::Tile(column, row, tile) => self.screen.set_tile(column, row, tile),
            }
        }
        if self.substate <= SUBSTATE_MENU {
            self.flash_stars(clock);
            // The palette reaches the screen one pass later than the sprites do.
            self.flicker_sun(c.wrapping_sub(1));
        }
        let since = c - self.entered;
        let mut outcome = TitleOutcome::Running;
        match self.substate {
            SUBSTATE_SUN => {
                if pressed.has(Buttons::START) {
                    self.fade_step = 6;
                    self.enter(SUBSTATE_FADE, c);
                    // The skip leaves only the last step: the logo is bright 8 frames later.
                } else if c >> 8 >= 2 {
                    self.enter(SUBSTATE_FADE, c);
                } else {
                    self.step_camera();
                }
            }
            SUBSTATE_FADE => {
                // Palette writes show one pass later (as the sun's flicker does).
                if (c - 1).is_multiple_of(8) && c - 1 > self.entered && self.fade_step < 7 {
                    self.apply_fade(usize::from(self.fade_step));
                    self.fade_step += 1;
                }
                if since >= FADE_FRAMES {
                    for star in &mut self.stars {
                        star.behind = true;
                    }
                    self.enter(SUBSTATE_SLIDE, c);
                }
            }
            SUBSTATE_SLIDE => {
                self.scroll_next.1 += 1;
                if c.is_multiple_of(2) {
                    for star in &mut self.stars {
                        star.y = star.y.wrapping_sub(1);
                    }
                }
                if since >= SLIDE_FRAMES {
                    for star in &mut self.stars {
                        if star.y >= HIDE_FROM {
                            star.y = HIDDEN;
                        }
                    }
                    self.enter(SUBSTATE_WRITE, c);
                }
            }
            SUBSTATE_WRITE => {
                if since == 1 {
                    self.writes.push(Write::Text(12, 18, "START GAME"));
                } else if since == 2 {
                    self.writes.push(Write::Text(13, 20, "CONTINUE"));
                } else if since >= 3 {
                    self.enter(SUBSTATE_BOX, c);
                }
            }
            SUBSTATE_BOX => {
                if since <= 26 {
                    self.draw_box_piece(since);
                } else {
                    self.writes
                        .push(Write::Text(4, 25, "© COPYRIGHT 1990 RARE LTD."));
                    for star in &mut self.stars[54..] {
                        star.removed = true;
                    }
                    self.enter(SUBSTATE_MENU, c);
                }
            }
            SUBSTATE_MENU => {
                self.walked += 1;
                self.walk(c);
                if pressed.has(Buttons::SELECT) {
                    self.selection = self.selection.toggled();
                }
                if pressed.has(Buttons::START) {
                    outcome = match self.selection {
                        Select::StartGame => TitleOutcome::NewGame,
                        Select::Continue => TitleOutcome::Continue,
                    };
                } else if since >= MENU_FRAMES {
                    self.start_badge(graphics, clock)?;
                }
            }
            _ => {
                if clock.counter - self.entered >= BADGE_FRAMES {
                    outcome = TitleOutcome::Timeout;
                }
            }
        }
        self.place_sprites();
        Ok(outcome)
    }

    fn enter(&mut self, substate: u8, counter: u32) {
        self.substate = substate;
        self.entered = counter;
        match substate {
            SUBSTATE_FADE => {
                self.scroll_next = (CAMERA_TARGET.0 as usize, CAMERA_TARGET.1 as usize);
            }
            SUBSTATE_MENU => {
                self.scroll_next = MENU_SCROLL;
            }
            _ => {}
        }
    }

    fn step_camera(&mut self) {
        for axis in &mut self.camera {
            axis.step();
        }
        self.scroll_next = (
            (self.camera[0].pixel() & 0xFF) as usize,
            (self.camera[1].pixel() & 0xFF) as usize,
        );
    }

    /// The star flash and the sun's flicker follow the frame counter (2.1).
    fn flash_stars(&mut self, clock: &Clock) {
        let c = clock.counter;
        if c.is_multiple_of(16) {
            let mut v = clock.rng.out;
            if v < 32 {
                v += 32;
            }
            let n = usize::from(v & !3) / 4 - 8;
            self.flashing = Some(n.min(55));
        } else if c % 16 == 6 {
            self.flashing = None;
        }
    }

    fn flicker_sun(&mut self, counter: u32) {
        let palette = &mut self.screen.palette.0;
        if counter % 16 < 8 {
            palette[13] = self.sun.0;
            palette[15] = self.sun.1.wrapping_sub(0x10);
        } else {
            palette[13] = self.white;
            palette[15] = self.sun.1;
        }
    }

    /// One step of the logo's fade-in (graphics.md 7.4): the six columns go to entries 1, 2,
    /// 3, 5 and 9, 6 and 10, 7 and 11; the last step then sets entries 9-11.
    fn apply_fade(&mut self, step: usize) {
        let palette = &mut self.screen.palette.0;
        let targets: [&[usize]; 6] = [&[1], &[2], &[3], &[5, 9], &[6, 10], &[7, 11]];
        for (column, entries) in self.fade.iter().zip(targets) {
            if let Some(&colour) = column.get(step) {
                for &entry in entries {
                    palette[entry] = colour;
                }
            }
        }
        if step == 6 {
            for (i, &colour) in self.fade_end.iter().enumerate() {
                palette[9 + i] = colour;
            }
        }
    }

    /// The menu box's k-th update of 26 (scenes.md 2.3): corners, then the top and bottom edges
    /// two tiles at a time (the bottom one from the right), then the sides.
    fn draw_box_piece(&mut self, k: u32) {
        let (left, right, top, bottom) = (10, 23, 16, 22);
        let writes = &mut self.writes;
        let mut put =
            |column: usize, row: usize, tile: u8| writes.push(Write::Tile(column, row, tile));
        match k {
            1 => put(left, top, BOX_TOP_LEFT),
            2 => put(right, bottom, BOX_BOTTOM_RIGHT),
            3..=14 => {
                let pair = ((k - 3) / 2) as usize;
                let (column, row) = if k % 2 == 1 {
                    (11 + 2 * pair, top)
                } else {
                    (21 - 2 * pair, bottom)
                };
                put(column, row, BOX_HORIZONTAL);
                put(column + 1, row, BOX_HORIZONTAL);
            }
            15 => put(right, top, BOX_TOP_RIGHT),
            16 => put(left, bottom, BOX_BOTTOM_LEFT),
            _ => {
                let i = (k - 17) as usize;
                let (column, row) = if i.is_multiple_of(2) {
                    (right, 17 + i / 2)
                } else {
                    (left, 21 - i / 2)
                };
                put(column, row, BOX_VERTICAL);
            }
        }
    }

    /// Sub-state 6: the page is cleared, the badge's palette loaded, the counter set to 1.
    fn start_badge(
        &mut self,
        graphics: &Graphics,
        clock: &mut Clock,
    ) -> Result<(), MissingGraphics> {
        self.screen.clear_page();
        self.scroll_next = MENU_SCROLL;
        let preset = graphics
            .presets
            .get(PRESET_BADGE)
            .ok_or_else(|| MissingGraphics("palette preset 20".to_owned()))?;
        self.screen.palette.apply(preset);
        self.screen.load_tiles(
            graphics,
            &[("tiles/attract/space-loonies-logo.png", 0x0C00)],
        )?;
        clock.counter = 1;
        self.enter(SUBSTATE_BADGE, 1);
        Ok(())
    }
}

/// The badge's 62 sprites (scenes.md 2.4) with the first row at line `first_y`: 69 in the title,
/// 21 once the intermission's badge has landed.
pub(super) fn badge_sprites(first_y: i32, sprites: &mut Vec<Sprite>) {
    badge_sprites_at(first_y, 0, sprites);
}

/// The same with every sprite `dx` pixels to the right (the credits' badge is 8 to the left).
pub(super) fn badge_sprites_at(first_y: i32, dx: i32, sprites: &mut Vec<Sprite>) {
    const ROWS: [(i32, i32, u8, &[u8]); 9] = [
        (0, 109, 6, &[0; 6]),
        (8, 101, 8, &[0; 8]),
        (16, 101, 8, &[1; 8]),
        (24, 101, 8, &[1; 8]),
        (32, 101, 8, &[2; 8]),
        (40, 101, 8, &[1, 1, 3, 3, 3, 1, 1, 1]),
        (48, 101, 8, &[0, 0, 3, 3, 3, 3, 0, 0]),
        (56, 109, 6, &[3; 6]),
        (64, 125, 2, &[3; 2]),
    ];
    let mut tile = 0xC0u8;
    for (dy, x, count, palettes) in ROWS {
        for i in 0..i32::from(count) {
            sprites.push(Sprite::new(
                x + dx + 8 * i,
                first_y + dy,
                tile,
                palettes[i as usize],
            ));
            tile = tile.wrapping_add(1);
        }
    }
}
