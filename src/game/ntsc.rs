//! The NES master palette: RGB for the 64 colour numbers, computed from a model of the PPU's
//! composite video signal rather than copied from any palette file.
//!
//! The model, from the public description of the signal on the NESdev wiki ("NTSC video"):
//!
//! - A colour number is a hue (bits 0-3) and a luma (bits 4-5). For each colour the PPU outputs a
//!   square wave between a low and a high voltage, one cycle per cycle of the 3.58 MHz colour
//!   subcarrier. We sample that cycle 12 times (30° apart). Hue 0 stays at the high level,
//!   hue 13 at the low level, hues 14 and 15 at black; hues 1-12 are high for 6 of the 12
//!   samples, the window moving one sample per hue step.
//! - Voltages, as measured on hardware: low levels 0.350, 0.518, 0.962, 1.550 V and high levels
//!   1.094, 1.506, 1.962, 1.962 V for lumas 0-3; black is 0.518 V and white 1.962 V. A level
//!   is normalised to 0 at black and 1 at white.
//! - A television separates the signal over one subcarrier cycle: luma Y is the mean of the 12
//!   samples, and the chroma components U and V are the signal correlated with the subcarrier
//!   (twice the mean of sample × cosine and sample × sine of its phase). The colour burst has
//!   the phase of hue 8 and stands for 180° on the U-V plane, so all hues are turned by the same
//!   angle to put hue 8 there; hue angles then run the other way round from the sample index.
//! - Y, U and V become RGB with the standard (Rec. 601) YUV equations. The result is used
//!   directly as sRGB, clamped to 0-1 (a television's gamma is close to sRGB's), and rounded.
//!
//! Colour emphasis is not modelled; the game does not use it.

/// Low and high signal levels in volts by luma.
const LOW: [f64; 4] = [0.350, 0.518, 0.962, 1.550];
const HIGH: [f64; 4] = [1.094, 1.506, 1.962, 1.962];
const BLACK: f64 = 0.518;
const WHITE: f64 = 1.962;

/// Samples per subcarrier cycle.
const SAMPLES: usize = 12;

/// The burst hue, which defines the reference phase.
const BURST_HUE: usize = 8;

/// The 12 normalised samples of one cycle of colour `colour` (0-63).
fn samples(colour: usize) -> [f64; SAMPLES] {
    let hue = colour & 15;
    let luma = (colour >> 4) & 3;
    std::array::from_fn(|p| {
        let volts = match hue {
            0 => HIGH[luma],
            13 => LOW[luma],
            14 | 15 => BLACK,
            _ if (hue + p) % SAMPLES < SAMPLES / 2 => HIGH[luma],
            _ => LOW[luma],
        };
        (volts - BLACK) / (WHITE - BLACK)
    })
}

/// Luma and the two chroma components of a colour, before turning to the burst phase.
fn demodulate(colour: usize) -> (f64, f64, f64) {
    let s = samples(colour);
    let n = SAMPLES as f64;
    let y = s.iter().sum::<f64>() / n;
    let (mut c, mut q) = (0.0, 0.0);
    for (p, &level) in s.iter().enumerate() {
        // The subcarrier phase advances by 30° per sample; hues step the opposite way.
        let phase = -(p as f64) * std::f64::consts::TAU / n;
        c += level * phase.cos();
        q += level * phase.sin();
    }
    (y, 2.0 * c / n, 2.0 * q / n)
}

/// RGB (0-255 per component) of every NES colour number.
pub fn master_palette() -> [[u8; 3]; 64] {
    let (_, bc, bq) = demodulate(BURST_HUE);
    let burst = bq.atan2(bc);
    let turn = std::f64::consts::PI - burst;
    let (sin, cos) = turn.sin_cos();
    std::array::from_fn(|colour| {
        let (y, c, q) = demodulate(colour);
        let u = c * cos - q * sin;
        let v = c * sin + q * cos;
        let r = y + 1.139_83 * v;
        let g = y - 0.394_65 * u - 0.580_60 * v;
        let b = y + 2.032_11 * u;
        [r, g, b].map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greys_follow_the_levels() {
        let palette = master_palette();
        for colour in [0x0D, 0x0E, 0x0F, 0x1D, 0x1E, 0x2E, 0x3F] {
            assert_eq!(palette[colour], [0, 0, 0], "{colour:#04X}");
        }
        assert_eq!(palette[0x20], [255, 255, 255]);
        assert_eq!(palette[0x30], [255, 255, 255]);
        // Hue 0 and 13 are grey, getting lighter with luma.
        let grey = |colour: usize| {
            let [r, g, b] = palette[colour];
            assert!(r.abs_diff(g) <= 1 && g.abs_diff(b) <= 1, "{colour:#04X}");
            r
        };
        assert!(grey(0x00) < grey(0x10));
        assert!(grey(0x2D) < grey(0x00));
        assert!(grey(0x3D) > grey(0x2D));
    }

    #[test]
    fn hues_land_where_expected() {
        let palette = master_palette();
        let [r, g, b] = palette[0x12];
        assert!(b > r && b > g, "$12 is blue: {r} {g} {b}");
        let [r, g, b] = palette[0x16];
        assert!(r > g && r > b, "$16 is red: {r} {g} {b}");
        let [r, g, b] = palette[0x1A];
        assert!(g > r && g > b, "$1A is green: {r} {g} {b}");
        let [r, g, b] = palette[0x28];
        assert!(r > b && g > b, "$28 is yellow: {r} {g} {b}");
        let [r, g, b] = palette[0x14];
        assert!(r > g && b > g, "$14 is magenta: {r} {g} {b}");
        let [r, g, b] = palette[0x2C];
        assert!(g > r && b > r, "$2C is cyan: {r} {g} {b}");
    }

    #[test]
    fn hues_of_one_luma_have_equal_brightness() {
        let (y1, ..) = demodulate(0x11);
        for hue in 2..=12 {
            let (y, ..) = demodulate(0x10 + hue);
            assert!((y - y1).abs() < 1e-12);
        }
    }
}
