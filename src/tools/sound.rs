//! Sound tools: the driver's data loaded from the ROM, rendering a song or effect to a WAV
//! file, and comparing our driver's register writes with captures of the original.

use crate::game::sound::data::{RAM_EFFECT_ID, SONG_COUNT, SoundData};
use crate::game::sound::{Channel, Driver, EFFECTS};
use crate::rom::Rom;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The driver's data: songs, effects and note periods imported from the ROM.
pub fn sound_data(rom: &Rom) -> Result<SoundData, String> {
    let pack = crate::rom::import(rom).map_err(|error| error.to_string())?;
    let periods = crate::rom::note_periods(&pack.files).map_err(|error| error.to_string())?;
    crate::rom::sound_data(&pack.files, periods).map_err(|error| error.to_string())
}

/// A WAV file of 16-bit mono PCM.
pub fn wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(sample_rate.to_le_bytes());
    out.extend((sample_rate * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        out.extend(value.to_le_bytes());
    }
    out
}

/// How long `song` takes, as our driver plays it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Length {
    /// Every channel has ended after this many frames.
    Ends(u32),
    /// Every channel has ended or looped back the asked number of times after this many.
    Loops(u32),
    /// Neither within the limit.
    Longer(u32),
}

impl Length {
    pub fn frames(self) -> u32 {
        match self {
            Length::Ends(frames) | Length::Loops(frames) | Length::Longer(frames) => frames,
        }
    }
}

/// Frames until every channel of `song` has ended or looped back at least `loops` times, as
/// our driver plays it (at most `limit`).
pub fn song_length(data: &SoundData, song: usize, loops: u32, limit: u32) -> Length {
    let mut driver = Driver::new(data.clone());
    driver.start_song(song);
    let mut writes = Vec::new();
    for frame in 1..=limit {
        writes.clear();
        driver.update(&mut writes);
        let states = Channel::ALL.map(|channel| driver.channel(channel));
        if states.iter().all(|state| !state.music_active) {
            return Length::Ends(frame);
        }
        if states
            .iter()
            .all(|state| !state.music_active || state.loops >= loops)
        {
            return Length::Loops(frame);
        }
    }
    Length::Longer(limit)
}

/// The original's request slot channel codes (sound.md 8.2).
pub fn channel_code(channel: Channel) -> u8 {
    match channel {
        Channel::Pulse1 => 0x15,
        Channel::Pulse2 => 0x18,
        Channel::Triangle => 0x1B,
        Channel::Noise => 0x1E,
    }
}

/// Effect requests for a capture: every effect of [`EFFECTS`], one every `spacing` frames from
/// `first`, as "frame slot channel id;..." for the capture script.
pub fn effect_requests(first: u32, spacing: u32) -> String {
    let mut text = String::new();
    for (index, effect) in EFFECTS.iter().enumerate() {
        let frame = first + spacing * index as u32;
        for (&(channel, id), slot) in effect.starts.iter().zip(["A", "B"]) {
            let _ = write!(
                text,
                "{frame} {slot} {:02X} {id:02X};",
                channel_code(channel)
            );
        }
    }
    text
}

/// Direct effect starts for a capture: every effect of [`EFFECTS`] on its channels, one every
/// `spacing` frames from `first`, as "frame channel id address;..." for the capture script.
/// `pointers` gives each id's bank 4 address (sound.md 3.2).
pub fn effect_starts(first: u32, spacing: u32, pointers: &[u16]) -> String {
    let mut text = String::new();
    for (index, effect) in EFFECTS.iter().enumerate() {
        let frame = first + spacing * index as u32;
        for &(channel, id) in effect.starts {
            let address = pointers.get(usize::from(id / 2)).copied().unwrap_or(0);
            let _ = write!(text, "{frame} {} {id:02X} {address:04X};", channel.index());
        }
    }
    text
}

/// The effect start addresses by id / 2, from the ROM's pointer table (sound.md 3.2).
pub fn effect_pointers(rom: &Rom) -> Result<Vec<u16>, String> {
    use crate::rom::sound::at;
    let bytes = rom
        .bytes(
            at::EFFECT_POINTERS,
            usize::from(crate::game::sound::data::LAST_EFFECT_ID) + 2,
        )
        .ok_or("the effect pointer table is past the end of the ROM")?;
    Ok(bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

/// One driver update of a capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Update {
    pub frame: u32,
    /// Driver RAM the game changed since the previous update ended.
    pub changes: Vec<(u16, u8)>,
    /// The driver's RAM when it changed: $F8, $F9, then $0637-$06ED.
    pub ram: Option<Vec<u8>>,
    /// APU writes during the update.
    pub writes: Vec<(u16, u8)>,
}

/// A parsed capture log.
#[derive(Debug, Clone, Default)]
pub struct Capture {
    pub updates: Vec<Update>,
    /// APU register writes outside driver updates ($4000-$4013, $4015, $4017): frame,
    /// register, value.
    pub other_writes: Vec<(u32, u16, u8)>,
    /// The frame the run ended at, if it ran to the end.
    pub end: Option<u32>,
}

const RAM_FIRST: u16 = 0x0637;

impl Update {
    /// A byte of the driver's RAM (zero page $F8/$F9 or $0637-$06ED).
    pub fn ram(&self, address: u16) -> Option<u8> {
        let ram = self.ram.as_ref()?;
        match address {
            0xF8 => ram.first().copied(),
            0xF9 => ram.get(1).copied(),
            _ => ram
                .get(2 + usize::from(address.checked_sub(RAM_FIRST)?))
                .copied(),
        }
    }
}

fn hex(text: &str) -> Result<u32, String> {
    u32::from_str_radix(text, 16).map_err(|_| format!("`{text}` is not hexadecimal"))
}

/// Reads a log written by `tools/mesen/sound-capture.lua`.
pub fn parse_capture(text: &str) -> Result<Capture, String> {
    let mut capture = Capture::default();
    for (number, line) in text.lines().enumerate() {
        let fail = |message: String| format!("line {}: {message}", number + 1);
        let mut words = line.split_whitespace();
        let current = capture.updates.last_mut();
        match (words.next(), current) {
            (Some("u"), _) => capture.updates.push(Update {
                frame: words
                    .next()
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| fail("frame".into()))?,
                ..Update::default()
            }),
            (Some("x"), Some(update)) => {
                for word in words {
                    let (address, value) = word.split_once('=').ok_or(fail(word.to_owned()))?;
                    update.changes.push((
                        hex(address).map_err(fail)? as u16,
                        hex(value).map_err(fail)? as u8,
                    ));
                }
            }
            (Some("s"), Some(update)) => {
                let digits = words.next().unwrap_or("");
                let bytes = (0..digits.len() / 2)
                    .map(|i| hex(&digits[2 * i..2 * i + 2]).map(|b| b as u8))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(fail)?;
                update.ram = Some(bytes);
            }
            (Some("w"), Some(update)) => {
                let register = hex(words.next().unwrap_or("")).map_err(fail)? as u16;
                let value = hex(words.next().unwrap_or("")).map_err(fail)? as u8;
                update.writes.push((register, value));
            }
            (Some("g"), _) => {
                let frame = words
                    .next()
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| fail("frame".into()))?;
                let register = hex(words.next().unwrap_or("")).map_err(fail)? as u16;
                let value = hex(words.next().unwrap_or("")).map_err(fail)? as u8;
                if matches!(register, 0x4000..=0x4013 | 0x4015 | 0x4017) {
                    capture.other_writes.push((frame, register, value));
                }
            }
            (Some("end"), _) => capture.end = words.next().and_then(|f| f.parse().ok()),
            (None, _) => {}
            (Some(word), _) => return Err(fail(format!("unexpected `{word}`"))),
        }
    }
    Ok(capture)
}

/// Where songs start, by song: tempo and the four track addresses (bank 4), read from the
/// ROM's tables (sound.md 3.1) to recognise song starts in a capture.
pub fn song_starts(rom: &Rom) -> Result<Vec<(u8, [u16; 4])>, String> {
    use crate::rom::sound::at;
    let byte = |offset: usize| {
        rom.byte(offset)
            .ok_or_else(|| format!("ROM 0x{offset:05X} is past the end"))
    };
    (0..SONG_COUNT)
        .map(|song| {
            let mut tracks = [0u16; 4];
            for (channel, track) in tracks.iter_mut().enumerate() {
                let index = song * 4 + channel;
                *track = u16::from_le_bytes([
                    byte(at::TRACKS_LOW + index)?,
                    byte(at::TRACKS_HIGH + index)?,
                ]);
            }
            Ok((byte(at::TEMPOS + song)?, tracks))
        })
        .collect()
}

/// What the game did to the driver between two updates, as recognised in a capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Happening {
    SongStart(usize),
    EffectStart(Channel, u8),
    RamEffect,
}

/// A frame whose register writes differ: the frame, the original's writes, ours.
pub type Mismatch = (u32, Vec<(u16, u8)>, Vec<(u16, u8)>);

/// The outcome of replaying a capture through our driver.
#[derive(Debug, Clone, Default)]
pub struct Comparison {
    pub updates: usize,
    pub matching: usize,
    /// The first mismatches: frame, original's writes, ours.
    pub mismatches: Vec<Mismatch>,
    pub happenings: Vec<(u32, Happening)>,
    /// Driver RAM the game changed that we could not explain: frame and addresses.
    pub unexplained: Vec<(u32, Vec<u16>)>,
}

/// Replays a capture through our driver: the song and effect starts seen in the original's
/// RAM between updates are made on ours, then each update's register writes are compared.
pub fn compare(data: &SoundData, starts: &[(u8, [u16; 4])], capture: &Capture) -> Comparison {
    let mut driver = Driver::new(data.clone());
    let mut result = Comparison::default();
    let mut ours = Vec::new();
    for update in &capture.updates {
        if update.ram.is_some() {
            recognise(&mut driver, starts, update, &mut result);
        }
        ours.clear();
        driver.update(&mut ours);
        let ours: Vec<(u16, u8)> = ours.iter().map(|w| (w.register, w.value)).collect();
        result.updates += 1;
        if ours == update.writes {
            result.matching += 1;
        } else if result.mismatches.len() < 20 {
            result
                .mismatches
                .push((update.frame, update.writes.clone(), ours));
        }
    }
    result
}

fn recognise(
    driver: &mut Driver,
    starts: &[(u8, [u16; 4])],
    update: &Update,
    result: &mut Comparison,
) {
    let ram = |address: u16| update.ram(address).unwrap_or(0);
    let changed: BTreeSet<u16> = update.changes.iter().map(|&(a, _)| a).collect();
    let mut explained = BTreeSet::new();
    let per_channel = |base: &[u16]| -> Vec<u16> {
        (0..4u16)
            .flat_map(|k| base.iter().map(move |&a| a + 4 * k))
            .collect()
    };

    // The effect built in RAM: whatever the game put there.
    let ram_effect: Vec<u16> = (0x06E8..=0x06ED).collect();
    if ram_effect.iter().any(|a| changed.contains(a)) {
        let bytes: Vec<u8> = ram_effect.iter().map(|&a| ram(a)).collect();
        if let Ok(mut effect) = crate::rom::sound::parse_effect(RAM_EFFECT_ID, &bytes, |_| {}) {
            effect.in_ram = true;
            driver.set_ram_effect(effect);
            explained.extend(&ram_effect);
            result.happenings.push((update.frame, Happening::RamEffect));
        }
    }

    // A song start: the reset state of sound.md 5 with one song's tracks and tempo.
    let resets = per_channel(&[
        0x0638, 0x0639, 0x063A, 0x063B, 0x0688, 0x068B, 0x0698, 0x06D8,
    ]);
    if resets.iter().any(|a| changed.contains(a)) && ram(0xF8) == 0 {
        let song = starts.iter().position(|(tempo, tracks)| {
            *tempo == ram(0xF9)
                && tracks.iter().enumerate().all(|(k, &track)| {
                    let base = 4 * k as u16;
                    ram(0x0638 + base) == 1
                        && ram(0x0639 + base) == 1
                        && u16::from_le_bytes([ram(0x063A + base), ram(0x063B + base)]) == track
                })
        });
        if let Some(song) = song {
            driver.start_song(song);
            explained.extend(&resets);
            explained.extend([0xF8, 0xF9, 0x0637]);
            result
                .happenings
                .push((update.frame, Happening::SongStart(song)));
        }
    }

    // Effect starts.
    for channel in Channel::ALL {
        let base = 4 * channel.index() as u16;
        let fields: Vec<u16> = [0x0648, 0x0649, 0x064A, 0x064B, 0x0668, 0x0669, 0x066B]
            .iter()
            .map(|a| a + base)
            .collect();
        if fields.iter().any(|a| changed.contains(a)) {
            let id = ram(0x0648 + base);
            if id != 0 {
                driver.start_effect(channel, id);
                explained.extend(&fields);
                explained.extend([0x066A + base, 0x0698 + base]);
                result
                    .happenings
                    .push((update.frame, Happening::EffectStart(channel, id)));
            }
        }
    }

    let unexplained: Vec<u16> = changed
        .iter()
        .copied()
        // At the first update every byte counts as changed; zeros are the power-on state.
        .filter(|a| !explained.contains(a) && !(result.updates == 0 && ram(*a) == 0))
        .collect();
    if !unexplained.is_empty() {
        result.unexplained.push((update.frame, unexplained));
    }
}

/// Register writes as text, e.g. `4000=B8 4002=C4`.
pub fn writes_text(writes: &[(u16, u8)]) -> String {
    writes
        .iter()
        .map(|(register, value)| format!("{register:04X}={value:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_and_samples() {
        let bytes = wav(&[0.0, 1.0, -1.0, 2.0], 22_050);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            22_050
        );
        let samples: Vec<i16> = bytes[44..]
            .chunks(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(samples, [0, 32767, -32767, 32767]);
    }

    #[test]
    fn captures_parse() {
        let text = "g 2 4015 0F\ng 3 4016 01\nu 7\nx 00F8=00 0638=01\ns 0080\n\
                    w 4000 B8\nw 4002 C4\nu 8\nend 9\n";
        let capture = parse_capture(text).unwrap();
        assert_eq!(capture.other_writes, [(2, 0x4015, 0x0F)]);
        assert_eq!(capture.end, Some(9));
        assert_eq!(capture.updates.len(), 2);
        let first = &capture.updates[0];
        assert_eq!(first.changes, [(0xF8, 0), (0x638, 1)]);
        assert_eq!(first.ram(0xF9), Some(0x80));
        assert_eq!(first.ram(0x0637), None);
        assert_eq!(first.writes, [(0x4000, 0xB8), (0x4002, 0xC4)]);
        assert!(parse_capture("w 4000 B8").is_err());
        assert!(parse_capture("q").is_err());
    }

    #[test]
    fn requests_text() {
        let text = effect_requests(100, 10);
        assert!(
            text.starts_with("100 A 15 02;100 B 18 04;110 A 18 06;"),
            "{text}"
        );
    }
}
