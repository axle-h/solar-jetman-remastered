//! The game-over screen's lettering (`docs/spec/scenes.md` 12): the frozen playfield with
//! "GAME OVER" in 32 sprites that bob on one triangle wave of 64 frames. The scene lasts 240
//! frames and reads no button.

/// The clock value at which the letters appear, and the last one the scene shows.
pub const LETTERS_FROM: u32 = 15;
pub const LAST_C: u32 = 240;
/// The stream that holds the letters' 26 tiles (the original streams them into the sprite
/// tiles $00-$19 two per frame during the scene's first 13 frames).
pub const TILES: &str = "tiles/sprites/streamed-animation.png";
/// The letters' 26 tiles come in two per frame from the scene's first frame, one pass late
/// (scenes.md 12, graphics.md 4): how many of them the screen shows at clock `c`.
pub fn tiles_loaded(c: u32) -> usize {
    (2 * c.saturating_sub(1) as usize).min(26)
}

/// Sprite palette of the letters.
pub const PALETTE: u8 = 1;

/// One letter: x of its left sprite, the tiles of its top and bottom rows (left, right), and the
/// wave it follows: the phase lag in frames and the extra lines.
struct Letter {
    x: i32,
    top: [u8; 2],
    bottom: [u8; 2],
    lag: i32,
    drop: i32,
}

const LETTERS: [Letter; 8] = [
    // GAME: G and M follow W(c), A and E follow W(c - 16).
    Letter {
        x: 99,
        top: [0x00, 0x01],
        bottom: [0x02, 0x03],
        lag: 0,
        drop: 0,
    },
    Letter {
        x: 116,
        top: [0x04, 0x05],
        bottom: [0x06, 0x07],
        lag: 16,
        drop: 0,
    },
    Letter {
        x: 134,
        top: [0x08, 0x09],
        bottom: [0x0A, 0x0B],
        lag: 0,
        drop: 0,
    },
    Letter {
        x: 151,
        top: [0x0C, 0x0D],
        bottom: [0x0E, 0x0F],
        lag: 16,
        drop: 0,
    },
    // OVER, 32 lines lower: O and E follow W(c - 16), V and R follow W(c).
    Letter {
        x: 99,
        top: [0x00, 0x10],
        bottom: [0x02, 0x11],
        lag: 16,
        drop: 32,
    },
    Letter {
        x: 116,
        top: [0x12, 0x13],
        bottom: [0x14, 0x15],
        lag: 0,
        drop: 32,
    },
    Letter {
        x: 134,
        top: [0x0C, 0x0D],
        bottom: [0x0E, 0x0F],
        lag: 16,
        drop: 32,
    },
    Letter {
        x: 151,
        top: [0x16, 0x17],
        bottom: [0x18, 0x19],
        lag: 0,
        drop: 32,
    },
];

/// The triangle wave: sprite-memory y of a top row, 8 lines peak to peak.
pub fn wave(c: i32) -> i32 {
    match (c - 18).rem_euclid(64) {
        0..=3 => 59,
        4..=7 => 58,
        8..=11 => 57,
        12..=19 => 56,
        20..=23 => 57,
        24..=27 => 58,
        28..=31 => 59,
        32..=35 => 60,
        36..=39 => 61,
        40..=43 => 62,
        44..=51 => 63,
        52..=55 => 62,
        56..=59 => 61,
        _ => 60,
    }
}

/// The 32 sprites at clock `c` as (x, sprite-memory y, tile); the screen line is y + 1.
pub fn sprites(c: u32) -> Vec<(i32, i32, u8)> {
    let c = c as i32;
    let mut out = Vec::with_capacity(32);
    for letter in &LETTERS {
        let y = wave(c - letter.lag) + letter.drop;
        for (column, (top, bottom)) in letter.top.iter().zip(&letter.bottom).enumerate() {
            let x = letter.x + 8 * column as i32;
            out.push((x, y, *top));
            out.push((x, y + 8, *bottom));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiles_stream_two_a_frame_and_are_all_there_when_the_letters_show() {
        assert_eq!(
            (tiles_loaded(0), tiles_loaded(1), tiles_loaded(2)),
            (0, 0, 2)
        );
        assert_eq!((tiles_loaded(13), tiles_loaded(14)), (24, 26));
        assert_eq!(tiles_loaded(LETTERS_FROM), 26);
    }

    #[test]
    fn the_letters_bob_as_the_spec_says() {
        // W = 60 at c = 15; the wave is 8 lines peak to peak with 8-frame rests at the ends.
        assert_eq!(wave(15), 60);
        assert_eq!((wave(18 + 12), wave(18 + 44)), (56, 63));
        let at_15 = sprites(15);
        assert_eq!(at_15.len(), 32);
        // G (top left) at W(15) = 60; A (second letter) at W(-1) = 60 as well; OVER hangs 32
        // lines lower.
        assert_eq!(at_15[0], (99, 60, 0x00));
        assert_eq!(at_15[1], (99, 68, 0x02));
        assert_eq!(at_15[16], (99, wave(-1) + 32, 0x00));
        assert_eq!(at_15[17], (99, wave(-1) + 40, 0x02));
        // Period 64.
        assert_eq!(sprites(15 + 64), at_15);
    }
}
