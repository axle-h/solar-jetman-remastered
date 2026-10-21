//! The attract story (`docs/spec/scenes.md` 3): a Jetman floating over a picture that changes a
//! row at a time, with a slowly drifting scroll and 56 stars in front of nothing. It runs on
//! the story clock `c` of the spec: the first picture shows with c = 4, the last with c = 757.

use super::ppu::{Screen, Sprite};
use super::{Clock, FONT_LOADS};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{Graphics, MissingGraphics};

const PRESET: usize = 19;
/// The clock value of the first picture, and the pass that ends the scene.
const FIRST_C: u32 = 4;
const END_C: u32 = 758;
/// The picture frames' size: 11 rows of 16 tiles, 12 attribute bytes.
const FRAME_ROWS: usize = 11;
const ROW_TILES: usize = 16;
const FRAME_ATTRIBUTES: usize = 12;
/// The face loop (the frames shown, in order, one every 11 passes from c = 331) and the closing
/// pictures, each with its first row and row count (the screenshots decide: `sj-tools scene`).
const FACE_FRAMES: [usize; 3] = [1, 2, 3];
const FACE_FIRST: u32 = 331;
const FACE_PERIOD: u32 = 11;
const FACE_END: u32 = 596;
const CLOSING: [(u32, usize, usize, usize); 6] = [
    (611, 5, 0, 11),
    (622, 6, 0, 11),
    (633, 7, 0, 11),
    (644, 8, 0, 11),
    (655, 9, 0, 11),
    (666, 10, 0, 11),
];
/// The background's tile set changes at this clock value (3, "PPU control").
const TILE_SET_C: u32 = 644;

/// Star sprites (scenes.md 3): sprite-memory y, x, shape tile and palette in slot order.
const STARS: [(u8, u8, u8, u8); 56] = [
    (154, 9, 5, 1),   // slot 0
    (32, 7, 0, 1),    // slot 1
    (44, 41, 0, 1),   // slot 2
    (16, 9, 0, 0),    // slot 4
    (211, 7, 4, 1),   // slot 5
    (0, 232, 1, 1),   // slot 6
    (0, 180, 1, 1),   // slot 7
    (16, 216, 5, 0),  // slot 9
    (141, 240, 4, 0), // slot 10
    (15, 88, 1, 0),   // slot 11
    (32, 255, 3, 3),  // slot 12
    (128, 255, 1, 1), // slot 13
    (5, 32, 1, 3),    // slot 14
    (217, 93, 0, 0),  // slot 15
    (11, 94, 1, 0),   // slot 16
    (48, 0, 4, 2),    // slot 17
    (112, 133, 1, 1), // slot 18
    (185, 7, 5, 0),   // slot 19
    (32, 144, 2, 3),  // slot 22
    (24, 197, 5, 1),  // slot 23
    (169, 133, 5, 0), // slot 24
    (32, 10, 5, 0),   // slot 26
    (24, 0, 3, 3),    // slot 27
    (32, 128, 1, 2),  // slot 28
    (2, 170, 0, 1),   // slot 29
    (141, 23, 3, 0),  // slot 30
    (5, 4, 1, 3),     // slot 31
    (142, 32, 0, 0),  // slot 32
    (6, 93, 1, 3),    // slot 33
    (32, 198, 4, 3),  // slot 34
    (137, 76, 5, 0),  // slot 35
    (215, 1, 5, 1),   // slot 36
    (1, 23, 5, 1),    // slot 37
    (169, 16, 1, 1),  // slot 38
    (212, 32, 0, 0),  // slot 39
    (32, 93, 0, 0),   // slot 40
    (0, 198, 5, 0),   // slot 41
    (32, 134, 2, 0),  // slot 42
    (2, 43, 0, 0),    // slot 43
    (169, 255, 4, 0), // slot 44
    (32, 66, 0, 2),   // slot 45
    (2, 211, 5, 1),   // slot 46
    (172, 133, 2, 3), // slot 47
    (160, 208, 0, 0), // slot 48
    (36, 4, 1, 3),    // slot 50
    (32, 1, 3, 0),    // slot 51
    (114, 128, 2, 0), // slot 52
    (141, 201, 0, 0), // slot 54
    (152, 165, 0, 0), // slot 55
    (160, 240, 1, 1), // slot 57
    (130, 169, 2, 0), // slot 58
    (185, 169, 0, 2), // slot 59
    (3, 2, 5, 2),     // slot 60
    (128, 162, 3, 1), // slot 61
    (128, 160, 0, 0), // slot 62
    (169, 188, 0, 0), // slot 63
];

/// The y scroll's one-line steps up and down while c = 5 to 317 (scenes.md 3).
const RISES: &[u32] = &[
    16, 21, 25, 28, 30, 33, 35, 37, 39, 41, 43, 45, 48, 50, 53, 57, 61, 70, 152, 157, 161, 164,
    166, 169, 171, 173, 175, 177, 179, 181, 184, 186, 189, 193, 197, 206, 259, 262, 263, 265, 267,
    268, 271, 303, 306, 307, 309, 311, 312, 315,
];
const FALLS: &[u32] = &[
    76, 85, 89, 93, 96, 98, 101, 103, 105, 107, 109, 111, 113, 116, 118, 121, 125, 130, 212, 221,
    225, 229, 232, 234, 237, 239, 241, 243, 244, 246, 249, 281, 284, 285, 287, 289, 290, 293,
];
/// The 24-pass wave while c = 318 to 599 (from c = 318).
const WAVE: [u8; 24] = [
    196, 196, 196, 196, 196, 195, 194, 193, 191, 190, 189, 188, 188, 187, 187, 188, 188, 189, 190,
    191, 193, 194, 195, 196,
];
/// The closing run (c = 681 on): lines gained, then lost in steps of 1, 2 and 3.
const CLOSING_RISES: [u32; 7] = [681, 682, 684, 685, 687, 688, 691];
const FALL_BY_1: [u32; 15] = [
    701, 704, 705, 707, 708, 710, 711, 712, 713, 714, 716, 717, 719, 720, 723,
];
const FALL_BY_2: [u32; 17] = [
    715, 718, 721, 722, 724, 725, 726, 727, 728, 729, 730, 731, 732, 734, 735, 738, 741,
];
const FALL_BY_3: [u32; 5] = [733, 736, 737, 739, 740];

/// What a pass of the story did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StoryOutcome {
    Running,
    Ended,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Story {
    screen: Screen,
    /// The story clock c.
    counter: u32,
    #[serde(with = "crate::game::state::attract_nametable")]
    nametable: Vec<u8>,
    #[serde(with = "crate::game::state::attract_attributes")]
    attributes: Vec<u8>,
}

impl Story {
    pub fn new(graphics: &Graphics, clock: &mut Clock) -> Result<Story, MissingGraphics> {
        let mut screen = Screen::default();
        screen.load_tiles(graphics, &FONT_LOADS)?;
        screen.load_tiles(
            graphics,
            &[
                ("tiles/attract/jetman-table-1.png", 0x1000),
                ("tiles/attract/jetman-table-0.png", 0x0000),
                ("tiles/title/sprites-0800.png", 0x0E00),
            ],
        )?;
        screen.palette.apply(
            graphics
                .presets
                .get(PRESET)
                .ok_or_else(|| MissingGraphics("palette preset 19".to_owned()))?,
        );
        if graphics.attract_nametable.len() < 11 * FRAME_ROWS * ROW_TILES
            || graphics.attract_attributes.len() < 11 * FRAME_ATTRIBUTES
        {
            return Err(MissingGraphics(
                "the attract story's picture frames".to_owned(),
            ));
        }
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen.background_table = 1;
        screen.sprite_table = 0;
        screen.refresh();
        screen.sprites = STARS
            .iter()
            .map(|&(y, x, tile, palette)| {
                let mut sprite = Sprite::new(i32::from(x), i32::from(y) + 1, 0xE0 + tile, palette);
                sprite.behind = true;
                sprite
            })
            .collect();
        let mut story = Story {
            screen,
            counter: FIRST_C,
            nametable: graphics.attract_nametable.clone(),
            attributes: graphics.attract_attributes.clone(),
        };
        for row in 0..FRAME_ROWS {
            story.write_row(0, row);
        }
        story.screen.scroll = scroll_at(FIRST_C);
        clock.counter = FIRST_C;
        Ok(story)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The story clock.
    pub fn counter(&self) -> u32 {
        self.counter
    }

    /// Writes row `row` of picture frame `frame` and the attribute bytes that go with it.
    fn write_row(&mut self, frame: usize, row: usize) {
        let from = (frame * FRAME_ROWS + row) * ROW_TILES;
        for column in 0..ROW_TILES {
            let tile = self.nametable[from + column];
            self.screen.set_tile(column, row, tile);
        }
        self.write_attributes(frame, row / 4);
    }

    /// Writes the four attribute bytes of picture `frame` for attribute row `group`.
    fn write_attributes(&mut self, frame: usize, group: usize) {
        let from = frame * FRAME_ATTRIBUTES + 4 * group;
        let bytes = [
            self.attributes[from],
            self.attributes[from + 1],
            self.attributes[from + 2],
            self.attributes[from + 3],
        ];
        self.screen.set_attributes(group, &bytes);
    }

    /// The picture the pass at clock `c` puts up, if it changes it: (frame, first row, rows).
    fn picture_at(c: u32) -> Option<(usize, usize, usize)> {
        if (FACE_FIRST..FACE_END).contains(&c) && (c - FACE_FIRST).is_multiple_of(FACE_PERIOD) {
            let step = ((c - FACE_FIRST) / FACE_PERIOD) as usize;
            return Some((FACE_FRAMES[step % 3], 3, 3));
        }
        CLOSING
            .iter()
            .find(|&&(first, ..)| first == c)
            .map(|&(_, frame, row, rows)| (frame, row, rows))
    }

    /// One pass: the clock goes up, the picture's row and the scroll follow it.
    pub fn step(&mut self, clock: &mut Clock, pressed: Buttons) -> StoryOutcome {
        if pressed.0 != 0 {
            return StoryOutcome::Ended;
        }
        self.counter += 1;
        clock.counter = self.counter;
        let c = self.counter;
        if c >= END_C {
            return StoryOutcome::Ended;
        }
        // The spec's write times are those of the video-memory log, one pass after the state
        // they belong to is on the screen.
        if let Some((frame, row, rows)) = Self::picture_at(c) {
            for row in row..row + rows {
                self.write_row(frame, row);
            }
            // The attribute bytes of the rows around it change with the picture.
            for group in 0..3 {
                self.write_attributes(frame, group);
            }
        }
        if c == TILE_SET_C {
            self.screen.background_table = 0;
            self.screen.refresh();
        }
        self.screen.scroll = scroll_at(c);
        StoryOutcome::Running
    }
}

/// The scroll at clock `c` (scenes.md 3, "Scroll").
pub fn scroll_at(c: u32) -> (usize, usize) {
    let x = if c <= FIRST_C {
        0
    } else if c <= 240 {
        -2 * (c as i32 - 4)
    } else {
        -2 * 236 - (c.min(600) as i32 - 240)
    };
    (x.rem_euclid(256) as usize, usize::from(y_at(c)))
}

/// The y scroll at clock `c` (scenes.md 3).
fn y_at(c: u32) -> u8 {
    let mut y: i32 = 184;
    for n in 5..=c.min(317) {
        y += i32::from(RISES.contains(&n)) - i32::from(FALLS.contains(&n));
    }
    if c >= 318 {
        y = i32::from(WAVE[((c.min(599) - 318) % 24) as usize]);
    }
    if c >= 600 {
        y = 190;
    }
    for n in 681..=c.min(758) {
        y += i32::from(CLOSING_RISES.contains(&n));
        y -= i32::from(FALL_BY_1.contains(&n));
        y -= 2 * i32::from(FALL_BY_2.contains(&n));
        y -= 3 * i32::from(FALL_BY_3.contains(&n) || n >= 742);
    }
    y as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scroll_follows_the_spec_at_its_known_points() {
        // x: 0 at c = 4, -2 a pass to c = 240, -1 to c = 600, then 192.
        let x = |c| scroll_at(c).0;
        assert_eq!((x(4), x(5), x(6)), (0, 254, 252));
        assert_eq!(x(240), 40);
        assert_eq!((x(600), x(601), x(700)), (192, 192, 192));
        // y: 184 at c = 4, 196 at the end of the first phase, 190 from c = 600, and the closing
        // fall (133 at c = 741, 85 at c = 757).
        let y = |c| scroll_at(c).1;
        assert_eq!((y(4), y(317), y(600), y(680)), (184, 196, 190, 190));
        assert_eq!((y(697), y(741), y(757)), (197, 133, 85));
    }
}
