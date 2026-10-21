//! Tile sheets: tiles in the NES 2-bit format as indexed PNG images, 16 tiles per row, with
//! pixel values 0-3 as palette indexes and a grey palette for viewing.

use crate::rom::decode::{TILE_BYTES, tile_bytes, tile_pixels};
use std::fmt;
use std::io::Cursor;

/// Tiles per row of a sheet.
pub const SHEET_COLUMNS: usize = 16;

/// The palette written into sheets: black, dark grey, light grey, white.
const GREYS: [u8; 12] = [
    0x00, 0x00, 0x00, 0x55, 0x55, 0x55, 0xAA, 0xAA, 0xAA, 0xFF, 0xFF, 0xFF,
];

/// A tile sheet that cannot be written or read back.
#[derive(Debug)]
pub enum SheetError {
    /// The PNG library failed to encode.
    Encode(png::EncodingError),
    /// The PNG library failed to decode.
    Decode(png::DecodingError),
    /// The image is not an indexed or 2-bit grey PNG.
    UnsupportedFormat,
    /// The image is too small to hold the requested tiles.
    TooSmall { tiles: usize },
    /// A pixel has a value above 3.
    ValueOutOfRange { value: u8 },
}

impl fmt::Display for SheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SheetError::Encode(error) => write!(f, "PNG encoding failed: {error}"),
            SheetError::Decode(error) => write!(f, "PNG decoding failed: {error}"),
            SheetError::UnsupportedFormat => {
                write!(f, "tile sheet is not an indexed (or 2-bit grey) PNG")
            }
            SheetError::TooSmall { tiles } => {
                write!(f, "tile sheet too small for {tiles} tiles")
            }
            SheetError::ValueOutOfRange { value } => {
                write!(f, "tile sheet pixel value {value} is above 3")
            }
        }
    }
}

impl std::error::Error for SheetError {}

/// Width and height in pixels of a sheet of `tiles` tiles (at least one row).
pub fn sheet_size(tiles: usize) -> (u32, u32) {
    let rows = tiles.div_ceil(SHEET_COLUMNS).max(1);
    ((SHEET_COLUMNS * 8) as u32, (rows * 8) as u32)
}

/// Encodes tiles as an indexed 2-bit PNG: `tiles.len() / 16` tiles, rounded up. A partial last
/// tile (a stream that ends a few bytes into a tile, graphics.md 1 "Writes run on") is kept,
/// padded with zero bytes, so the whole decoded stream can be read back; the manifest records
/// its exact length. Cells after the last tile are value 0.
pub fn encode_sheet(tiles: &[u8]) -> Result<Vec<u8>, SheetError> {
    let count = tiles.len().div_ceil(TILE_BYTES);
    let (width, height) = sheet_size(count);
    let line = width as usize / 4;
    let mut packed = vec![0u8; line * height as usize];
    for (index, bytes) in tiles.chunks(TILE_BYTES).enumerate() {
        let mut tile = [0u8; TILE_BYTES];
        tile[..bytes.len()].copy_from_slice(bytes);
        let pixels = tile_pixels(&tile);
        let x0 = index % SHEET_COLUMNS * 8;
        let y0 = index / SHEET_COLUMNS * 8;
        for (i, &value) in pixels.iter().enumerate() {
            let x = x0 + i % 8;
            let y = y0 + i / 8;
            packed[y * line + x / 4] |= value << (6 - 2 * (x % 4));
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Two);
    encoder.set_palette(&GREYS[..]);
    let mut writer = encoder.write_header().map_err(SheetError::Encode)?;
    writer
        .write_image_data(&packed)
        .map_err(SheetError::Encode)?;
    writer.finish().map_err(SheetError::Encode)?;
    Ok(out)
}

/// Reads the first `tiles` tiles back from a sheet as 2-bit tile bytes. Accepts indexed PNGs
/// of any bit depth (indexes must be 0-3) and 2-bit grey PNGs, so a sheet re-saved by an image
/// editor still loads.
pub fn decode_sheet(png_bytes: &[u8], tiles: usize) -> Result<Vec<u8>, SheetError> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(SheetError::Decode)?;
    let size = reader
        .output_buffer_size()
        .ok_or(SheetError::UnsupportedFormat)?;
    let mut buffer = vec![0u8; size];
    let info = reader.next_frame(&mut buffer).map_err(SheetError::Decode)?;
    let depth = match (info.color_type, info.bit_depth) {
        (png::ColorType::Indexed, png::BitDepth::One) => 1,
        (png::ColorType::Indexed, png::BitDepth::Two)
        | (png::ColorType::Grayscale, png::BitDepth::Two) => 2,
        (png::ColorType::Indexed, png::BitDepth::Four) => 4,
        (png::ColorType::Indexed, png::BitDepth::Eight) => 8,
        _ => return Err(SheetError::UnsupportedFormat),
    };
    let columns = info.width as usize / 8;
    let rows = info.height as usize / 8;
    if columns == 0 || columns * rows < tiles {
        return Err(SheetError::TooSmall { tiles });
    }
    let pixel = |x: usize, y: usize| -> u8 {
        let bit = x * depth;
        let byte = buffer[y * info.line_size + bit / 8];
        let shift = 8 - depth - bit % 8;
        (byte >> shift) & ((1u16 << depth) - 1) as u8
    };
    let mut out = Vec::with_capacity(tiles * TILE_BYTES);
    for index in 0..tiles {
        let x0 = index % columns * 8;
        let y0 = index / columns * 8;
        let mut pixels = [0u8; 64];
        for (i, value) in pixels.iter_mut().enumerate() {
            *value = pixel(x0 + i % 8, y0 + i / 8);
            if *value > 3 {
                return Err(SheetError::ValueOutOfRange { value: *value });
            }
        }
        out.extend(tile_bytes(&pixels));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_tiles(count: usize) -> Vec<u8> {
        (0..count * TILE_BYTES)
            .map(|i| (i as u8).wrapping_mul(29).rotate_left(3) ^ (i / 16) as u8)
            .collect()
    }

    #[test]
    fn sizes() {
        assert_eq!(sheet_size(0), (128, 8));
        assert_eq!(sheet_size(16), (128, 8));
        assert_eq!(sheet_size(17), (128, 16));
    }

    #[test]
    fn round_trip() {
        for count in [1, 16, 37] {
            let tiles = synthetic_tiles(count);
            let png_bytes = encode_sheet(&tiles).unwrap();
            assert_eq!(decode_sheet(&png_bytes, count).unwrap(), tiles);
        }
    }

    #[test]
    fn partial_tile_is_kept_padded() {
        let mut tiles = synthetic_tiles(2);
        tiles.push(0xFF);
        let png_bytes = encode_sheet(&tiles).unwrap();
        let back = decode_sheet(&png_bytes, 3).unwrap();
        assert_eq!(back[..33], tiles[..]);
        assert_eq!(back[33..], [0u8; 15]);
        // the rest of the sheet is empty
        assert_eq!(decode_sheet(&png_bytes, 4).unwrap()[48..], [0u8; 16]);
    }

    #[test]
    fn png_is_indexed_two_bit() {
        let png_bytes = encode_sheet(&synthetic_tiles(3)).unwrap();
        let decoder = png::Decoder::new(Cursor::new(&png_bytes[..]));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.color_type, png::ColorType::Indexed);
        assert_eq!(info.bit_depth, png::BitDepth::Two);
        assert_eq!((info.width, info.height), (128, 8));
    }

    #[test]
    fn eight_bit_indexed_sheet_loads() {
        let tiles = synthetic_tiles(2);
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 16, 8);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(&GREYS[..]);
        let mut data = vec![0u8; 16 * 8];
        for t in 0..2 {
            let pixels = tile_pixels(tiles[t * 16..t * 16 + 16].try_into().unwrap());
            for (i, &p) in pixels.iter().enumerate() {
                data[(i / 8) * 16 + t * 8 + i % 8] = p;
            }
        }
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
        assert_eq!(decode_sheet(&out, 2).unwrap(), tiles);
        assert!(matches!(
            decode_sheet(&out, 3),
            Err(SheetError::TooSmall { tiles: 3 })
        ));
    }
}
