//! Decoders for the storage formats of the original's graphics (`docs/spec/graphics.md`,
//! sections 1 and 2). They work on byte slices and know nothing about where the data lives.

/// Bytes per 8×8 tile in the NES 2-bit-per-pixel format.
pub const TILE_BYTES: usize = 16;

/// Bytes per glyph of the 1-bit fonts.
pub const GLYPH_BYTES: usize = 7;

/// The 64 pixel values (0-3) of a tile, row by row, left to right.
pub fn tile_pixels(tile: &[u8; TILE_BYTES]) -> [u8; 64] {
    let mut pixels = [0u8; 64];
    for row in 0..8 {
        let low = tile[row];
        let high = tile[row + 8];
        for column in 0..8 {
            let bit = 7 - column;
            pixels[row * 8 + column] = (low >> bit & 1) | (high >> bit & 1) << 1;
        }
    }
    pixels
}

/// The inverse of [`tile_pixels`]: packs 64 pixel values (only their low two bits count) into
/// the 16 bytes of a tile.
pub fn tile_bytes(pixels: &[u8; 64]) -> [u8; TILE_BYTES] {
    let mut tile = [0u8; TILE_BYTES];
    for row in 0..8 {
        for column in 0..8 {
            let value = pixels[row * 8 + column];
            let bit = 7 - column;
            tile[row] |= (value & 1) << bit;
            tile[row + 8] |= (value >> 1 & 1) << bit;
        }
    }
    tile
}

/// Raw tiles stored XOR-ed with a mask byte (section 2.5; with mask 0 these are plain raw
/// tiles, section 2.1).
pub fn unmask(data: &[u8], mask: u8) -> Vec<u8> {
    data.iter().map(|&byte| byte ^ mask).collect()
}

/// Expands 1-bit font glyphs (section 2.4) into tiles: each 7-byte glyph becomes one 16-byte
/// tile whose plane-0 row 0 is `mask`, rows 1-7 the glyph bytes XOR `mask`, and plane 1 zero.
/// Trailing bytes that do not make a whole glyph are ignored.
pub fn font_tiles(glyphs: &[u8], mask: u8) -> Vec<u8> {
    let mut tiles = Vec::with_capacity(glyphs.len() / GLYPH_BYTES * TILE_BYTES);
    for glyph in glyphs.as_chunks::<GLYPH_BYTES>().0 {
        tiles.push(mask);
        tiles.extend(glyph.iter().map(|&row| row ^ mask));
        tiles.extend([0u8; 8]);
    }
    tiles
}

/// Decodes a run-length stream (section 2.3). `stream` is exactly the `L` bytes of the
/// stream; `bias` is added (mod 256) to every output byte.
///
/// Blocks start with a control byte `c`: below $80 it is followed by `c + 1` literal bytes;
/// from $80 it is followed by one value repeated `257 − c` times. A literal block cut short
/// by the end of the stream outputs only the literals present; a trailing control byte
/// outputs nothing. An empty stream outputs nothing.
pub fn run_length(stream: &[u8], bias: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(stream.len() * 2);
    let mut rest = stream;
    while let Some((&control, after)) = rest.split_first() {
        if control < 0x80 {
            let count = (usize::from(control) + 1).min(after.len());
            let (literals, next) = after.split_at(count);
            out.extend(literals.iter().map(|&byte| byte.wrapping_add(bias)));
            rest = next;
        } else {
            let Some((&value, next)) = after.split_first() else {
                break;
            };
            let count = 257 - usize::from(control);
            out.extend(std::iter::repeat_n(value.wrapping_add(bias), count));
            rest = next;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_planes_combine_msb_first() {
        let mut tile = [0u8; 16];
        tile[0] = 0b1000_0001; // plane 0, row 0
        tile[8] = 0b1100_0000; // plane 1, row 0
        tile[15] = 0x01; // plane 1, row 7, rightmost
        let pixels = tile_pixels(&tile);
        assert_eq!(&pixels[..8], &[3, 2, 0, 0, 0, 0, 0, 1]);
        assert_eq!(pixels[63], 2);
        assert!(pixels[8..63].iter().all(|&p| p == 0));
        assert_eq!(tile_bytes(&pixels), tile);
    }

    #[test]
    fn tile_round_trip() {
        let tile: [u8; 16] = std::array::from_fn(|i| (i as u8).wrapping_mul(37) ^ 0xA5);
        assert_eq!(tile_bytes(&tile_pixels(&tile)), tile);
    }

    #[test]
    fn unmask_xors() {
        assert_eq!(unmask(&[0x00, 0xFF, 0x0F], 0), vec![0x00, 0xFF, 0x0F]);
        assert_eq!(unmask(&[0x00, 0xFF, 0x0F], 0xF0), vec![0xF0, 0x0F, 0xFF]);
    }

    #[test]
    fn font_glyph_becomes_tile() {
        let glyph = [0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x00];
        let tiles = font_tiles(&glyph, 0);
        assert_eq!(
            tiles,
            [
                0x00, 0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x00, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        let pixels = tile_pixels(&tiles[..16].try_into().unwrap());
        assert!(pixels[..8].iter().all(|&p| p == 0), "empty top row");
        assert_eq!(&pixels[8..16], &[0, 0, 0, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn font_mask_and_partial_glyph() {
        let data = [0u8; 7 + 3];
        let tiles = font_tiles(&data, 0xFF);
        assert_eq!(tiles.len(), 16);
        assert!(tiles[..8].iter().all(|&b| b == 0xFF));
        assert!(tiles[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn run_length_literals_and_runs() {
        // literal of 2, run of 3, run of 129, run of 2
        let stream = [0x01, 0xAA, 0xBB, 0xFE, 0x07, 0x80, 0x01, 0xFF, 0x02];
        let out = run_length(&stream, 0);
        let mut expected = vec![0xAA, 0xBB, 0x07, 0x07, 0x07];
        expected.extend([0x01; 129]);
        expected.extend([0x02; 2]);
        assert_eq!(out, expected);
    }

    #[test]
    fn run_length_bias_wraps() {
        let stream = [0xD8, 0x00, 0x03, 0x01, 0x02, 0x03, 0xFC];
        let out = run_length(&stream, 0x64);
        assert_eq!(out.len(), 41 + 4);
        assert!(out[..41].iter().all(|&b| b == 0x64));
        assert_eq!(&out[41..], &[0x65, 0x66, 0x67, 0x60]);
    }

    #[test]
    fn run_length_attribute_example_shape() {
        // The first blocks of the spec's attribute example, made up as synthetic data.
        let stream = [0xFE, 0x00, 0x00, 0xF0, 0xFD, 0x00];
        assert_eq!(run_length(&stream, 0), vec![0, 0, 0, 0xF0, 0, 0, 0, 0]);
    }

    #[test]
    fn run_length_end_inside_a_block() {
        // Literal block announcing 4 bytes with only 2 present.
        assert_eq!(run_length(&[0x03, 0x11, 0x22], 0), vec![0x11, 0x22]);
        // Literal block with no literal present.
        assert_eq!(run_length(&[0x00, 0x11, 0x05], 0), vec![0x11]);
        // A trailing run control byte outputs nothing.
        assert_eq!(run_length(&[0x00, 0x11, 0xFF], 0), vec![0x11]);
        // A run whose value is the last byte outputs the whole run.
        assert_eq!(run_length(&[0xFB, 0x09], 1), vec![0x0A; 6]);
    }

    #[test]
    fn run_length_empty() {
        assert!(run_length(&[], 0).is_empty());
    }
}
