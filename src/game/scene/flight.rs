//! The interplanetary flight (`docs/spec/scenes.md` 5): the mothership drifts across a field of
//! growing stars, a ring of eight big stars spirals in and implodes in a white flash, and the
//! screen fades to black.
//!
//! One call of [`Flight::step`] is one pass of the original's handler. The model follows the
//! spec's rules; the numbers in the comments name its sections.

use super::ppu::{Screen, Sprite};
use super::{Clock, SoundRequest};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{Graphics, MissingGraphics, PalettePreset};

const PICTURE: &str = "pictures/flight.ron";
const PRESET_BACKGROUND: usize = 1;
const PRESET_SPRITES: usize = 2;
const PRESET_FLASH: usize = 7;
const SONG: u8 = 4;
/// Loads of the flight's tiles (graphics.md 4.1, 4.2; the flight is the screen of bank 2 entry
/// $8033): the title's sprite tiles, the two big-star tiles and the mothership's background.
/// The scene's tile loads in order (scenes.md 5, "Load list"): the three sprite sets; then
/// printer message 49 lays out the mothership's picture (the nametable); then the stream of the
/// picture's own tiles.
const LOADS: [(&str, u16); 4] = [
    ("tiles/title/sprites-0800.png", 0x0800),
    ("tiles/title/sprites-0860.png", 0x0860),
    ("tiles/title/sprites-0b80.png", 0x0B80),
    ("tiles/unidentified-b/tiles.png", 0x1000),
];
/// Slots of the small stars and of the ring (5.3, 5.4), and the stars' tiles; both kinds are
/// drawn behind the mothership's picture.
const SMALL_SLOTS: usize = 22;
const BIG_SLOTS: usize = 8;
const SMALL_FIRST_TILE: u8 = 0x81;
const BIG_TOP_TILE: u8 = 0xB8;
const BIG_BOTTOM_TILE: u8 = 0xB9;
const SPRITE_BEHIND_PICTURE: bool = true;
/// Where the camera object starts and what it homes on (5.2).
const CAMERA_START: (i32, i32) = (112, 140);
const CAMERA_TARGET: (i32, i32) = (124, 126);
const CAMERA_MAX: u16 = 0x0320;
/// The ring's radius (8.8) at the start, the pass it begins to shrink in (the pass after K
/// reaches 5), how fast it grows in the explosion and its largest value (5.4).
const START_RADIUS: i32 = 160 << 8;
const SHRINK_FROM: u32 = 801;
const EXPLOSION_GROWTH: i32 = 6 << 8;
const MAX_RADIUS: i32 = 255 << 8;
const EXPLOSION_SPIN: u8 = 55;

/// What a pass of the flight did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FlightOutcome {
    Running,
    /// A was pressed or the scene ran its course.
    Ended,
}

/// One axis of an object: a 16.8 position, an 8.8 speed and a direction (true = decreasing).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Axis {
    position: i32,
    speed: u16,
    negative: bool,
}

impl Axis {
    /// "Home toward `target` with accel `a`, max `m`" and move (behaviour.md 2.1-2.3).
    fn home(&mut self, target: i32, accel: u16, max: u16) {
        let target_above = target < self.position >> 8;
        let toward = target_above == self.negative;
        if toward {
            self.speed = self.speed.saturating_add(accel);
            if self.speed >> 8 >= 3 {
                self.speed = 0x0300;
            }
        } else {
            self.speed = self.speed.saturating_sub(accel);
            if self.speed == 0 {
                self.negative = !self.negative;
            }
        }
        if self.speed >= max {
            self.speed = max;
        }
        if self.negative {
            self.position -= i32::from(self.speed);
        } else {
            self.position += i32::from(self.speed);
        }
    }
}

/// A small star (5.3).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Small {
    /// 16.8 positions; the star is gone once either leaves 0-255.
    x: i32,
    y: i32,
    right: bool,
    down: bool,
    speed_x: u16,
    speed_y: u16,
    code_x: u8,
    code_y: u8,
    build_x: u8,
    build_y: u8,
    frame: u8,
}

/// A big star of the ring (5.4).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Big {
    angle: u8,
    /// Passes of the colour-in countdown, 15 down to 0.
    fade: u8,
    /// Palette it keeps afterwards.
    final_palette: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Flight {
    screen: Screen,
    #[serde(with = "crate::game::state::flight_window")]
    window: Vec<u8>,
    #[serde(with = "crate::game::state::flight_sine")]
    sine: Vec<u8>,
    black: u8,
    flash: PalettePreset,
    sprite_palettes: PalettePreset,
    mask: u8,
    speedbase: u8,
    k: u8,
    /// The ring's radius, 8.8, and the speed it changes at.
    radius: i32,
    radius_speed: i32,
    exploding: bool,
    camera: [Axis; 2],
    small: [Option<Small>; SMALL_SLOTS],
    big: [Option<Big>; BIG_SLOTS],
    created: u8,
    /// Passes after the implosion before the radius starts to grow and the ring to spin fast.
    growth_wait: u8,
    /// Passes since the scene's start.
    passes: u32,
    song: Option<SoundRequest>,
}

impl Flight {
    /// Starts the flight: the counters are set as the scene's start does (`$5D` to 1, `$5E` to
    /// 0, `$60` to 0) and the music requested.
    pub fn new(graphics: &Graphics, clock: &mut Clock) -> Result<Flight, MissingGraphics> {
        clock.counter = (clock.counter & 0xFF_0000) | 1;
        clock.job = 0;
        let mut screen = Screen::default();
        screen.load_tiles(graphics, &LOADS[..3])?;
        let picture = graphics
            .pictures
            .get(PICTURE)
            .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?;
        screen.load_picture(picture);
        screen.load_tiles(graphics, &LOADS[3..])?;
        // The picture's right edge wraps to the left of the screen, where the console masks the
        // background (the spec's "cut off at x = 255").
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let preset = |number: usize| {
            graphics
                .presets
                .get(number)
                .cloned()
                .ok_or_else(|| MissingGraphics(format!("palette preset {number}")))
        };
        screen.palette.apply(&preset(PRESET_BACKGROUND)?);
        let sprite_palettes = preset(PRESET_SPRITES)?;
        screen.palette.apply(&sprite_palettes);
        if graphics.flight_window.len() < 12 || graphics.flight_sine.len() < 128 {
            return Err(MissingGraphics("the flight's tables".to_owned()));
        }
        let black = screen.palette.universal_background();
        let axis = |position: i32, negative: bool| Axis {
            position: position << 8,
            speed: 0,
            negative,
        };
        Ok(Flight {
            screen,
            window: graphics.flight_window.clone(),
            sine: graphics.flight_sine.clone(),
            black,
            flash: preset(PRESET_FLASH)?,
            sprite_palettes,
            mask: 15,
            speedbase: 0,
            k: 0,
            radius: START_RADIUS,
            radius_speed: 0,
            exploding: false,
            camera: [axis(CAMERA_START.0, false), axis(CAMERA_START.1, true)],
            small: [None; SMALL_SLOTS],
            big: [None; BIG_SLOTS],
            created: 0,
            growth_wait: 0,
            passes: 0,
            song: Some(SoundRequest::SongIfNew(SONG)),
        })
    }

    pub fn take_song_request(&mut self) -> Option<SoundRequest> {
        self.song.take()
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// One pass (5): A ends the scene at once; otherwise the counter goes up, the generator
    /// steps, the stars and counters and the camera move and the fade runs.
    pub fn step(&mut self, clock: &mut Clock, held: Buttons, pressed: Buttons) -> FlightOutcome {
        if pressed.has(Buttons::A) {
            return FlightOutcome::Ended;
        }
        clock.advance(held, pressed);
        let c = clock.counter as u8;
        let b = (clock.counter >> 8) as u8;
        let (r, r1) = (clock.rng.out, clock.rng.high);

        self.spawn_small(b, r, r1);
        self.move_small(c);
        self.passes = u32::from(b) * 256 + u32::from(c);
        self.ring(c);

        // Preset 2 is written on every pass, after the stars (5): the implosion's own write of
        // preset 7 over the same entries never shows.
        self.screen.palette.apply(&self.sprite_palettes);

        // The counters (5.2).
        if c == 0 {
            self.mask >>= 1;
        }
        if c & 127 == 0 {
            self.speedbase = self.speedbase.wrapping_add(1);
        }
        if b >= 2 && c & 63 == 32 {
            self.k += 1;
        }

        // The lit window (5.1).
        let mut index = usize::from(c & 31) / 2;
        if index >= 12 {
            index -= 12;
        }
        self.screen.palette.0[7] = self.window[index];

        // The camera (5.2).
        let accel = u16::from(self.k.wrapping_mul(8).wrapping_add(2));
        self.camera[0].home(CAMERA_TARGET.0, accel, CAMERA_MAX);
        self.camera[1].home(CAMERA_TARGET.1, accel, CAMERA_MAX);
        self.screen.scroll = (
            (self.camera[0].position >> 8).rem_euclid(256) as usize,
            (self.camera[1].position >> 8).rem_euclid(240) as usize,
        );

        // The fade-out and the end (5.5).
        let mut ended = false;
        if b == 5 && c & 7 == 0 {
            let entry = self.screen.palette.0[16];
            if entry == self.black && c >= 64 {
                ended = true;
            } else {
                self.screen.palette.0[16] = entry.checked_sub(16).unwrap_or(self.black);
            }
        }
        self.place_sprites();
        if ended {
            FlightOutcome::Ended
        } else {
            FlightOutcome::Running
        }
    }

    /// A new small star when the generator allows (5.3).
    fn spawn_small(&mut self, b: u8, r: u8, r1: u8) {
        if r & self.mask != 0 || b >= 5 {
            return;
        }
        let Some(slot) = self.small.iter().position(Option::is_none) else {
            return;
        };
        let across = i32::from(r1 & 31);
        let down = i32::from((r1 >> 2) & 31);
        self.small[slot] = Some(Small {
            x: (96 + across - 15) << 8,
            y: (96 + down - 15) << 8,
            right: across >= 15,
            down: down >= 15,
            speed_x: 0x0020,
            speed_y: 0x0020,
            code_x: (r1 & 7).wrapping_add(self.speedbase),
            code_y: ((r1 >> 3) & 7).wrapping_add(self.speedbase),
            build_x: 0,
            build_y: 0,
            frame: 0,
        });
    }

    fn move_small(&mut self, c: u8) {
        for slot in &mut self.small {
            let Some(star) = slot else { continue };
            star.build_x = build_up(star.build_x, star.code_x, c);
            star.build_y = build_up(star.build_y, star.code_y, c);
            let gain_x = if star.right {
                u16::from(star.build_x.wrapping_mul(2))
            } else {
                u16::from(star.build_x / 4)
            };
            star.speed_x = star.speed_x.wrapping_add(gain_x);
            star.speed_y = star.speed_y.wrapping_add(u16::from(star.build_y));
            star.frame = if star.speed_x >> 8 >= 4 || star.speed_y >> 8 >= 4 {
                4
            } else {
                (2 * (star.speed_y >> 8) + (star.speed_y >> 7 & 1)).min(4) as u8
            };
            star.x += if star.right {
                i32::from(star.speed_x)
            } else {
                -i32::from(star.speed_x)
            };
            star.y += if star.down {
                i32::from(star.speed_y)
            } else {
                -i32::from(star.speed_y)
            };
            if !(0..0x10000).contains(&star.x) || !(0..0x10000).contains(&star.y) {
                *slot = None;
            }
        }
    }

    /// The ring of big stars, the implosion and the explosion (5.4).
    fn ring(&mut self, c: u8) {
        // The ring turns first, with K as the passes before left it: the implosion's K := 55
        // comes after the rotation of its own pass (the camera, later in the pass, sees it).
        for star in self.big.iter_mut().flatten() {
            star.angle = star.angle.wrapping_add(self.k);
            star.fade = star.fade.saturating_sub(1);
        }
        if self.exploding {
            if self.growth_wait > 0 {
                self.growth_wait -= 1;
            } else {
                self.radius = (self.radius + EXPLOSION_GROWTH).min(MAX_RADIUS);
            }
        } else if self.passes >= SHRINK_FROM {
            self.radius -= self.radius_speed;
            if c & 1 == 0 {
                self.radius_speed += 1;
            }
            if self.radius < 1 << 8 {
                self.implode();
            }
        }
        if c & 63 == 0
            && let Some(slot) = self.big.iter().position(Option::is_none)
        {
            let order = self.created;
            self.created += 1;
            self.big[slot] = Some(Big {
                angle: order.wrapping_mul(32),
                fade: 15,
                final_palette: u8::from(order % 2 == 1),
            });
        }
    }

    fn implode(&mut self) {
        self.exploding = true;
        self.radius = 0;
        self.screen.show_background = false;
        self.screen.palette.apply(&self.flash);
        self.speedbase = self.speedbase.wrapping_mul(4);
        self.growth_wait = 1;
        self.k = EXPLOSION_SPIN;
    }

    fn sine_at(&self, index: usize) -> i32 {
        i32::from(self.sine.get(index).copied().unwrap_or(0))
    }

    /// (dx, dy) of a star at `angle` for the radius `r` (5.4a).
    fn offsets(&self, angle: u8, r: i32) -> (i32, i32) {
        let fold = |a: u8| {
            if a < 128 {
                usize::from(a)
            } else {
                256 - usize::from(a)
            }
        };
        let dx = self.sine_at(fold(angle)) * r / 256;
        let j = angle.wrapping_add(64);
        let dy = self.sine_at(fold(j)) * r / 256;
        (
            if angle >= 128 { -dx } else { dx },
            if j >= 128 { -dy } else { dy },
        )
    }

    fn place_sprites(&mut self) {
        let mut sprites = Vec::new();
        for star in self.small.iter().flatten() {
            let mut sprite = Sprite::new(
                star.x >> 8,
                (star.y >> 8) + 1,
                SMALL_FIRST_TILE + star.frame,
                0,
            );
            sprite.behind = SPRITE_BEHIND_PICTURE;
            sprites.push(sprite);
        }
        let radius = self.radius >> 8;
        if !(self.exploding && radius >= 255) {
            for star in self.big.iter().flatten() {
                let (dx, dy) = self.offsets(star.angle, radius);
                let cx = 96 + dx - (dx >> 2);
                let cy = 96 + dy;
                let palette = if star.fade > 0 {
                    3 - star.fade / 4
                } else {
                    star.final_palette
                };
                for (column, flip) in [(-4, false), (4, true)] {
                    for (row, tile) in [(-4, BIG_TOP_TILE), (4, BIG_BOTTOM_TILE)] {
                        let mut sprite = Sprite::new(cx + column, cy + row + 1, tile, palette);
                        sprite.behind = SPRITE_BEHIND_PICTURE;
                        sprite.flip_x = flip;
                        sprites.push(sprite);
                    }
                }
            }
        }
        self.screen.sprites = sprites;
    }
}

/// One axis's build-up value for a pass (5.3).
fn build_up(build: u8, code: u8, c: u8) -> u8 {
    if code >= 4 {
        build.wrapping_add(code / 4)
    } else {
        let mask = [15, 7, 3, 1][usize::from(code)];
        if c & mask == 0 {
            build.wrapping_add(1)
        } else {
            build
        }
    }
}
