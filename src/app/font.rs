//! The port's own small bitmap font (5×7 cells, upper case only) and a plain RGB surface to
//! draw it on. Used by the port menu and the message screens, so it works before any import.

/// Width of a glyph (7 rows high), and the pitch text is laid out at.
pub const GLYPH_W: usize = 5;
pub const PITCH_X: usize = 6;
pub const PITCH_Y: usize = 9;

/// Glyph rows, top to bottom; bit 4 is the leftmost column.
const GLYPHS: &[(char, [u8; 7])] = &[
    (
        'A',
        [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
    ),
    (
        'B',
        [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
    ),
    (
        'C',
        [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
    ),
    (
        'D',
        [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
    ),
    (
        'E',
        [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
    ),
    (
        'F',
        [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
    ),
    (
        'G',
        [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
    ),
    (
        'H',
        [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
    ),
    (
        'I',
        [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
    ),
    (
        'J',
        [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
    ),
    (
        'K',
        [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
    ),
    (
        'L',
        [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
    ),
    (
        'M',
        [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
    ),
    (
        'N',
        [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
    ),
    (
        'O',
        [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
    ),
    (
        'P',
        [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
    ),
    (
        'Q',
        [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
    ),
    (
        'R',
        [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
    ),
    (
        'S',
        [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
    ),
    (
        'T',
        [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
    ),
    (
        'U',
        [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
    ),
    (
        'V',
        [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
    ),
    (
        'W',
        [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
    ),
    (
        'X',
        [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
    ),
    (
        'Y',
        [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
    ),
    (
        'Z',
        [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
    ),
    (
        '0',
        [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
    ),
    (
        '1',
        [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
    ),
    (
        '2',
        [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
    ),
    (
        '3',
        [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
    ),
    (
        '4',
        [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
    ),
    (
        '5',
        [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
    ),
    (
        '6',
        [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
    ),
    (
        '7',
        [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
    ),
    (
        '8',
        [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
    ),
    (
        '9',
        [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
    ),
    ('.', [0, 0, 0, 0, 0, 0b01100, 0b01100]),
    (',', [0, 0, 0, 0, 0b01100, 0b00100, 0b01000]),
    (':', [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0]),
    (';', [0, 0b01100, 0b01100, 0, 0b01100, 0b00100, 0b01000]),
    (
        '!',
        [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
    ),
    (
        '?',
        [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
    ),
    ('-', [0, 0, 0, 0b11111, 0, 0, 0]),
    ('+', [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0]),
    ('=', [0, 0, 0b11111, 0, 0b11111, 0, 0]),
    (
        '/',
        [
            0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000,
        ],
    ),
    (
        '\\',
        [
            0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001,
        ],
    ),
    (
        '(',
        [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
    ),
    (
        ')',
        [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
    ),
    (
        '[',
        [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
    ),
    (
        ']',
        [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
    ),
    (
        '<',
        [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
    ),
    (
        '>',
        [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
    ),
    ('_', [0, 0, 0, 0, 0, 0, 0b11111]),
    ('\'', [0b00100, 0b00100, 0b01000, 0, 0, 0, 0]),
    ('"', [0b01010, 0b01010, 0, 0, 0, 0, 0]),
    (
        '%',
        [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
    ),
    (
        '#',
        [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
    ),
    ('*', [0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0]),
];

/// Rows of a character's glyph; lower case draws as upper case, unknown characters as `?`,
/// space as blank.
pub fn glyph(c: char) -> [u8; 7] {
    let c = c.to_ascii_uppercase();
    if c == ' ' {
        return [0; 7];
    }
    let find = |c| GLYPHS.iter().find(|(g, _)| *g == c).map(|(_, rows)| *rows);
    find(c).or_else(|| find('?')).unwrap_or([0; 7])
}

/// Width of `text` in pixels.
pub fn text_width(text: &str) -> usize {
    text.chars().count() * PITCH_X
}

/// A packed RGB24 pixel buffer to draw on.
pub struct Surface<'a> {
    pub pixels: &'a mut [u8],
    pub width: usize,
    pub height: usize,
}

pub type Rgb = [u8; 3];

impl Surface<'_> {
    pub fn put(&mut self, x: usize, y: usize, colour: Rgb) {
        if x < self.width && y < self.height {
            let at = (y * self.width + x) * 3;
            self.pixels[at..at + 3].copy_from_slice(&colour);
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, colour: Rgb) {
        for row in y..(y + h).min(self.height) {
            for column in x..(x + w).min(self.width) {
                self.put(column, row, colour);
            }
        }
    }

    /// Darkens the whole surface to a quarter of its brightness.
    pub fn dim(&mut self) {
        for byte in self.pixels.iter_mut() {
            *byte >>= 2;
        }
    }

    /// Draws `text` with its top left corner at (`x`, `y`).
    pub fn text(&mut self, x: usize, y: usize, text: &str, colour: Rgb) {
        for (index, c) in text.chars().enumerate() {
            let left = x + index * PITCH_X;
            for (row, bits) in glyph(c).into_iter().enumerate() {
                for column in 0..GLYPH_W {
                    if bits & (0b10000 >> column) != 0 {
                        self.put(left + column, y + row, colour);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_printable_ascii_character_has_a_glyph() {
        for c in ' '..='~' {
            let blank = glyph(c) == [0; 7];
            assert_eq!(blank, c == ' ', "{c:?}");
        }
        // Lower case is the upper case glyph.
        assert_eq!(glyph('a'), glyph('A'));
    }

    #[test]
    fn glyphs_are_distinct_and_fit_five_columns() {
        for (i, (a, rows)) in GLYPHS.iter().enumerate() {
            assert!(rows.iter().all(|r| *r < 32), "{a}");
            for (b, other) in &GLYPHS[i + 1..] {
                assert_ne!(rows, other, "{a} and {b}");
            }
        }
    }

    #[test]
    fn text_lands_where_expected() {
        let mut pixels = vec![0; 16 * 16 * 3];
        let mut surface = Surface {
            pixels: &mut pixels,
            width: 16,
            height: 16,
        };
        surface.text(2, 3, "I", [9, 9, 9]);
        // The top row of "I" is 01110: columns 1..=3.
        assert_eq!(&pixels[((3 * 16) + 2) * 3..][..3], [0, 0, 0]);
        assert_eq!(&pixels[((3 * 16) + 3) * 3..][..3], [9, 9, 9]);
        assert_eq!(&pixels[((3 * 16) + 6) * 3..][..3], [0, 0, 0]);
    }
}
