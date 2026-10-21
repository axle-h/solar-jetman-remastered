//! The music and sound-effect data the driver plays, in our own event format. Songs and effects
//! are ripped from the player's ROM by `src/rom`'s import (`sound/song-NN.ron`,
//! `sound/effects.ron`; the format is described there), and so are the note periods, tuning
//! constants (`game/sound.ron`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Number of songs (`docs/spec/sound.md` 3.1).
pub const SONG_COUNT: usize = 34;
/// Number of entries in the note period table (sound.md 3.3).
pub const NOTE_PERIODS: usize = 61;
/// The highest effect id (sound.md 3.2).
pub const LAST_EFFECT_ID: u8 = 0x44;
/// The effect built in RAM, the tractor-beam hum (sound.md 4.3).
pub const RAM_EFFECT_ID: u8 = 0x1A;
/// File of the pack's `game/` folder with the note periods.
pub const SOUND_FILE: &str = "sound.ron";

/// One song: a tempo shared by its four tracks (pulse 1, pulse 2, triangle, noise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    /// Added to the tempo accumulator every frame; a tick when it carries (sound.md 6.1).
    pub tempo: u8,
    pub tracks: Vec<Track>,
}

/// A channel's part of a song: events read from index 0 on. Jumps and calls name event
/// indexes in the same list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub events: Vec<Event>,
}

/// One event of a track (sound.md 4.1). Durations are in ticks, 1-255, with 0 meaning 256.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// A note: `key` 1-127 (the note byte less $80), transposed when played; a key that
    /// transposes to 0 is a rest.
    Note(u8, u8),
    /// A rest (note byte $80, never transposed).
    Rest(u8),
    /// End of the track: the channel stops ($00).
    End,
    /// Continue at this event ($01).
    Jump(u16),
    /// Run the events from `to` until a `Return`, `times` times (0 = 256), then continue after
    /// the call ($04). One level only.
    Call { times: u8, to: u16 },
    /// End of a called block ($05).
    Return,
    /// Clears bit 0 of the id of the effect on this channel ($02; a data slip in one song,
    /// sound.md 4.4).
    ClearEffectLock,
    /// Channel setup ($03): control byte, sweep byte, period-high byte.
    Setup { control: u8, sweep: u8, high: u8 },
    /// Vibrato on ($08): steps per half cycle, interval − 1, depth.
    Vibrato { length: u8, rate: u8, depth: u8 },
    /// Vibrato off ($09).
    VibratoOff,
    /// Pitch slide ($0A up, $0B down): start delay, interval − 1, steps, amount per step, steps
    /// in the opposite direction first.
    Slide {
        up: bool,
        delay: u8,
        rate: u8,
        steps: u8,
        amount: u8,
        back: u8,
    },
    /// Set volume ($0C).
    Volume(u8),
    /// Volume ramp ($0D): delay, interval − 1, target.
    VolumeRamp { delay: u8, rate: u8, target: u8 },
    /// Volume oscillation ($0E): half period, interval − 1, signed delta.
    VolumeWave { half: u8, rate: u8, delta: u8 },
    /// Volume effect off ($0F).
    VolumeEffectOff,
    /// Tempo := value ($10).
    Tempo(u8),
    /// Tempo += value ($11).
    TempoAdd(u8),
    /// This channel's transpose := value ($12).
    Transpose(u8),
    /// This channel's transpose += value ($13).
    TransposeAdd(u8),
    /// Every channel's transpose := value ($14).
    TransposeAll(u8),
    /// Retrigger off ($15).
    RetriggerOff,
    /// Retrigger on, then a volume ramp ($16): delay, interval − 1, target.
    Retrigger { delay: u8, rate: u8, target: u8 },
}

/// One sound effect (sound.md 4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    /// The even id, $02-$44.
    pub id: u8,
    /// Built in RAM by the game, which changes its pitch while it plays (sound.md 4.3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_ram: bool,
    /// Upper four bits of the first register (header byte 0 AND $F0).
    pub control: u8,
    /// Each step lasts this + 1 frames (header byte 0 AND $0F).
    pub frames_per_step: u8,
    /// Sweep byte.
    pub sweep: u8,
    /// Initial fourth-register byte: length index in bits 3-7, period bits 8-10 in bits 0-2.
    pub high: u8,
    pub steps: Vec<EffectStep>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One step of an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectStep {
    /// Volume 1-15 and an 11-bit period (for noise: bits 0-7 are the period register's value).
    Tone(u8, u16),
    /// Clears bit 0 of the playing effect's id, then the next step is read at once ($02).
    ClearLock,
}

/// `sound/effects.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effects {
    pub effects: Vec<Effect>,
}

/// `game/sound.ron`: the sound constants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundTables {
    /// Timer periods of the notes: entry 0 is a rest, 1-60 are C2 to B6 (sound.md 3.3).
    pub note_periods: Vec<u16>,
}

impl SoundTables {
    /// Reads `game/sound.ron`, checking the table's length.
    pub fn parse(text: &str) -> Result<SoundTables, SoundDataError> {
        let tables: SoundTables = ron::from_str(text)
            .map_err(|error| SoundDataError(format!("{SOUND_FILE}: {error}")))?;
        if tables.note_periods.len() != NOTE_PERIODS {
            return Err(SoundDataError(format!(
                "{SOUND_FILE}: {} note periods, expected {NOTE_PERIODS}",
                tables.note_periods.len()
            )));
        }
        Ok(tables)
    }
}

/// Everything the driver plays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundData {
    pub songs: Vec<Song>,
    /// Indexed by id / 2; `None` for id 0 and ids without data.
    pub effects: Vec<Option<Effect>>,
    pub note_periods: Vec<u16>,
}

/// Why sound data cannot be used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundDataError(pub String);

impl fmt::Display for SoundDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sound data: {}", self.0)
    }
}

impl std::error::Error for SoundDataError {}

impl SoundData {
    /// Puts songs, effects and periods together, checking that every song has four tracks,
    /// every jump and call stays in its track, and the period table is complete.
    pub fn new(
        songs: Vec<Song>,
        effects: Vec<Effect>,
        note_periods: Vec<u16>,
    ) -> Result<SoundData, SoundDataError> {
        let fail = |message: String| Err(SoundDataError(message));
        if songs.len() != SONG_COUNT {
            return fail(format!("{} songs, expected {SONG_COUNT}", songs.len()));
        }
        if note_periods.len() != NOTE_PERIODS {
            return fail(format!(
                "{} note periods, expected {NOTE_PERIODS}",
                note_periods.len()
            ));
        }
        for (number, song) in songs.iter().enumerate() {
            if song.tracks.len() != 4 {
                return fail(format!("song {number} has {} tracks", song.tracks.len()));
            }
            for (channel, track) in song.tracks.iter().enumerate() {
                let len = track.events.len();
                let bad = track.events.iter().any(|event| match *event {
                    Event::Jump(to) | Event::Call { to, .. } => usize::from(to) >= len,
                    _ => false,
                });
                if bad || len == 0 {
                    return fail(format!(
                        "song {number}, channel {channel}: a jump or call leaves the track"
                    ));
                }
            }
        }
        let mut by_id = vec![None; usize::from(LAST_EFFECT_ID) / 2 + 1];
        for effect in effects {
            let slot = usize::from(effect.id / 2);
            if effect.id & 1 != 0 || effect.id == 0 || slot >= by_id.len() {
                return fail(format!("effect id ${:02X} out of range", effect.id));
            }
            by_id[slot] = Some(effect);
        }
        Ok(SoundData {
            songs,
            effects: by_id,
            note_periods,
        })
    }

    /// The effect with this id (bit 0 ignored), if there is one.
    pub fn effect(&self, id: u8) -> Option<&Effect> {
        self.effects.get(usize::from(id / 2))?.as_ref()
    }
}
