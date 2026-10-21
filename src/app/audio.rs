//! Sound output: an SDL audio stream whose callback pulls samples from the core's [`Player`],
//! which runs the sound driver on the NES frame clock as it fills them.

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};
use solar_jetman::game::sound::SoundData;
use solar_jetman::game::sound::player::Player;

/// Output sample rate.
const SAMPLE_RATE: u32 = 48_000;

/// The callback's side: the player and a buffer reused between calls.
pub struct Feed {
    player: Player,
    buffer: Vec<f32>,
}

impl AudioCallback<f32> for Feed {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let count = usize::try_from(requested).unwrap_or(0);
        self.buffer.resize(count, 0.0);
        self.player.fill(&mut self.buffer);
        // A failed put only loses this chunk; the callback has no way to report it.
        let _ = stream.put_data_f32(&self.buffer);
    }
}

/// The open audio stream.
pub struct Audio {
    stream: AudioStreamWithCallback<Feed>,
}

impl Audio {
    /// Opens the default playback device, mono at [`SAMPLE_RATE`], and starts it.
    pub fn open(sdl: &sdl3::Sdl, data: SoundData) -> Result<Audio, String> {
        let error = |error: sdl3::Error| format!("SDL audio: {error}");
        let subsystem = sdl.audio().map_err(error)?;
        let spec = AudioSpec {
            freq: Some(SAMPLE_RATE as i32),
            channels: Some(1),
            format: Some(AudioFormat::f32_sys()),
        };
        let feed = Feed {
            player: Player::new(data, SAMPLE_RATE),
            buffer: Vec::new(),
        };
        let stream = subsystem.open_playback_stream(&spec, feed).map_err(error)?;
        stream.resume().map_err(error)?;
        Ok(Audio { stream })
    }

    /// Runs `change` on the player, with the audio thread held off meanwhile.
    pub fn with_player<T>(&mut self, change: impl FnOnce(&mut Player) -> T) -> Option<T> {
        let mut guard = self.stream.lock()?;
        Some(change(&mut guard.player))
    }
}
