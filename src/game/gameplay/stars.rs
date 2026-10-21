//! The background's parallax stars (`docs/spec/graphics.md` 5.7.3): eight one-pixel stars that
//! drift against the scroll over open sky. The job that moves them also leaves the horizontal
//! thrust carry-in of `docs/spec/player.md` 4.6.

/// Layer of each star: how much of the camera's movement it follows (1, 3/4 or 1/2).
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Layer {
    Full,
    ThreeQuarters,
    Half,
}

pub(crate) const LAYERS: [Layer; 8] = [
    Layer::Full,
    Layer::ThreeQuarters,
    Layer::Half,
    Layer::ThreeQuarters,
    Layer::ThreeQuarters,
    Layer::Full,
    Layer::ThreeQuarters,
    Layer::Half,
];
/// The frame counter value (AND $7F) at which a dead star appears.
pub(crate) const SPAWN_PHASE: [u8; 8] = [0x02, 0x28, 0x32, 0x67, 0x16, 0x35, 0x56, 0x71];
pub(crate) const SPAWN_AT: [(u8, u8); 8] = [
    (0, 0x45),
    (84, 0xFC),
    (0, 0xC4),
    (199, 0xFC),
    (0, 0x23),
    (135, 0xFC),
    (0, 0x87),
    (33, 0xFC),
];

/// One star: position in screen pixels with fractions, and whether it is on the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Star {
    pub x: u8,
    pub y: u8,
    pub(crate) x_fraction: u8,
    pub(crate) y_fraction: u8,
    pub dead: bool,
}

/// The job's state: the mode RAM $D5 (0 off, 1 on, $FF fading out) and the stars.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stars {
    pub mode: u8,
    pub stars: [Star; 8],
}

impl Stars {
    /// The state at a level (re)start: every star dead, the mode as it was (it is not reset).
    pub fn new(mode: u8) -> Stars {
        Stars {
            mode,
            stars: [Star {
                dead: true,
                ..Star::default()
            }; 8],
        }
    }

    /// Validation: the bytes a star keeps from the session before the level (position,
    /// fractions and whether it is dead).
    pub fn set_leftover(&mut self, index: usize, at: (u8, u8), fractions: (u8, u8), dead: bool) {
        let star = &mut self.stars[index];
        (star.x, star.y) = at;
        (star.x_fraction, star.y_fraction) = fractions;
        star.dead = dead;
    }

    /// A level load or a respawn: every star's y becomes $FF; the mode, the dead mask, x and
    /// the fractions stay (graphics.md 5.7.3).
    pub fn level_load(&mut self) {
        for star in &mut self.stars {
            star.y = 0xFF;
        }
    }

    /// One call of the job. `camera` is the frame's camera movement, `sky` whether the window's
    /// top-left block lies in chunk 0, `counter` the frame counter. Returns the horizontal
    /// thrust carry-in byte the call leaves, or `None` when it leaves the byte alone.
    ///
    /// The calls only ever see an even counter, so stars 3, 5 and 7, whose spawn phases are
    /// odd, never appear (`docs/deviations.md`). With `every_phase` a call also takes the frame
    /// before it, so every star spawns.
    pub fn call(
        &mut self,
        camera: (i32, i32),
        sky: bool,
        counter: u32,
        every_phase: bool,
    ) -> Option<u8> {
        if self.mode == 0 {
            if !sky {
                return None;
            }
            for star in &mut self.stars {
                star.y = 0;
            }
            self.mode = 1;
        } else if !sky {
            self.mode = 0xFF;
        }
        // 8.8 signed values: whole pixels in the high byte.
        let full = (((-camera.0) as i16) << 8, ((-camera.1) as i16) << 8);
        let half = (full.0 >> 1, full.1 >> 1);
        let quarter = (half.0 >> 1, half.1 >> 1);
        let carry = (half.0 >> 8) as u8;
        let fading = self.mode == 0xFF;
        for i in (0..8).rev() {
            let star = &mut self.stars[i];
            if star.dead {
                let phase = |c: u32| (c & 0x7F) as u8 == SPAWN_PHASE[i];
                if !fading && (phase(counter) || every_phase && phase(counter.wrapping_sub(1))) {
                    star.dead = false;
                    (star.x, star.y) = SPAWN_AT[i];
                }
                continue;
            }
            let (mx, my) = if fading {
                full
            } else {
                match LAYERS[i] {
                    Layer::Full => full,
                    Layer::ThreeQuarters => (half.0 + quarter.0, half.1 + quarter.1),
                    Layer::Half => half,
                }
            };
            let x = u16::from_be_bytes([star.x, star.x_fraction]).wrapping_add(mx as u16);
            [star.x, star.x_fraction] = x.to_be_bytes();
            if fading && star.x < 8 {
                star.dead = true;
                star.y = 0xFC;
                continue;
            }
            let y = u16::from_be_bytes([star.y, star.y_fraction]).wrapping_add(my as u16);
            [star.y, star.y_fraction] = y.to_be_bytes();
            if fading && star.y >= 0xF8 {
                star.dead = true;
                star.y = 0xFC;
            }
        }
        if fading && self.stars.iter().all(|star| star.dead) {
            self.mode = 0;
        }
        Some(carry)
    }

    /// The stars put in sprite memory: all eight while the job is on, live or dead, at their
    /// own bytes (a dead one at its last place), with their index; none when off
    /// (graphics.md 5.7.3, "The exact drawing rule").
    pub fn drawn(&self) -> impl Iterator<Item = (usize, &Star)> {
        let on = self.mode != 0;
        self.stars.iter().enumerate().filter(move |_| on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_off_below_the_sky_and_starts_in_it() {
        let mut stars = Stars::new(0);
        assert_eq!(stars.call((3, 0), false, 0, false), None);
        assert_eq!(stars.mode, 0);
        // Camera moving right by 3: the carry-in is floor(-3 / 2) = -2.
        assert_eq!(stars.call((3, 0), true, 2, false), Some(0xFE));
        assert_eq!(stars.mode, 1);
        // Star 0 spawns at counter phase 2, at (0, $45).
        assert!(!stars.stars[0].dead);
        // All eight go into sprite memory while the job is on, the dead ones too.
        assert_eq!(stars.drawn().count(), 8);
        assert_eq!(stars.stars.iter().filter(|star| !star.dead).count(), 1);
        assert_eq!((stars.stars[0].x, stars.stars[0].y), (0, 0x45));
    }

    #[test]
    fn odd_phases_spawn_only_with_every_phase() {
        // Calls see even counters: star 3's phase $67 is never met unless the call also takes
        // the frame before.
        let mut stars = Stars::new(1);
        for c in (0..256).step_by(2) {
            stars.call((0, 0), true, c, false);
        }
        assert!(stars.stars[3].dead && stars.stars[5].dead && stars.stars[7].dead);
        for c in (0..256).step_by(2) {
            stars.call((0, 0), true, c, true);
        }
        assert!(stars.stars.iter().all(|star| !star.dead));
    }

    #[test]
    fn fading_out_ends_with_the_job_off() {
        let mut stars = Stars::new(1);
        for (x, y) in [(10, 100), (100, 100)] {
            let star = &mut stars.stars[usize::from(x == 100)];
            *star = Star {
                x,
                y,
                ..Star::default()
            };
        }
        // Out of the sky the stars slide at the full camera speed; the first dies below x 8.
        assert!(stars.call((4, 0), false, 1, false).is_some());
        assert_eq!(stars.mode, 0xFF);
        assert!(stars.stars[0].dead);
        assert_eq!(stars.stars[0].y, 0xFC);
        assert!(!stars.stars[1].dead);
        // The other one slides right with the camera moving left and dies at the wrap.
        for _ in 0..3 {
            stars.call((-8, 0), false, 3, false);
        }
        assert_eq!(stars.mode, 0xFF, "the second star is still on the field");
        for _ in 0..40 {
            stars.call((-8, 0), false, 3, false);
        }
        assert_eq!(stars.mode, 0, "all stars are gone: the job switches off");
    }
}
