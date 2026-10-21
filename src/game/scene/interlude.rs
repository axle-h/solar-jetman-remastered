//! The backdrop interlude and the badge intermission (`docs/spec/scenes.md` 11): a bonus flight
//! of 2,048 steps up a field of falling streaks, then the Space Loonies badge counting what was
//! collected.

use super::message::load_panel;
use super::ppu::{Screen, Sprite};
use super::{Clock, SoundRequest};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::sprites::{Piece, SpriteFile, piece_left, pod_stream};
use crate::game::graphics::{Graphics, MissingGraphics, Palette};
use crate::game::hud::Hud;

/// The backdrop's tiles: the streaks' tiles in the background table, the rocks' and mines' in
/// the sprite table (graphics.md 4.6).
const BACKDROP_BACKGROUND: (&str, u16) = ("tiles/backdrop/table-1.png", 0x1000);
const BACKDROP_SPRITES: (&str, u16) = ("tiles/backdrop/table-0.png", 0x0800);

/// Steps of the flight: the speed rises 4/256 a step for 1,024 steps and falls as many.
pub const STEPS: u32 = 2048;
/// The pod's place: x at the start, and the screen line it stays on.
const POD_X: i32 = 120;
const POD_LINE: i32 = 160;
/// The pod's x limits, and the top speed sideways (8.8).
const X_MIN: i32 = 16;
const X_MAX: i32 = 240;
const H_MAX: i32 = 0x0700;
/// The bars of the backdrop and the phases each draws in turn (scenes.md 11.1.1).
const BARS: usize = 15;
const PHASES: usize = 4;
/// Objects are kept while they are within this margin of the screen (objects.md 3.5).
const MARGIN: i32 = 128;
/// The song of the interlude (scenes.md 15).
pub const SONG: u8 = 31;
/// The effect of a gem touched, on both pulses (sound.md 8.3).
pub const GEM_SOUND: u8 = 0x1C;

/// Pod layouts of the three steering positions (objects.md 6.4).
const FRAME_UPRIGHT: u8 = 0;
const FRAME_RIGHT: u8 = 1;
const FRAME_LEFT: u8 = 0x1F;

/// Frames of the objects (table B).
const FRAME_GEM: u8 = 0x65;
const FRAME_ROCK: u8 = 0xA1;
const FRAME_MINE: u8 = 0xA5;
const FRAME_EXPLOSION: u8 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Kind {
    Gem,
    Rock,
    Mine,
    /// A mine that destroyed the pod: it follows the pod's y for 7 frames of 4 steps.
    Explosion,
    /// The original's spawn index on planet 4 reads bytes beyond the lists (Q40): objects that
    /// fill a slot and are never drawn or touched.
    Phantom,
}

/// One of the four objects (slots 11-14).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Object {
    kind: Kind,
    x: i32,
    /// World y, 16 bits with 8 fraction bits.
    y: u32,
    /// Steps since a mine destroyed the pod: 0 in the pass of the hit, which still shows the
    /// mine.
    burst: u32,
}

/// What a pass of the interlude leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InterludeOutcome {
    Running,
    /// The flight is over with this many gems collected.
    Ended {
        gems: u8,
    },
}

/// The flight.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Interlude {
    screen: Screen,
    /// The pod: x with its fraction (8.8) and the direction bit "moving left", the sideways
    /// speed (8.8, a magnitude), world y (16.8), the layout shown and whether it still lives.
    x: i32,
    left: bool,
    h: i32,
    y: u32,
    frame: u8,
    alive: bool,
    /// The upward speed (8.8) and whether it is still rising.
    v: i32,
    accelerating: bool,
    /// The vertical scroll, 0-239 with 8 fraction bits.
    scroll: i32,
    /// The bars' phases (0 = idle).
    bars: [u8; BARS],
    objects: [Option<Object>; 4],
    /// RAM $0393 of the four slots: counts the steps of a rock or mine, and is not cleared when
    /// a slot is used again.
    counts: [u8; 4],
    /// The y fractions the slots keep when freed (RAM $023E): the original gives a new object
    /// the previous one's (quirk Q17, `docs/deviations.md`).
    fractions: [u8; 4],
    /// The backdrop's index in the list 0, 4, 8, and the spawn index of the last step.
    backdrop: usize,
    last_index: Option<u8>,
    /// The first value of the spawn index's base (Q40): the original counts from the camera's
    /// high byte, which is only right where it starts at $FF; the remake counts from 0.
    index_base: Option<u8>,
    gems: u8,
    /// Gems touched since the sound was last taken (effect $1C each, sound.md 8.3).
    gem_sounds: u8,
    steps: u32,
    /// Writes the console's video buffer holds until the next pass: a row of tiles, the scroll.
    pending_row: Option<(usize, Vec<u8>)>,
    pending_scroll: Option<usize>,
    /// The sprites built in the last pass: the console's sprite transfer comes a frame later.
    pending_sprites: Vec<Sprite>,
    #[serde(with = "crate::game::state::backdrop")]
    tables: crate::game::graphics::Backdrop,
    original_rules: bool,
}

impl Interlude {
    /// Builds the flight over the gameplay of `level` (a planet's backdrop number is the
    /// position of its index in 0, 4, 8), with the pod at world y `pod_y`.
    pub fn new(
        graphics: &Graphics,
        level: usize,
        pod_type: u8,
        pod_y: u16,
        palette: &Palette,
        hud: Hud,
        original_rules: bool,
    ) -> Result<Interlude, MissingGraphics> {
        let mut screen = Screen::default();
        let mut loads = vec![(pod_stream(pod_type), 0x0010)];
        loads.extend(crate::game::gameplay::sprites::COMMON_SPRITES);
        loads.extend([BACKDROP_SPRITES, BACKDROP_BACKGROUND]);
        screen.load_tiles(graphics, &loads)?;
        let backdrop = [0, 4, 8].iter().position(|&l| l == level).unwrap_or(0);
        load_panel(&mut screen, graphics, hud)?;
        screen.palette = *palette;
        let tables = graphics.backdrop.clone();
        if tables.sprite_colours.len() >= 16 {
            // Measured: sprite palette 0 keeps the gameplay's colours (the pod's); the spec has
            // the table's 16 colours replace entries 16-31, but the pod's 18 and 19 differ.
            screen.palette.0[20..32].copy_from_slice(&tables.sprite_colours[4..16]);
        }
        if let Some(variant) = tables.variants.get(backdrop)
            && variant.len() >= 4
        {
            screen.palette.0[12..16].copy_from_slice(&variant[..4]);
        }
        // The streaks are all drawn in background palette 3 (attributes $FF).
        for index in 0..64 {
            screen.set_attribute(index, 0xFF);
        }
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen.refresh();
        Ok(Interlude {
            screen,
            x: POD_X << 8,
            left: false,
            h: 0,
            y: u32::from(pod_y) << 8,
            frame: FRAME_UPRIGHT,
            alive: true,
            v: 0,
            accelerating: true,
            scroll: 0,
            bars: [0; BARS],
            objects: [None; 4],
            counts: [0; 4],
            fractions: [0; 4],
            backdrop,
            last_index: None,
            index_base: None,
            gems: 0,
            gem_sounds: 0,
            steps: 0,
            pending_row: None,
            pending_scroll: None,
            pending_sprites: Vec::new(),
            tables,
            original_rules,
        })
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The pod's x (whole pixels).
    pub fn pod_x(&self) -> i32 {
        self.x >> 8
    }

    /// How many gems were touched since the last call.
    pub fn take_gem_sounds(&mut self) -> u8 {
        std::mem::take(&mut self.gem_sounds)
    }

    /// The sound the scene starts with.
    pub fn song(&self) -> SoundRequest {
        SoundRequest::SongIfNew(SONG)
    }

    /// What the four slots hold, for tests: 'g'em, 'r'ock, 'm'ine, 'x' explosion, 'p'hantom.
    #[cfg(test)]
    pub(super) fn slot_kinds(&self) -> String {
        self.objects
            .iter()
            .map(|o| match o.map(|o| o.kind) {
                None => '.',
                Some(Kind::Gem) => 'g',
                Some(Kind::Rock) => 'r',
                Some(Kind::Mine) => 'm',
                Some(Kind::Explosion) => 'x',
                Some(Kind::Phantom) => 'p',
            })
            .collect()
    }

    /// The camera's y: the pod stays on line 160.
    fn camera_y(&self) -> u16 {
        ((self.y >> 8) as u16).wrapping_sub(POD_LINE as u16)
    }

    /// One pass (scenes.md 11.1, "Each frame, in this order").
    pub fn step(
        &mut self,
        layouts: &SpriteFile,
        clock: &mut Clock,
        held: Buttons,
        pressed: Buttons,
    ) -> InterludeOutcome {
        // The console's video buffer is flushed at the start of the pass.
        if let Some((row, tiles)) = self.pending_row.take() {
            for (column, &tile) in tiles.iter().enumerate() {
                self.screen.set_tile(column, row, tile);
            }
        }
        if let Some(y) = self.pending_scroll.take() {
            self.screen.scroll.1 = y;
        }
        self.screen.sprites = std::mem::take(&mut self.pending_sprites);
        // 1. The generator steps first in the step; the console does it in the status-bar split
        // of the frame before, so the pass below sees the byte as the last pass left it (the
        // step is made at the end of this pass, for the next).
        // 2. The pod climbs; the world scrolls down.
        self.y = self.y.wrapping_sub(self.v as u32) & 0xFF_FFFF;
        let before = self.scroll;
        self.scroll -= self.v;
        if self.scroll < 0 {
            self.scroll += 240 << 8;
        }
        let (old, new) = ((before >> 8) as usize, (self.scroll >> 8) as usize);
        if old & 0xF8 != new & 0xF8 {
            let row = (new / 8 + 29) % 30;
            let tiles = self.backdrop_row(clock);
            self.pending_row = Some((row, tiles));
        }
        self.pending_scroll = Some(new);
        // 4. Steering, then 3. the objects: measured, the pod's touch of an object comes after
        // the steering and sees the object's place before its move of the pass.
        if self.alive {
            self.steer(held);
        }
        self.objects_step();
        // 5. Draw, then change the speed.
        self.place_sprites(layouts);
        clock.advance_split(held, pressed);
        self.steps += 1;
        if self.accelerating {
            self.v += 4;
            if self.v >> 8 >= 16 {
                self.accelerating = false;
            }
        } else {
            self.v -= 4;
            if self.v <= 0 {
                return InterludeOutcome::Ended { gems: self.gems };
            }
        }
        InterludeOutcome::Running
    }

    /// The tiles of the row the scroll just uncovered (scenes.md 11.1.1): each bar in turn draws
    /// the next phase of its streak, or starts one on a random test.
    fn backdrop_row(&mut self, clock: &mut Clock) -> Vec<u8> {
        let mut row = vec![0u8; 32];
        let t = &self.tables;
        if t.bar_widths.len() < BARS || t.bar_columns.len() < BARS || t.phase_tiles.len() < 128 {
            return row;
        }
        for bar in 0..BARS {
            let mut phase = self.bars[bar];
            if phase == 0 {
                clock.rng.out = clock.rng.out.wrapping_add(0x37);
                if clock.rng.out & 0x7F >= 23 {
                    continue;
                }
            }
            let first = usize::from(t.bar_columns[bar]);
            let width = usize::from(t.bar_widths[bar]);
            for (column, cell) in row.iter_mut().enumerate().skip(first).take(width) {
                let byte = t.phase_tiles[32 * usize::from(phase) + column];
                *cell = byte.wrapping_add(1);
            }
            phase += 1;
            self.bars[bar] = if usize::from(phase) == PHASES {
                0
            } else {
                phase
            };
        }
        row
    }

    /// Spawning, moving and removing the gems, rocks and mines, and the pod's touch of them
    /// (scenes.md 11.1.2).
    fn objects_step(&mut self) {
        let camera = self.camera_y();
        let raw = ((256 - i32::from(camera >> 8)) & 0xFF) as u8;
        // Q40: the remake takes the first value of the climb as its start (0 and 1 are the
        // original's own start on planets 0 and 8).
        let base = *self
            .index_base
            .get_or_insert(if self.original_rules { 0 } else { raw & 0xFE });
        let index = raw.wrapping_sub(base) / 2;
        if self.last_index != Some(index) {
            self.last_index = Some(index);
            self.spawn(index);
        }
        let speed = ((self.v >> 1) + (self.v >> 2)) as u32;
        for slot in 0..self.objects.len() {
            let Some(mut o) = self.objects[slot] else {
                continue;
            };
            let mut hit = false;
            if self.alive && self.touching(&o) {
                match o.kind {
                    Kind::Gem => {
                        self.gems = self.gems.wrapping_add(1);
                        self.gem_sounds += 1;
                        self.objects[slot] = None;
                        continue;
                    }
                    Kind::Rock => self.h = 0x0300 | (self.h & 0xFF),
                    // Measured: the hit pass still moves the mine and shows it (its count
                    // stays); the explosion's first picture comes in the next pass.
                    Kind::Mine => {
                        self.alive = false;
                        hit = true;
                    }
                    Kind::Explosion | Kind::Phantom => {}
                }
            }
            if o.kind == Kind::Explosion {
                // It follows the pod's y for 7 pictures of 4 steps.
                o.burst += 1;
                o.y = self.y;
                self.objects[slot] = (o.burst <= 28).then_some(o);
                if o.burst > 28 {
                    // The explosion counts its steps down in the slot's count, which stays at
                    // its reset value for the next object there (measured).
                    self.counts[slot] = 4;
                }
                continue;
            }
            if o.kind != Kind::Gem && !hit {
                self.counts[slot] = self.counts[slot].wrapping_add(1);
            }
            if o.kind == Kind::Mine && self.counts[slot] >= 48 {
                self.counts[slot] = 0;
            }
            o.y = o.y.wrapping_sub(speed) & 0xFF_FFFF;
            self.fractions[slot] = o.y as u8;
            let ry = i32::from(((o.y >> 8) as u16).wrapping_sub(camera) as i16);
            if !(-MARGIN..256 + MARGIN).contains(&ry) {
                self.objects[slot] = None;
                continue;
            }
            if hit {
                o.kind = Kind::Explosion;
                o.burst = 0;
            }
            self.objects[slot] = Some(o);
        }
    }

    /// Whether the pod touches `o`: within -12 to +11 on both axes (scenes.md 11.1.2).
    fn touching(&self, o: &Object) -> bool {
        if o.kind == Kind::Phantom {
            return false;
        }
        let pod_y = (self.y >> 8) as u16;
        let dx = (self.x >> 8) - o.x;
        let dy = i32::from(pod_y.wrapping_sub((o.y >> 8) as u16) as i16);
        (-12..12).contains(&dx) && (-12..12).contains(&dy)
    }

    fn spawn(&mut self, index: u8) {
        if usize::from(index) > 32 {
            // The original reads bytes beyond the lists here: on planet 4 three objects that
            // are never drawn or touched but fill slots (Q40; x = 3, types 5, 232, 224).
            if self.original_rules && self.backdrop == 1 && (125..=127).contains(&index) {
                self.place(Kind::Phantom, 3);
            }
            return;
        }
        let at = 32 * self.backdrop + usize::from(index);
        let (Some(&x), Some(&kind)) =
            (self.tables.spawn_x.get(at), self.tables.spawn_types.get(at))
        else {
            return;
        };
        let kind = match kind {
            1 => Kind::Gem,
            2 => Kind::Rock,
            3 => Kind::Mine,
            _ => return,
        };
        self.place(kind, i32::from(x));
    }

    /// Puts a new object just above the top edge into the first free slot (none free: the
    /// spawn is lost).
    fn place(&mut self, kind: Kind, x: i32) {
        let y = u32::from(self.camera_y().wrapping_sub(32)) << 8;
        if let Some(slot) = self.objects.iter().position(Option::is_none) {
            // Quirk Q17: the original leaves the slot's count and y fraction from the object
            // before; the remake starts every object clean.
            if !self.original_rules {
                self.counts[slot] = 0;
            }
            let stale = if self.original_rules {
                u32::from(self.fractions[slot])
            } else {
                0
            };
            self.objects[slot] = Some(Object {
                kind,
                x,
                y: y | stale,
                burst: 0,
            });
        }
    }

    /// The sideways control (scenes.md 11.1.3).
    fn steer(&mut self, held: Buttons) {
        let (left, right) = (held.has(Buttons::LEFT), held.has(Buttons::RIGHT));
        let grow = |h: i32| (h + 0x28).min(H_MAX);
        if left {
            if self.left {
                self.h = grow(self.h);
                self.frame = FRAME_LEFT;
            } else if self.h <= 0x40 {
                self.h = 0;
                self.left = true;
                self.frame = FRAME_LEFT;
            } else {
                self.h = grow(self.h - 0x40);
                self.frame = FRAME_RIGHT;
            }
        } else if right {
            if self.left {
                self.h = 0;
                self.left = false;
            } else {
                self.h = grow(self.h);
            }
            self.frame = FRAME_RIGHT;
        } else {
            self.h = (self.h - 0x60).max(0);
            self.frame = FRAME_UPRIGHT;
        }
        self.x += if self.left { -self.h } else { self.h };
        // At a limit the whole part is put on it and the speed cleared, the fraction kept.
        // Measured: the right limit counts from 240 itself, so the pod hugging the wall creeps
        // along the fraction (the spec says "crossing"; the left limit is by symmetry).
        if self.x >> 8 >= X_MAX {
            self.x = X_MAX << 8 | (self.x & 0xFF);
            self.h = 0;
        } else if self.x >> 8 < X_MIN {
            self.x = X_MIN << 8 | (self.x & 0xFF);
            self.h = 0;
        }
    }

    /// The sprites: the pod, then the objects in slot order, each first sprite in front.
    fn place_sprites(&mut self, layouts: &SpriteFile) {
        let mut sprites = Vec::new();
        let camera = self.camera_y();
        if self.alive {
            let at = (self.x >> 8, POD_LINE);
            for piece in layouts.pod_pieces(self.frame, false) {
                sprites.push(piece_sprite(&piece, at, 0));
            }
        }
        for (slot, o) in self.objects.iter().enumerate() {
            let Some(o) = o else { continue };
            if o.kind == Kind::Phantom {
                continue;
            }
            let count = u32::from(self.counts[slot]);
            let frame = match o.kind {
                Kind::Gem => FRAME_GEM,
                Kind::Rock => FRAME_ROCK + ((count / 8) % 4) as u8,
                Kind::Mine => FRAME_MINE + (count / 8).min(5) as u8,
                Kind::Phantom => continue,
                Kind::Explosion if o.burst == 0 => FRAME_MINE + (count / 8).min(5) as u8,
                Kind::Explosion => FRAME_EXPLOSION + ((o.burst - 1) / 4).min(6) as u8,
            };
            let Some(pieces) = layouts.table_b.get(usize::from(frame)) else {
                continue;
            };
            let y = i32::from(((o.y >> 8) as u16).wrapping_sub(camera) as i16);
            let stray = i32::from(self.original_rules);
            for piece in pieces {
                sprites.push(piece_sprite(piece, (o.x, y), stray));
            }
        }
        self.pending_sprites = sprites;
    }
}

/// A layout piece with the object point at `at` (one line below its y, as the console shows
/// it); `stray` is the original's drawer pixel (Q35).
fn piece_sprite(piece: &Piece, at: (i32, i32), stray: i32) -> Sprite {
    let mut sprite = Sprite::new(
        piece_left(at.0, i32::from(piece.dx), false, false) + stray,
        at.1 + i32::from(piece.dy) + 1,
        piece.tile,
        piece.attributes & 3,
    );
    sprite.behind = piece.attributes & 0x20 != 0;
    sprite.flip_x = piece.attributes & 0x40 != 0;
    sprite.flip_y = piece.attributes & 0x80 != 0;
    sprite
}
