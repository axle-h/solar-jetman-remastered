//! The ending picture (`docs/spec/scenes.md` 14.2): Jetman with his feet up in the warpship's
//! cabin, a small creature on the console that jumps six times, and a star in the window that
//! grows into a sun. 902 frames, no input. Song 32 plays on from the final message. In the
//! remake any button ends it and the credits (`Game::set_skip_scenes`).

use super::Clock;
use super::ppu::{Screen, Sprite};
use crate::game::graphics::{Graphics, MissingGraphics};

const PRESET: usize = 22;
const PICTURE: &str = "pictures/ending.ron";
const LOADS: [(&str, u16); 4] = [
    ("tiles/ending/tiles-1000.png", 0x1000),
    ("tiles/ending/tiles-1dc0.png", 0x1DC0),
    ("tiles/ending/tiles-0000.png", 0x0000),
    ("tiles/ending/tiles-0d60.png", 0x0D60),
];
/// Screen line from which the background uses pattern table 0 (the sprite-0 split).
const SPLIT_LINE: usize = 136;

/// The pass on which the scene is over (the credits start in it): the 902 visible frames
/// before it, plus the fourth flash step that finds the screen done.
const END_PASS: u32 = 902;
/// The creature rests at this y (8.8) and jumps six times, the n-th at speed n.
const REST_Y: i32 = 141 << 8;
const JUMPS: i32 = 6;
const GRAVITY: i32 = 0x2C;
const FALL_CAP: i32 = 0x300;
/// Passes between a landing and the next jump's first move.
const REST_PASSES: u32 = 16;
/// Idle ticks (every 4th counter value) before the first jump, and the pose of each 16.
const IDLE_TICKS: u32 = 112;
const POSES: [Pose; 7] = [
    Pose::Rest,
    Pose::Rest,
    Pose::TurnedA,
    Pose::TurnedB,
    Pose::TurnedA,
    Pose::Rest,
    Pose::Rest,
];
/// The creature's columns and the screen line of its top row at rest minus its y (7 above).
const CREATURE_X: [i32; 2] = [184, 192];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Pose {
    Rest,
    TurnedA,
    TurnedB,
    Air,
}

impl Pose {
    /// Tiles of the three rows, left then right.
    fn tiles(self) -> [[u8; 2]; 3] {
        match self {
            Pose::Rest => [[0xDC, 0xDD], [0xDE, 0xDF], [0xE0, 0xE1]],
            Pose::TurnedA => [[0xE8, 0xE9], [0xEA, 0xEB], [0xE0, 0xE1]],
            Pose::TurnedB => [[0xE4, 0xE5], [0xE6, 0xE7], [0xE0, 0xE1]],
            Pose::Air => [[0xDC, 0xDD], [0xDE, 0xDF], [0xE2, 0xE3]],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Creature {
    /// Waiting for idle ticks, `ticks` counted.
    Idle {
        ticks: u32,
    },
    /// Resting `left` more passes before jump number `next` (1-6).
    Resting {
        left: u32,
        next: i32,
    },
    /// In the air: speed (8.8, a magnitude), rising or falling.
    Jumping {
        n: i32,
        speed: i32,
        rising: bool,
    },
    Done,
}

/// The face's cells (row, first column, tiles) in each state (scenes.md 14.2).
const BLINK: [(usize, usize, &[u8]); 2] =
    [(6, 3, &[0xDC, 0xDD, 0xDE]), (7, 3, &[0xDF, 0xE0, 0xE1])];
const FACE_A: [(usize, usize, &[u8]); 5] = [
    (6, 3, &[0xE2, 0xE3, 0xDE]),
    (7, 3, &[0xE4, 0xE5, 0xE6]),
    (8, 3, &[0xE7, 0xE8, 0xE9]),
    (9, 2, &[0xEA, 0xEB, 0xEC, 0xED]),
    (10, 4, &[0xEE, 0xEF]),
];
const FACE_B: [(usize, usize, &[u8]); 6] = [
    (6, 3, &[0xF0, 0xF1, 0xF2]),
    (7, 3, &[0xF3, 0xF4, 0xF5]),
    (8, 3, &[0xF6]),
    (8, 5, &[0xF7]),
    (9, 3, &[0xF8, 0xF9, 0xFA]),
    (10, 3, &[0xFB, 0xFC, 0xFD]),
];
const FACE_B_LOWER: (usize, usize, &[u8]) = (11, 4, &[0xFE, 0xFF]);
/// The console button after the last landing: rows 18-20, columns 23-24.
const BUTTON: [(usize, usize, &[u8]); 3] = [
    (18, 23, &[0xD6, 0xD7]),
    (19, 23, &[0xD8, 0xD9]),
    (20, 23, &[0xDA, 0xDB]),
];

/// The star's sprites per copy, for the three big frames (scenes.md 14.2): x, y, tile.
const STAR_12: [(i32, i32, u8); 3] = [(208, 32, 0xF3), (200, 40, 0xF4), (208, 40, 0xFF)];
const STAR_16: [(i32, i32, u8); 4] = [
    (200, 32, 0xF5),
    (208, 32, 0xF6),
    (200, 40, 0xF7),
    (208, 40, 0xFF),
];
const STAR_36: [(i32, i32, u8); 9] = [
    (192, 24, 0xF8),
    (200, 24, 0xF9),
    (208, 24, 0xFA),
    (192, 32, 0xFB),
    (200, 32, 0xFF),
    (208, 32, 0xFF),
    (192, 40, 0xFC),
    (200, 40, 0xFF),
    (208, 40, 0xFF),
];
/// The sun's yellow, which the disc's palette-2 pixels take in three steps.
const SUN: u8 = 0x27;

/// What a pass of the ending leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EndingOutcome {
    Running,
    /// The picture is over: the credits follow.
    Ended,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Ending {
    screen: Screen,
    pass: u32,
    creature: Creature,
    /// The creature's y (8.8) and its pose.
    y: i32,
    pose: Pose,
    /// The idle poses count ticks; the face is the base one, blinking, then A and B.
    blinking: bool,
    face_changed: bool,
    /// Where the star is: steps taken since the last landing's slot, and its tile.
    star_step: Option<u32>,
    star: Vec<(i32, i32, u8)>,
    /// The base cells of the blink, kept to put back.
    base: [Vec<u8>; 2],
    /// Sprites built in the last pass: the console's transfer comes a pass later.
    pending_sprites: Vec<Sprite>,
}

impl Ending {
    /// Builds the picture: the first frame, in which nothing has moved yet.
    pub fn new(graphics: &Graphics) -> Result<Ending, MissingGraphics> {
        let mut screen = Screen::default();
        screen.load_tiles(graphics, &LOADS)?;
        screen.split = Some((SPLIT_LINE, 0));
        screen.refresh();
        screen.load_picture(
            graphics
                .pictures
                .get(PICTURE)
                .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?,
        );
        screen.palette.apply(
            graphics
                .presets
                .get(PRESET)
                .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET}")))?,
        );
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        let base = BLINK.map(|(row, column, tiles)| {
            (0..tiles.len())
                .map(|i| screen.tile(column + i, row))
                .collect()
        });
        let mut ending = Ending {
            screen,
            pass: 0,
            creature: Creature::Idle { ticks: 0 },
            y: REST_Y,
            pose: Pose::Rest,
            blinking: false,
            face_changed: false,
            star_step: None,
            star: vec![(208, 40, 0xEC)],
            base,
            pending_sprites: Vec::new(),
        };
        ending.place_sprites();
        Ok(ending)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// One pass. The generator and the frame counter step first (the pass after the loading
    /// frames is number 1).
    pub fn step(&mut self, clock: &mut Clock) -> EndingOutcome {
        self.screen.sprites = std::mem::take(&mut self.pending_sprites);
        self.pass += 1;
        // The generator steps with the counters as the last pass left them, and the job
        // counter stands still (measured against the generator's bytes, which the blink reads).
        let before = clock.counter;
        let seen = clock.rng.out;
        clock.rng.step([
            0,
            0,
            0,
            0,
            before as u8,
            (before >> 8) as u8,
            (before >> 16) as u8,
            0,
        ]);
        // Only the low byte counts: the carry into $5E does not happen here.
        clock.counter = before & !0xFF | (before + 1) & 0xFF;
        let counter = clock.counter;
        // The events fall on multiples of the counter as the pass before this one left it.
        let c = counter.wrapping_sub(1);
        self.blink(seen, c);
        self.star_slot(c);
        self.creature_step(c);
        self.place_sprites();
        if self.pass >= END_PASS && counter & 7 == 0 {
            return EndingOutcome::Ended;
        }
        EndingOutcome::Running
    }

    fn write(&mut self, cells: &[(usize, usize, &[u8])]) {
        for &(row, column, tiles) in cells {
            for (i, &tile) in tiles.iter().enumerate() {
                self.screen.set_tile(column + i, row, tile);
            }
        }
    }

    /// The blink: where the counter is not a multiple of 64 the random byte (as the last pass
    /// left it) is tested, and at a multiple of 64 the base face comes back.
    fn blink(&mut self, random: u8, c: u32) {
        if self.face_changed {
            return;
        }
        if c & 63 == 0 {
            if self.blinking {
                self.blinking = false;
                for (k, (row, column, _)) in BLINK.iter().enumerate() {
                    for (i, &tile) in self.base[k].iter().enumerate() {
                        self.screen.set_tile(column + i, *row, tile);
                    }
                }
            }
        } else if random == 0x0E && !self.blinking {
            self.blinking = true;
            self.write(&BLINK);
        }
    }

    fn creature_step(&mut self, c: u32) {
        match self.creature {
            Creature::Idle { ticks } => {
                if c & 3 == 0 {
                    let ticks = ticks + 1;
                    if ticks >= IDLE_TICKS {
                        self.start_jump(1);
                    } else {
                        self.pose = POSES[(ticks / 16) as usize];
                        self.creature = Creature::Idle { ticks };
                    }
                }
            }
            Creature::Resting { left, next } => {
                if left > 0 {
                    self.creature = Creature::Resting {
                        left: left - 1,
                        next,
                    };
                } else {
                    // A later jump moves in the pass that changes the pose.
                    self.start_jump(next);
                    self.jump_move();
                }
            }
            Creature::Jumping { .. } => self.jump_move(),
            Creature::Done => {}
        }
    }

    /// The creature leaves the ground at speed `n`: its pose changes now (the first jump's first
    /// move comes with the next pass).
    fn start_jump(&mut self, n: i32) {
        self.pose = Pose::Air;
        self.creature = Creature::Jumping {
            n,
            speed: n << 8,
            rising: true,
        };
    }

    /// One frame of a jump (scenes.md 14.2, "Creature jump physics").
    fn jump_move(&mut self) {
        let Creature::Jumping { n, speed, rising } = self.creature else {
            return;
        };
        if rising {
            self.y -= speed;
            let speed = speed - GRAVITY;
            self.creature = if speed <= 0 {
                Creature::Jumping {
                    n,
                    speed: 0,
                    rising: false,
                }
            } else {
                Creature::Jumping { n, speed, rising }
            };
        } else {
            self.y += speed;
            if self.y >= REST_Y {
                // The whole part is put back on the ground, the fraction stays (measured:
                // the six jumps' pixels follow from it).
                self.y = REST_Y | (self.y & 0xFF);
                self.pose = Pose::Rest;
                self.creature = if n == JUMPS {
                    // The button on the console changes with the last landing.
                    self.write(&BUTTON);
                    self.star_step = Some(0);
                    Creature::Done
                } else {
                    Creature::Resting {
                        left: REST_PASSES,
                        next: n + 1,
                    }
                };
            } else {
                self.creature = Creature::Jumping {
                    n,
                    speed: (speed + GRAVITY).min(FALL_CAP),
                    rising: false,
                };
            }
        }
    }

    /// The star's steps come every 8th counter value once the creature is done: face A, six
    /// frames of one sprite, three bigger ones, then the sun's three palette steps.
    fn star_slot(&mut self, c: u32) {
        let Some(step) = self.star_step else { return };
        if c & 7 != 0 || self.pass == 0 {
            return;
        }
        match step {
            0 => {
                self.face_changed = true;
                self.write(&FACE_A);
            }
            1..=6 => self.star = vec![(208, 40, 0xEC + step as u8)],
            7 => self.star = STAR_12.to_vec(),
            8 => self.star = STAR_16.to_vec(),
            9 => self.star = STAR_36.to_vec(),
            10..=12 => {
                self.star.clear();
                self.screen.palette.0[(step - 1) as usize] = SUN;
            }
            _ => {}
        }
        if step == 5 {
            self.write(&FACE_B);
            self.write(&[FACE_B_LOWER]);
        }
        self.star_step = Some(step + 1);
    }

    /// The creature's six sprites and the star's four copies of each of its sprites.
    fn place_sprites(&mut self) {
        let mut sprites = Vec::new();
        let top = (self.y >> 8) - 7;
        for (row, tiles) in self.pose.tiles().iter().enumerate() {
            for (column, &tile) in tiles.iter().enumerate() {
                sprites.push(Sprite::new(
                    CREATURE_X[column],
                    top + 8 * row as i32,
                    tile,
                    3,
                ));
            }
        }
        for &(x, y, tile) in &self.star {
            for (flip_x, flip_y) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut s = Sprite::new(
                    if flip_x { 424 - x } else { x },
                    if flip_y { 88 - y } else { y },
                    tile,
                    0,
                );
                s.flip_x = flip_x;
                s.flip_y = flip_y;
                sprites.push(s);
            }
        }
        self.pending_sprites = sprites;
    }
}
