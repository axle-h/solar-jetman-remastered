//! The launch scene (`docs/spec/scenes.md` 9): inside the mothership the pilot walks along the
//! gantry to the pod, climbs in, the hatch closes and the pod flies up out of the top of the
//! screen. One call of [`Launch::step`] is one pass of the original's handler.

use super::Clock;
use super::ppu::{Screen, Sprite};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::sprites::piece_left;
use crate::game::gameplay::sprites::{Piece, SpriteFile};
use crate::game::graphics::{Graphics, MissingGraphics};

const PICTURE: &str = "pictures/launch.ron";
const PRESET: usize = 27;
/// The pod's own tiles by pod type (graphics.md 4.3); type 3 would use the set of type 1.
const POD_STREAMS: [&str; 3] = [
    "tiles/launch/pod-type-0.png",
    "tiles/launch/pod-type-1.png",
    "tiles/launch/pod-type-2.png",
];
const BACKGROUND: (&str, u16) = ("tiles/launch/background.png", 0x1000);
const SPRITES: (&str, u16) = ("tiles/launch/sprites.png", 0x0170);

/// Where the pod and hatch start (9) and the frames of the objects.
const POD_AT: (i32, i32) = (67, 50);
/// The flame stays where it starts, at the mothership's exhaust, while the pod rises (9).
const FLAME_AT: (i32, i32) = (67, 34);
const BODY_FRAME: u8 = 0xC1;
const HATCH_FRAME: u8 = 0xC2;
const FLAME_FRAME: u8 = 0xC6;
const FIRST_PILOT_PASS: u32 = 5;
const PASSES_PER_STEP: u32 = 6;
/// The pilot's walk: x, y and frame for each of the 21 steps (9).
const WALK_X: [u8; 21] = [
    143, 147, 143, 139, 135, 131, 127, 123, 119, 115, 111, 107, 103, 98, 68, 64, 64, 64, 64, 64, 64,
];
const WALK_Y: [u8; 21] = [
    63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 58, 58, 36, 40, 40, 40, 40, 40, 40,
];
const WALK_FRAME: [u8; 21] = [
    0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4,
    0xD5, 0xD6, 0xD7, 0xD8, 0xD9,
];
/// The pass the pilot is removed in and the pod starts to rise.
const FLIGHT_PASS: u32 = 131;

/// What a pass of the launch did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LaunchOutcome {
    Running,
    Ended,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Launch {
    screen: Screen,
    pass: u32,
    /// The pod's y as a 16.8 value and its rising speed (8.8).
    pod_y: i32,
    speed: u32,
    pilot: Option<(i32, i32, u8)>,
    hatch_frame: u8,
    flame_frame: Option<u8>,
    /// Quirk Q35: the original's drawer puts every layout one pixel right.
    pub original_rules: bool,
}

impl Launch {
    pub fn new(graphics: &Graphics, pod_type: u8) -> Result<Launch, MissingGraphics> {
        let mut screen = Screen::default();
        let pod = POD_STREAMS[match pod_type & 3 {
            3 => 1,
            other => usize::from(other),
        }];
        screen.load_tiles(graphics, &[(pod, 0x0000), SPRITES, BACKGROUND])?;
        let picture = graphics
            .pictures
            .get(PICTURE)
            .ok_or_else(|| MissingGraphics(PICTURE.to_owned()))?;
        screen.load_picture(picture);
        // The console masks the background in the leftmost 8 pixels here too.
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen.palette.apply(
            graphics
                .presets
                .get(PRESET)
                .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET}")))?,
        );
        Ok(Launch {
            screen,
            pass: 0,
            pod_y: POD_AT.1 << 8,
            speed: 0,
            pilot: None,
            hatch_frame: HATCH_FRAME,
            flame_frame: None,
            original_rules: false,
        })
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Draws the first picture, before the first pass.
    pub fn show(&mut self, layouts: &SpriteFile) {
        self.place_sprites(layouts);
    }

    /// One pass (9): any newly pressed button ends the scene at once; the frame counter goes
    /// up by one and the generator is not stepped.
    pub fn step(
        &mut self,
        layouts: &SpriteFile,
        clock: &mut Clock,
        pressed: Buttons,
    ) -> LaunchOutcome {
        if pressed.0 != 0 {
            return LaunchOutcome::Ended;
        }
        // The counter's value before this pass's increment (scenes.md 9: "those with $5D odd").
        let counter = clock.counter;
        clock.counter = (counter + 1) & 0xFF_FFFF;
        let pass = self.pass;
        self.pass += 1;
        if (FIRST_PILOT_PASS..FLIGHT_PASS).contains(&pass)
            && (pass - FIRST_PILOT_PASS).is_multiple_of(PASSES_PER_STEP)
        {
            let step = ((pass - FIRST_PILOT_PASS) / PASSES_PER_STEP) as usize;
            self.pilot = Some((
                i32::from(WALK_X[step]),
                i32::from(WALK_Y[step]),
                WALK_FRAME[step],
            ));
            if (18..=20).contains(&step) {
                self.hatch_frame = HATCH_FRAME + (step as u8 - 17);
            }
        }
        let mut ended = false;
        if pass >= FLIGHT_PASS {
            // The pod's mover and the flame start with the pilot's removal, in pass 131, but its
            // speed is still 0 there: it rises on the passes whose $5D, once incremented, is odd
            // (the counter read here is the value before the increment), so y first changes in
            // pass 132 when the scene starts on an even counter (scenes.md 9, phase 2). Started
            // on an odd counter, the whole rise comes one pass earlier.
            self.pilot = None;
            if counter & 1 == 0 {
                self.speed += 1;
            }
            self.pod_y -= self.speed as i32;
            let y = self.pod_y >> 8;
            // The flame's frame follows bit 2 of the counter as the frame before left it.
            self.flame_frame = Some(if y > 9 {
                FLAME_FRAME + ((counter.wrapping_sub(1) >> 2) & 1) as u8
            } else {
                FLAME_FRAME + ((9 - y) / 2) as u8
            });
            // The scene ends in the first pass in which the pod's y (16 bits and fraction) goes
            // below 0. The picture stops changing two passes earlier, when the whole part
            // reaches 0, so these are repeated frames (scenes.md 9).
            ended = self.pod_y < 0;
        }
        self.place_sprites(layouts);
        if ended {
            LaunchOutcome::Ended
        } else {
            LaunchOutcome::Running
        }
    }

    fn place_sprites(&mut self, layouts: &SpriteFile) {
        let mut sprites = Vec::new();
        let pod = (POD_AT.0, self.pod_y >> 8);
        // Slots 11 to 14, each object's sprites in layout order: the first in front.
        let original = self.original_rules;
        let mut object = |frame: u8, at: (i32, i32)| {
            let Some(pieces) = layouts.table_b.get(usize::from(frame)) else {
                return;
            };
            for piece in pieces {
                sprites.push(piece_sprite(piece, at, original));
            }
        };
        if let Some((x, y, frame)) = self.pilot {
            object(frame, (x, y));
        }
        if let Some(frame) = self.flame_frame {
            object(frame, (FLAME_AT.0, FLAME_AT.1));
        }
        object(self.hatch_frame, pod);
        object(BODY_FRAME, pod);
        self.screen.sprites = sprites;
    }
}

/// A layout piece placed with its object point at `at`. The console shows a sprite one line
/// below its Y (objects.md 6.2), and in this scene one pixel right of the x the spec gives (the
/// original's sprites sit at the spec's x + 1; measured on the hatch and the pilot).
fn piece_sprite(piece: &Piece, at: (i32, i32), original: bool) -> Sprite {
    let mut sprite = Sprite::new(
        piece_left(at.0, i32::from(piece.dx), false, original),
        at.1 + i32::from(piece.dy) + 1,
        piece.tile,
        piece.attributes & 3,
    );
    sprite.behind = piece.attributes & 0x20 != 0;
    sprite.flip_x = piece.attributes & 0x40 != 0;
    sprite.flip_y = piece.attributes & 0x80 != 0;
    sprite
}
