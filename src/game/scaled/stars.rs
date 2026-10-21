pub use crate::game::gameplay::stars::Star;
use crate::game::gameplay::stars::{LAYERS, Layer, SPAWN_AT, SPAWN_PHASE};

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

    /// The job's move of one tick (`docs/scaled.md`): `camera` is the
    /// tick's camera move, `sky` whether the window's top-left block lies in chunk 0. The
    /// layers follow the whole move at their ratios.
    pub fn drift(&mut self, camera: (i32, i32), sky: bool) {
        if self.mode == 0 {
            if !sky {
                return;
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
        let fading = self.mode == 0xFF;
        for i in (0..8).rev() {
            let star = &mut self.stars[i];
            if star.dead {
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
    }

    /// The spawn test of one frame of the NES's clock: a dead star appears when the frame
    /// counter reaches its phase. Every frame is tested, so all eight stars appear (the
    /// Original's calls see even counters only: `docs/deviations.md`).
    pub fn spawn(&mut self, counter: u32) {
        if self.mode != 1 {
            return;
        }
        for (i, star) in self.stars.iter_mut().enumerate() {
            if star.dead && (counter & 0x7F) as u8 == SPAWN_PHASE[i] {
                star.dead = false;
                (star.x, star.y) = SPAWN_AT[i];
            }
        }
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
        stars.drift((3, 0), false);
        assert_eq!(stars.mode, 0);
        stars.drift((3, 0), true);
        assert_eq!(stars.mode, 1);
        // Star 0 spawns at counter phase 2, at (0, $45); star 3 at the odd phase $67.
        stars.spawn(2);
        stars.spawn(0x67);
        assert!(!stars.stars[0].dead && !stars.stars[3].dead);
        assert_eq!(stars.drawn().count(), 8);
        assert_eq!((stars.stars[0].x, stars.stars[0].y), (0, 0x45));
        // A spawned star moves with the next tick's camera move.
        stars.drift((4, 0), true);
        assert_eq!(stars.stars[0].x, 252);
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
        stars.drift((4, 0), false);
        assert_eq!(stars.mode, 0xFF);
        assert!(stars.stars[0].dead);
        assert!(!stars.stars[1].dead);
        for _ in 0..43 {
            stars.drift((-8, 0), false);
        }
        assert_eq!(stars.mode, 0, "all stars are gone: the job switches off");
    }
}
