//! [`Sound`] driving `src/apu` on the NES frame clock: every frame's register writes go to the
//! APU at the start of that frame, and the frame's samples are handed out as the caller asks
//! for them. The audio clock is the frame clock: a caller that pulls samples (an audio
//! callback) runs the driver at exactly 60.0988 frames per second of audio.

use super::{RegisterWrite, Sound, SoundData};
use crate::apu::{Apu, TimedWrite};
use crate::game::state::StateError;

/// The driver and the synthesiser.
///
/// Its state can be saved with a game's ([`Player::restore`]): the driver, the synthesiser and
/// the samples of the frame under way. What the program chooses (muting, gain) and the
/// imported sound data are not part of it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Player {
    sound: Sound,
    apu: Apu,
    /// Writes waiting for the start of the next frame (pause, resume).
    queued: Vec<RegisterWrite>,
    /// Scratch of the frame under way.
    #[serde(skip)]
    writes: Vec<RegisterWrite>,
    #[serde(skip)]
    timed: Vec<TimedWrite>,
    samples: Vec<f32>,
    /// Samples of `samples` already handed out.
    taken: usize,
    #[serde(skip)]
    muted: bool,
    #[serde(skip, default = "default_gain")]
    gain: f32,
    frames: u64,
}

fn default_gain() -> f32 {
    DEFAULT_GAIN
}

/// The default output gain: the mixer's full scale is about 1.0, which music never nears, so
/// the output is doubled (and clipped to ±1).
pub const DEFAULT_GAIN: f32 = 2.0;

impl Player {
    /// A powered-on console producing `sample_rate` samples a second.
    pub fn new(data: SoundData, sample_rate: u32) -> Player {
        let mut sound = Sound::new(data);
        let mut queued = Vec::new();
        sound.power_on(&mut queued);
        Player {
            sound,
            apu: Apu::new(sample_rate),
            queued,
            writes: Vec::new(),
            timed: Vec::new(),
            samples: Vec::new(),
            taken: 0,
            muted: false,
            gain: DEFAULT_GAIN,
            frames: 0,
        }
    }

    /// Takes the state of a saved player (as [`Game::load_state`](crate::game::Game::load_state)
    /// returns it), keeping this one's sound data, muting and gain. Refused, changing nothing,
    /// when it was saved at another sample rate.
    pub fn restore(&mut self, mut saved: Player) -> Result<(), StateError> {
        if saved.apu.sample_rate() != self.apu.sample_rate() {
            return Err(StateError::Damaged(format!(
                "the sound was saved at {} Hz, this one runs at {} Hz",
                saved.apu.sample_rate(),
                self.apu.sample_rate()
            )));
        }
        saved.sound.rebind(self.sound.driver().data().clone());
        saved.muted = self.muted;
        saved.gain = self.gain;
        *self = saved;
        Ok(())
    }

    pub fn sound(&self) -> &Sound {
        &self.sound
    }

    /// For requests: songs, effects, the RAM effect's pitch.
    pub fn sound_mut(&mut self) -> &mut Sound {
        &mut self.sound
    }

    pub fn sample_rate(&self) -> u32 {
        self.apu.sample_rate()
    }

    /// Frames run so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Muting keeps everything running and outputs silence.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Multiplies the output (clipped to ±1).
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn pause(&mut self) {
        self.sound.pause(&mut self.queued);
    }

    pub fn resume(&mut self) {
        self.sound.resume(&mut self.queued);
    }

    /// Runs one frame and returns its register writes and its samples.
    pub fn run_frame(&mut self) -> (&[RegisterWrite], &[f32]) {
        self.writes.clear();
        self.writes.append(&mut self.queued);
        self.sound.frame(&mut self.writes);
        self.timed.clear();
        self.timed
            .extend(self.writes.iter().map(|write| TimedWrite {
                cycle: 0,
                register: write.register,
                value: write.value,
            }));
        self.samples.clear();
        self.taken = 0;
        self.apu.run_frame(&self.timed, &mut self.samples);
        self.frames += 1;
        if self.muted {
            self.samples.fill(0.0);
        } else {
            for sample in &mut self.samples {
                *sample = (*sample * self.gain).clamp(-1.0, 1.0);
            }
        }
        (&self.writes, &self.samples)
    }

    /// Fills `out` with the next samples, running frames as needed.
    pub fn fill(&mut self, out: &mut [f32]) {
        let mut done = 0;
        while done < out.len() {
            if self.taken == self.samples.len() {
                self.run_frame();
                continue;
            }
            let count = (self.samples.len() - self.taken).min(out.len() - done);
            out[done..done + count].copy_from_slice(&self.samples[self.taken..self.taken + count]);
            self.taken += count;
            done += count;
        }
    }
}
