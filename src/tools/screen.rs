//! Comparing the remake's pictures with Mesen's screenshots: the remake's
//! NES colour numbers are turned into RGB with Mesen's own colours, measured by
//! `tools/mesen/palette-capture.lua`, so a pixel matches only when the console shows the same
//! colour there.

use crate::game::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::fs;
use std::path::Path;

/// Mesen's RGB colour of each NES colour number.
pub type MesenPalette = [[u8; 3]; 64];

/// Reads the table of `palette-capture.lua`: 64 lines "INDEX RRGGBB".
pub fn read_palette(path: &Path) -> Result<MesenPalette, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut palette = [[0; 3]; 64];
    let mut seen = 0u64;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(index), Some(rgb)) = (parts.next(), parts.next()) else {
            continue;
        };
        let bad = || format!("{}: bad line `{line}`", path.display());
        let index = usize::from_str_radix(index, 16).map_err(|_| bad())?;
        let rgb = u32::from_str_radix(rgb, 16).map_err(|_| bad())?;
        if index >= 64 {
            return Err(bad());
        }
        palette[index] = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
        seen |= 1 << index;
    }
    if seen != u64::MAX {
        return Err(format!("{}: not all 64 colours", path.display()));
    }
    Ok(palette)
}

/// A 256 × 240 screenshot as RGB pixels.
pub fn read_png(path: &Path) -> Result<Vec<[u8; 3]>, String> {
    let file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut data = vec![0; reader.output_buffer_size().ok_or("screenshot too large")?];
    let info = reader
        .next_frame(&mut data)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let step = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("{}: unsupported {other:?}", path.display())),
    };
    if (info.width, info.height) != (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32) {
        return Err(format!("{}: not 256 × 240", path.display()));
    }
    Ok(data[..info.buffer_size()]
        .chunks_exact(step)
        .map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// Writes RGB pixels, `width` wide, as a PNG.
pub fn write_png(path: &Path, width: usize, pixels: &[[u8; 3]]) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let height = pixels.len() / width;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(pixels.as_flattened())
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

/// The sprite memory in a capture's dump (`step-N.txt`: palette memory, then sprite memory).
pub fn sprite_memory(text: &str) -> Vec<u8> {
    text.lines()
        .nth(1)
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect()
}

/// Pixels of the picture where the console differs from the remake by design: the lines more
/// than 8 sprites `height` lines tall cross in `oam` (the console drops sprites there), the 8
/// topmost and bottommost lines (a sprite partly over an edge wraps round or hides as a whole),
/// the cell of sprite 0, the one-dot marker of a split (`docs/deviations.md`), and where two
/// sprites overlap: the original rotates their order every pass against flicker, so either
/// may be in front (`objects.md` 6.6).
pub fn console_mask(oam: &[u8], height: usize) -> Vec<bool> {
    let mut count = vec![0; SCREEN_HEIGHT];
    for sprite in oam.as_chunks::<4>().0.iter().filter(|s| s[0] < 0xEF) {
        let top = usize::from(sprite[0]) + 1;
        for n in count.iter_mut().skip(top).take(height) {
            *n += 1;
        }
    }
    let mut mask: Vec<bool> = count
        .iter()
        .enumerate()
        .flat_map(|(row, &n)| {
            std::iter::repeat_n(
                n > 8 || !(8..SCREEN_HEIGHT - 8).contains(&row),
                SCREEN_WIDTH,
            )
        })
        .collect();
    let mut cover = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT];
    for sprite in oam
        .as_chunks::<4>()
        .0
        .iter()
        .skip(1)
        .filter(|s| s[0] < 0xEF)
    {
        let (left, top) = (usize::from(sprite[3]), usize::from(sprite[0]) + 1);
        for row in (top..top + height).filter(|&row| row < SCREEN_HEIGHT) {
            for column in (left..left + 8).filter(|&column| column < SCREEN_WIDTH) {
                cover[row * SCREEN_WIDTH + column] += 1;
            }
        }
    }
    for (masked, &n) in mask.iter_mut().zip(&cover) {
        *masked |= n > 1;
    }
    if let [y, _, _, x, ..] = *oam {
        let (left, top) = (usize::from(x), usize::from(y) + 1);
        for row in (top..top + height).filter(|&row| row < SCREEN_HEIGHT) {
            for column in (left..left + 8).filter(|&column| column < SCREEN_WIDTH) {
                mask[row * SCREEN_WIDTH + column] = true;
            }
        }
    }
    mask
}

/// How one of our pictures compares with the original's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScreenDiff {
    /// Pixels whose colour differs.
    pub differing: usize,
    /// Those of them outside the masked lines.
    pub unexplained: usize,
    /// The box around the unexplained pixels: left, top, right, bottom (inclusive).
    pub area: Option<(usize, usize, usize, usize)>,
}

/// Compares `ours` (NES colour numbers) with `theirs`; `masked` marks the pixels to leave out.
pub fn compare(
    ours: &[u8],
    theirs: &[[u8; 3]],
    palette: &MesenPalette,
    masked: &[bool],
) -> ScreenDiff {
    let mut diff = ScreenDiff::default();
    for (i, (&colour, rgb)) in ours.iter().zip(theirs).enumerate() {
        if palette[usize::from(colour & 0x3F)] == *rgb {
            continue;
        }
        diff.differing += 1;
        if masked.get(i).copied().unwrap_or(false) {
            continue;
        }
        let (x, y) = (i % SCREEN_WIDTH, i / SCREEN_WIDTH);
        diff.unexplained += 1;
        diff.area = Some(match diff.area {
            None => (x, y, x, y),
            Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
        });
    }
    diff
}

/// The original's picture, ours, and the differences (magenta where unexplained, dim where
/// masked, the original greyed out elsewhere), side by side.
pub fn side_by_side(
    ours: &[u8],
    theirs: &[[u8; 3]],
    palette: &MesenPalette,
    masked: &[bool],
) -> Vec<[u8; 3]> {
    let mut pixels = Vec::with_capacity(3 * ours.len());
    for y in 0..SCREEN_HEIGHT {
        let row = y * SCREEN_WIDTH..(y + 1) * SCREEN_WIDTH;
        pixels.extend_from_slice(&theirs[row.clone()]);
        pixels.extend(
            ours[row.clone()]
                .iter()
                .map(|&c| palette[usize::from(c & 0x3F)]),
        );
        for i in row {
            let ours = palette[usize::from(ours[i] & 0x3F)];
            let [r, g, b] = theirs[i];
            let grey = ((u16::from(r) + u16::from(g) + u16::from(b)) / 9) as u8;
            pixels.push(match (ours == theirs[i], masked[i]) {
                (true, _) => [grey; 3],
                (false, false) => [0xFF, 0x00, 0xFF],
                (false, true) => [0x80, 0x00, 0x80],
            });
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crowded_lines() {
        // Sprite 0 at (20, 51), eight more on lines 101-108, one more below.
        let mut oam = vec![0xF8; 256];
        (oam[0], oam[3]) = (50, 20);
        for i in 1..10 {
            oam[4 * i] = 100;
        }
        oam[40] = 150;
        let mask = console_mask(&oam, 8);
        let at = |x: usize, y: usize| mask[y * SCREEN_WIDTH + x];
        assert!(at(100, 0) && at(100, 7) && !at(100, 8));
        assert!(!at(0, 100) && at(0, 101) && at(255, 108) && !at(0, 109));
        assert!(!at(0, 151) && at(0, 239));
        assert!(at(20, 51) && at(27, 58) && !at(28, 51) && !at(20, 50));
        // Two sprites overlapping on (104..108, 151..158).
        let mut oam = vec![0xF8; 256];
        (oam[4], oam[7], oam[8], oam[11]) = (150, 100, 150, 104);
        let mask = console_mask(&oam, 8);
        let at = |x: usize, y: usize| mask[y * SCREEN_WIDTH + x];
        assert!(at(104, 151) && at(107, 158) && !at(103, 151) && !at(108, 151));
    }

    #[test]
    fn differences() {
        let mut palette = [[0; 3]; 64];
        palette[1] = [1, 2, 3];
        let ours = vec![1u8; SCREEN_WIDTH * SCREEN_HEIGHT];
        let mut theirs = vec![[1, 2, 3]; ours.len()];
        theirs[20 * SCREEN_WIDTH + 30] = [0; 3];
        theirs[2 * SCREEN_WIDTH] = [0; 3];
        let masked = console_mask(&[], 8);
        let diff = compare(&ours, &theirs, &palette, &masked);
        assert_eq!(diff.differing, 2);
        assert_eq!(diff.unexplained, 1);
        assert_eq!(diff.area, Some((30, 20, 30, 20)));
        assert_eq!(
            side_by_side(&ours, &theirs, &palette, &masked).len(),
            3 * ours.len()
        );
    }
}
