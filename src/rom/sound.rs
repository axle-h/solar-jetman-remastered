//! The music and sound effects (`docs/spec/sound.md`, sections 2-4 and 9): decoded from bank 4
//! into our own event format, which the driver in `src/game` plays. The data is ripped (it goes
//! into the player's asset pack, never into git); the tables that say where it is (track and
//! effect pointers) are read here and leave no trace in the output, and the note period table
//! goes into the pack's `game/sound.ron` with the level data ([`crate::rom::extract`]).
//!
//! # Pack files
//!
//! `sound/song-NN.ron`, one per song 0-33: a [`Song`] with the song's tempo and four tracks
//! (pulse 1, pulse 2, triangle, noise). A track is a list of [`Event`]s read from index 0:
//!
//! - `Note(key, ticks)`: the note byte less $80 (1-127, transposed when played) and its
//!   duration in ticks (0 = 256). Durations are resolved here: the fixed-duration commands
//!   ($06, $07) only change how the bytes are read, so they leave no event.
//! - `Rest(ticks)`: note byte $80.
//! - `End`, `Jump(index)`, `Call(times: n, to: index)`, `Return`: the flow commands $00,
//!   $01, $04, $05. Addresses become event indexes in the same track. A block of bytes read
//!   with two fixed durations (sound.md 9) is decoded once for each; where a decoded stretch
//!   runs into one decoded before, a `Jump` joins them (it takes no time). The fixed duration
//!   carries through returns, so a repeated block that changes it reads differently on its
//!   next pass (song 4, pulse 2): such a call becomes one-pass calls of each reading, then a
//!   call for the remaining passes once the reading repeats.
//! - `ClearEffectLock` ($02), and one event per remaining command ($03, $08-$16), with the
//!   command's operands as named fields.
//!
//! `sound/effects.ron`: [`Effects`](crate::game::sound::data::Effects), every effect id $02-$44 with its header split into
//! fields and its steps as `Tone(volume, period)` or `ClearLock`; the end marker is the end of
//! the list. Effect $1A is the one the game builds in RAM, imported from its template in bank 0
//! (`in_ram: true`).

use crate::game::sound::data::{
    Effect, EffectStep, Event, LAST_EFFECT_ID, NOTE_PERIODS, RAM_EFFECT_ID, SONG_COUNT, Song, Track,
};
use crate::rom::cartridge::{Rom, bank_offset};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Where the sound tables and data are (sound.md 2), as PRG ROM offsets.
pub mod at {
    /// Tempo of each song: ripped.
    pub const TEMPOS: usize = 0x22320;
    /// Note periods, 61 little-endian words: committed.
    pub const NOTE_PERIODS: usize = 0x22847;
    /// Effect start addresses by id (a word per even id $00-$44).
    pub const EFFECT_POINTERS: usize = 0x228C1;
    /// Track start addresses, low then high bytes, by song × 4 + channel.
    pub const TRACKS_LOW: usize = 0x22907;
    pub const TRACKS_HIGH: usize = 0x2298F;
    /// The music and effect data reached from the pointers: ripped.
    pub const DATA: (usize, usize) = (0x22A17, 0x27E4C);
    /// The 6-byte template of the effect built in RAM, in bank 0.
    pub const RAM_EFFECT_TEMPLATE: usize = 0x07273;
    /// Its RAM address, as the effect pointer table gives it.
    pub const RAM_EFFECT_ADDRESS: u16 = 0x06E8;
    /// The bank holding the sound driver and its data.
    pub const BANK: u8 = 4;
}

/// Length of the RAM effect's template.
pub const RAM_EFFECT_BYTES: usize = 6;

/// Songs and effects decoded, with the bytes of the data region that were read.
#[derive(Debug, Clone)]
pub struct DecodedSound {
    pub songs: Vec<Song>,
    pub effects: Vec<Effect>,
    /// One flag per byte of [`at::DATA`]: read by some song or effect.
    pub coverage: Vec<bool>,
}

/// Decodes all songs and effects.
pub fn decode(rom: &Rom) -> Result<DecodedSound, String> {
    let mut coverage = vec![false; at::DATA.1 - at::DATA.0];
    let mut songs = Vec::with_capacity(SONG_COUNT);
    for song in 0..SONG_COUNT {
        let tempo = byte(rom, at::TEMPOS + song)?;
        let mut tracks = Vec::with_capacity(4);
        for channel in 0..4 {
            let index = song * 4 + channel;
            let start = u16::from_le_bytes([
                byte(rom, at::TRACKS_LOW + index)?,
                byte(rom, at::TRACKS_HIGH + index)?,
            ]);
            let events = TrackDecoder::new(rom, &mut coverage)
                .decode(start)
                .map_err(|error| format!("song {song}, channel {channel}: {error}"))?;
            tracks.push(Track { events });
        }
        songs.push(Song { tempo, tracks });
    }
    let mut effects = Vec::new();
    for id in (2..=LAST_EFFECT_ID).step_by(2) {
        let pointer = at::EFFECT_POINTERS + usize::from(id);
        let address = u16::from_le_bytes([byte(rom, pointer)?, byte(rom, pointer + 1)?]);
        let effect = if address == at::RAM_EFFECT_ADDRESS {
            let template = rom
                .bytes(at::RAM_EFFECT_TEMPLATE, RAM_EFFECT_BYTES)
                .ok_or("the RAM effect template is past the end of the ROM")?;
            let mut effect = parse_effect(id, template, |_| {})?;
            effect.in_ram = true;
            effect
        } else {
            let offset = data_offset(address)?;
            let bytes = rom.prg().get(offset..at::DATA.1).unwrap_or_default();
            parse_effect(id, bytes, |position| {
                coverage[offset - at::DATA.0 + position] = true;
            })
            .map_err(|error| format!("effect ${id:02X} at ${address:04X}: {error}"))?
        };
        effects.push(effect);
    }
    if effects
        .iter()
        .filter(|e| e.in_ram)
        .map(|e| e.id)
        .ne([RAM_EFFECT_ID])
    {
        return Err(format!(
            "effect ${RAM_EFFECT_ID:02X} is not the only one built in RAM"
        ));
    }
    Ok(DecodedSound {
        songs,
        effects,
        coverage,
    })
}

/// The note period table (committed data).
pub fn note_periods(rom: &Rom) -> Result<Vec<u16>, String> {
    let bytes = rom
        .bytes(at::NOTE_PERIODS, 2 * NOTE_PERIODS)
        .ok_or("the note period table is past the end of the ROM")?;
    Ok(bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

fn byte(rom: &Rom, offset: usize) -> Result<u8, String> {
    rom.byte(offset)
        .ok_or_else(|| format!("ROM 0x{offset:05X} is past the end"))
}

/// The ROM offset of a bank 4 address inside the sound data region.
fn data_offset(address: u16) -> Result<usize, String> {
    let offset = bank_offset(at::BANK, address);
    if address < 0x8000 || !(at::DATA.0..at::DATA.1).contains(&offset) {
        return Err(format!(
            "${address:04X} is outside the sound data (bank 4 $AA17-$FE4B)"
        ));
    }
    Ok(offset)
}

/// Reads an effect (sound.md 4.2) from `bytes`, calling `read` with the position of every byte
/// it reads.
pub fn parse_effect(id: u8, bytes: &[u8], mut read: impl FnMut(usize)) -> Result<Effect, String> {
    let mut at = 0;
    let mut next = || -> Result<u8, String> {
        let value = *bytes.get(at).ok_or("the effect runs past its data")?;
        read(at);
        at += 1;
        Ok(value)
    };
    let first = next()?;
    let sweep = next()?;
    let high = next()?;
    let mut steps = Vec::new();
    loop {
        match next()? {
            0x00 => break,
            0x02 => steps.push(EffectStep::ClearLock),
            command @ 0x01..=0x0F => {
                return Err(format!("command ${command:02X} in an effect"));
            }
            a => {
                let b = next()?;
                steps.push(EffectStep::Tone(
                    a >> 4,
                    u16::from(a & 7) << 8 | u16::from(b),
                ));
            }
        }
    }
    Ok(Effect {
        id,
        in_ram: false,
        control: first & 0xF0,
        frames_per_step: first & 0x0F,
        sweep,
        high,
        steps,
    })
}

/// Where a decoded event points before indexes are known: a bank 4 address read with a fixed
/// duration (0 = off).
type Key = (u16, u8);

/// Decodes one track into a flat event list.
struct TrackDecoder<'a> {
    rom: &'a Rom,
    coverage: &'a mut Vec<bool>,
    events: Vec<Event>,
    /// Index of the event decoded from each key.
    decoded: HashMap<Key, usize>,
    /// Events whose target is filled in at the end: (event index, target).
    targets: Vec<(usize, Key)>,
    /// Keys still to decode, in the order they were met.
    pending: VecDeque<Key>,
    /// Exit fixed duration of each called block, by its entry key.
    exits: BTreeMap<Key, Option<u8>>,
}

/// What one command does to the decoding.
enum Step {
    /// An event; decoding continues after it.
    Event(Event),
    /// No event (fixed duration on or off).
    Nothing,
    /// An event after which nothing follows in this stretch (end, jump, return).
    Last(Event),
    /// A call: repeats (0 = 256) and the called block.
    Call(u8, Key),
}

impl<'a> TrackDecoder<'a> {
    fn new(rom: &'a Rom, coverage: &'a mut Vec<bool>) -> TrackDecoder<'a> {
        TrackDecoder {
            rom,
            coverage,
            events: Vec::new(),
            decoded: HashMap::new(),
            targets: Vec::new(),
            pending: VecDeque::new(),
            exits: BTreeMap::new(),
        }
    }

    fn decode(mut self, start: u16) -> Result<Vec<Event>, String> {
        self.pending.push_back((start, 0));
        while let Some(key) = self.pending.pop_front() {
            if !self.decoded.contains_key(&key) {
                self.stretch(key)?;
            }
        }
        for (index, key) in std::mem::take(&mut self.targets) {
            let target = u16::try_from(self.decoded[&key])
                .map_err(|_| "more than 65 535 events in a track".to_owned())?;
            match &mut self.events[index] {
                Event::Jump(to) | Event::Call { to, .. } => *to = target,
                _ => unreachable!("only jumps and calls have targets"),
            }
        }
        Ok(self.events)
    }

    fn read(&mut self, address: u16) -> Result<u8, String> {
        let offset = data_offset(address)?;
        self.coverage[offset - at::DATA.0] = true;
        Ok(self.rom.prg()[offset])
    }

    /// Decodes events from `key` on until the stretch ends or joins decoded events.
    fn stretch(&mut self, (mut address, mut fixed): Key) -> Result<(), String> {
        loop {
            if self.decoded.contains_key(&(address, fixed)) {
                let joined = self.events.len();
                self.events.push(Event::Jump(0));
                self.targets.push((joined, (address, fixed)));
                return Ok(());
            }
            let index = self.events.len();
            let (step, next, next_fixed) = self.command(address, fixed)?;
            // Every event starts at a key; a command without an event ($06, $07) shares the
            // key of the event after it.
            self.decoded.insert((address, fixed), index);
            match step {
                Step::Nothing => fixed = next_fixed,
                Step::Event(event) => self.events.push(event),
                Step::Last(event) => {
                    self.events.push(event);
                    return Ok(());
                }
                Step::Call(times, target) => match self.call(times, target)? {
                    // Decoding goes on with the fixed duration the called block returns with.
                    Some(exit) => fixed = exit,
                    // The block never returns: nothing after the call is reached.
                    None => return Ok(()),
                },
            }
            address = next;
        }
    }

    /// Decodes the command at `address`: the step, the address after it and the fixed
    /// duration after it.
    fn command(&mut self, address: u16, fixed: u8) -> Result<(Step, u16, u8), String> {
        let mut at = address;
        let mut next = |this: &mut Self| -> Result<u8, String> {
            let value = this.read(at)?;
            at = at.wrapping_add(1);
            Ok(value)
        };
        let code = next(self)?;
        let mut operands = |this: &mut Self, count: usize| -> Result<Vec<u8>, String> {
            (0..count).map(|_| next(this)).collect()
        };
        let mut fixed_after = fixed;
        let step = match code {
            0x80..=0xFF => {
                let ticks = if fixed == 0 {
                    operands(self, 1)?[0]
                } else {
                    fixed
                };
                Step::Event(if code == 0x80 {
                    Event::Rest(ticks)
                } else {
                    Event::Note(code & 0x7F, ticks)
                })
            }
            0x00 => Step::Last(Event::End),
            0x01 => {
                let o = operands(self, 2)?;
                let target = (u16::from_le_bytes([o[0], o[1]]), fixed);
                self.targets.push((self.events.len(), target));
                self.pending.push_back(target);
                Step::Last(Event::Jump(0))
            }
            0x02 => {
                // Sound.md 4.4: the next byte is read under effect rules; in the data it is
                // always a command $01-$0F, which then runs as a music command.
                let following = self.read(at)?;
                if !(0x01..=0x0F).contains(&following) {
                    return Err(format!(
                        "command $02 at ${address:04X} is followed by ${following:02X}"
                    ));
                }
                Step::Event(Event::ClearEffectLock)
            }
            0x03 => {
                let o = operands(self, 3)?;
                Step::Event(Event::Setup {
                    control: o[0],
                    sweep: o[1],
                    high: o[2],
                })
            }
            0x04 => {
                let o = operands(self, 3)?;
                Step::Call(o[0], (u16::from_le_bytes([o[1], o[2]]), fixed))
            }
            0x05 => Step::Last(Event::Return),
            0x06 => {
                fixed_after = operands(self, 1)?[0];
                Step::Nothing
            }
            0x07 => {
                fixed_after = 0;
                Step::Nothing
            }
            0x08 => {
                let o = operands(self, 3)?;
                Step::Event(Event::Vibrato {
                    length: o[0],
                    rate: o[1],
                    depth: o[2],
                })
            }
            0x09 => Step::Event(Event::VibratoOff),
            0x0A | 0x0B => {
                let o = operands(self, 5)?;
                Step::Event(Event::Slide {
                    up: code == 0x0A,
                    delay: o[0],
                    rate: o[1],
                    steps: o[2],
                    amount: o[3],
                    back: o[4],
                })
            }
            0x0C => Step::Event(Event::Volume(operands(self, 1)?[0])),
            0x0D | 0x16 => {
                let o = operands(self, 3)?;
                let (delay, rate, target) = (o[0], o[1], o[2]);
                Step::Event(if code == 0x0D {
                    Event::VolumeRamp {
                        delay,
                        rate,
                        target,
                    }
                } else {
                    Event::Retrigger {
                        delay,
                        rate,
                        target,
                    }
                })
            }
            0x0E => {
                let o = operands(self, 3)?;
                Step::Event(Event::VolumeWave {
                    half: o[0],
                    rate: o[1],
                    delta: o[2],
                })
            }
            0x0F => Step::Event(Event::VolumeEffectOff),
            0x10 => Step::Event(Event::Tempo(operands(self, 1)?[0])),
            0x11 => Step::Event(Event::TempoAdd(operands(self, 1)?[0])),
            0x12 => Step::Event(Event::Transpose(operands(self, 1)?[0])),
            0x13 => Step::Event(Event::TransposeAdd(operands(self, 1)?[0])),
            0x14 => Step::Event(Event::TransposeAll(operands(self, 1)?[0])),
            0x15 => Step::Event(Event::RetriggerOff),
            _ => return Err(format!("unknown command ${code:02X} at ${address:04X}")),
        };
        Ok((step, at, fixed_after))
    }

    /// Emits a call of the block at `target` repeated `times` (0 = 256) and returns the fixed
    /// duration after it, or `None` if the block never returns. The fixed duration carries
    /// through returns, so when the block changes it, the next pass reads the block another
    /// way: such passes become calls of one repeat each, until a pass leaves the fixed duration
    /// as it found it, and the rest are one call.
    fn call(&mut self, times: u8, (address, mut fixed): Key) -> Result<Option<u8>, String> {
        let mut remaining = if times == 0 { 256 } else { u16::from(times) };
        loop {
            let exit = self.exit_of((address, fixed))?;
            let settled = exit.is_none_or(|exit| exit == fixed);
            let repeats = if settled { remaining } else { 1 };
            self.targets.push((self.events.len(), (address, fixed)));
            self.pending.push_back((address, fixed));
            self.events.push(Event::Call {
                times: (repeats & 0xFF) as u8,
                to: 0,
            });
            remaining -= repeats;
            match exit {
                Some(exit) if remaining > 0 => fixed = exit,
                _ => return Ok(exit),
            }
        }
    }

    /// The fixed duration a called block returns with, or `None` if it never returns. Calls
    /// inside a called block are not supported (sound.md 4.1: one level only; the data has
    /// none).
    fn exit_of(&mut self, entry: Key) -> Result<Option<u8>, String> {
        if let Some(&exit) = self.exits.get(&entry) {
            return Ok(exit);
        }
        let (mut address, mut fixed) = entry;
        let mut seen = Vec::new();
        let exit = loop {
            if seen.contains(&(address, fixed)) {
                break None; // loops for ever without returning
            }
            seen.push((address, fixed));
            // Peek without recording targets or coverage side effects that matter: a scratch
            // decoder reads the same bytes.
            let mut scratch = TrackDecoder::new(self.rom, self.coverage);
            let (step, next, next_fixed) = scratch.command(address, fixed)?;
            match step {
                Step::Last(Event::Return) => break Some(fixed),
                Step::Last(Event::End) => break None,
                Step::Last(Event::Jump(_)) => {
                    let target = scratch.targets[0].1;
                    (address, fixed) = target;
                }
                Step::Call(..) => {
                    return Err(format!(
                        "a call inside the called block at ${:04X}",
                        entry.0
                    ));
                }
                _ => (address, fixed) = (next, next_fixed),
            }
        };
        self.exits.insert(entry, exit);
        Ok(exit)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rom::import::tests::synthetic_rom;

    /// Writes bytes at a bank 4 address of a ROM.
    fn put(prg: &mut [u8], address: u16, bytes: &[u8]) {
        let offset = bank_offset(4, address);
        prg[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    /// Made-up sound data in a synthetic ROM: every track an end of track at $B000, every
    /// effect a made-up one at $AB00, and effect $1A in RAM.
    pub(crate) fn add_sound(prg: &mut [u8]) {
        put(prg, 0xB000, &[0x00]);
        for index in 0..4 * SONG_COUNT {
            prg[at::TRACKS_LOW + index] = 0x00;
            prg[at::TRACKS_HIGH + index] = 0xB0;
        }
        put(
            prg,
            0xAB00,
            &[0x31, 0x00, 0x08, 0x5F, 0x12, 0x02, 0x40, 0x01, 0x00],
        );
        for id in (0..=LAST_EFFECT_ID as usize).step_by(2) {
            prg[at::EFFECT_POINTERS + id] = 0x00;
            prg[at::EFFECT_POINTERS + id + 1] = 0xAB;
        }
        prg[at::EFFECT_POINTERS + 0x1A] = 0xE8;
        prg[at::EFFECT_POINTERS + 0x1B] = 0x06;
        prg[at::RAM_EFFECT_TEMPLATE..at::RAM_EFFECT_TEMPLATE + 6]
            .copy_from_slice(&[0x84, 0x00, 0x08, 0x80, 0x20, 0x00]);
    }

    /// The synthetic ROM with `track` as song 1's pulse 1 track, at $C000.
    fn rom_with(track: &[u8]) -> Rom {
        let mut prg = synthetic_rom().prg().to_vec();
        prg[at::TRACKS_LOW + 4] = 0x00;
        prg[at::TRACKS_HIGH + 4] = 0xC0;
        put(&mut prg, 0xC000, track);
        Rom::unchecked(prg)
    }

    #[test]
    fn spec_example_track_decodes() {
        let rom = rom_with(&[
            0x03, 0xB7, 0x43, 0x9A, 0x0C, 0x08, 0x98, 0x04, 0x80, 0x02, 0x00,
        ]);
        let sound = decode(&rom).unwrap();
        assert_eq!(
            sound.songs[1].tracks[0].events,
            [
                Event::Setup {
                    control: 0xB7,
                    sweep: 0x43,
                    high: 0x9A
                },
                Event::Volume(0x08),
                Event::Note(0x18, 4),
                Event::Rest(2),
                Event::End,
            ]
        );
        assert_eq!(sound.songs[0].tracks[0].events, [Event::End]);
        assert_eq!(sound.songs.len(), SONG_COUNT);
    }

    #[test]
    fn fixed_durations_calls_and_loops() {
        // $C000: fixed 3; call $C010 twice; jump back to $C003 (the call).
        // $C010: a note, $02 then a set volume, return.
        let mut track = vec![0x06, 0x03, 0x04, 0x02, 0x10, 0xC0, 0x01, 0x02, 0xC0];
        track.resize(0x10, 0xFF);
        track.extend([0x85, 0x02, 0x0C, 0x05, 0x05]);
        let rom = rom_with(&track);
        let events = &decode(&rom).unwrap().songs[1].tracks[0].events;
        assert_eq!(
            *events,
            [
                Event::Call { times: 2, to: 2 },
                Event::Jump(0),
                Event::Note(0x05, 3),
                Event::ClearEffectLock,
                Event::Volume(0x05),
                Event::Return,
            ]
        );
    }

    #[test]
    fn a_block_read_with_two_durations_is_decoded_twice() {
        // Main: call $C020 (durations carried), fixed 2, jump to $C020, which then reads the
        // same bytes as notes without durations.
        let mut track = vec![0x04, 0x01, 0x20, 0xC0, 0x06, 0x02, 0x01, 0x20, 0xC0];
        track.resize(0x20, 0xFF);
        track.extend([0x81, 0x02, 0x0C, 0x05, 0x05]);
        let rom = rom_with(&track);
        let events = &decode(&rom).unwrap().songs[1].tracks[0].events;
        assert_eq!(
            *events,
            [
                Event::Call { times: 1, to: 2 },
                Event::Jump(5),
                Event::Note(0x01, 2),
                Event::Volume(0x05),
                Event::Return,
                Event::Note(0x01, 2),
                Event::ClearEffectLock,
                Event::Volume(0x05),
                Event::Return,
            ]
        );
    }

    #[test]
    fn a_repeated_block_that_changes_the_fixed_duration_is_unrolled() {
        // Call $C010 three times; the block turns fixed duration 4 on, and reads its first
        // note with a duration byte only on the first pass.
        let mut track = vec![0x04, 0x03, 0x10, 0xC0, 0x00];
        track.resize(0x10, 0xFF);
        track.extend([0x81, 0x02, 0x0C, 0x05, 0x06, 0x04, 0x05]);
        let events = &decode(&rom_with(&track)).unwrap().songs[1].tracks[0].events;
        assert_eq!(
            *events,
            [
                Event::Call { times: 1, to: 3 },
                Event::Call { times: 2, to: 6 },
                Event::End,
                Event::Note(0x01, 2),
                Event::Volume(0x05),
                Event::Return,
                Event::Note(0x01, 4),
                Event::ClearEffectLock,
                Event::Volume(0x05),
                // The return was decoded with fixed duration 4 by the first pass.
                Event::Jump(5),
            ]
        );
    }

    #[test]
    fn effects_decode_with_the_ram_template() {
        let rom = rom_with(&[0x00]);
        let sound = decode(&rom).unwrap();
        assert_eq!(sound.effects.len(), 34);
        let first = &sound.effects[0];
        assert_eq!(
            (
                first.id,
                first.control,
                first.frames_per_step,
                first.sweep,
                first.high
            ),
            (0x02, 0x30, 1, 0x00, 0x08)
        );
        assert_eq!(
            first.steps,
            [
                EffectStep::Tone(5, 0x712),
                EffectStep::ClearLock,
                EffectStep::Tone(4, 0x001)
            ]
        );
        let hum = sound.effects.iter().find(|e| e.in_ram).unwrap();
        assert_eq!(hum.id, RAM_EFFECT_ID);
        assert_eq!(hum.steps, [EffectStep::Tone(8, 0x020)]);
    }

    #[test]
    fn bad_data_is_reported() {
        assert!(decode(&rom_with(&[0x20])).is_err());
        assert!(decode(&rom_with(&[0x02, 0x40])).is_err());
        // A call inside a called block.
        let mut track = vec![0x04, 0x02, 0x10, 0xC0, 0x00];
        track.resize(0x10, 0xFF);
        track.extend([0x04, 0x01, 0x10, 0xC0, 0x05]);
        assert!(decode(&rom_with(&track)).is_err());
        assert!(parse_effect(2, &[0x10, 0, 0, 0x03, 0], |_| {}).is_err());
        assert!(parse_effect(2, &[0x10, 0, 0, 0x30], |_| {}).is_err());
    }
}
