//! Sound: the game's sound driver (`docs/spec/sound.md`) and what the game asks of it.
//!
//! - [`data`]: the songs, effects and note periods, in our own format.
//! - [`driver`]: the per-frame driver, producing APU register writes.
//! - [`Sound`]: the driver with the game's side (sound.md 8): song starts, the two effect
//!   request slots and their lock, pause, and the power-on register writes.
//! - [`player`]: [`Sound`] feeding `src/apu` on the frame clock, for the app and tools.
//! - [`jukebox`]: picking songs and effects to listen to, for the viewer.

pub mod data;
pub mod driver;
pub mod jukebox;
pub mod player;

pub use data::{Effect, EffectStep, Event, Song, SoundData, SoundTables, Track};
pub use driver::{Channel, Driver, RegisterWrite};

/// The song that stops the music: its four tracks end at once (sound.md 8.1).
pub const SILENCE: usize = 0;

/// One of the two effect request slots (sound.md 8.2): A is serviced on even frames, B on odd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Slot {
    A,
    B,
}

/// The driver and the game's requests to it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Sound {
    driver: Driver,
    /// Slot A, slot B.
    slots: [Option<(Channel, u8)>; 2],
    /// RAM $F7: while non-zero, servicing counts it down and drops requests until it goes
    /// below $80.
    lock: u8,
    /// RAM $5D, low bit: which slot is serviced this frame.
    frame: u8,
    paused: bool,
    /// The demo drops requests made through the slots.
    demo: bool,
    /// The effects that started (channel, id), kept when recording is on, for the validation.
    started: Option<Vec<(Channel, u8)>>,
}

/// What the game asks of the sound driver (sound.md 8): song starts, effect requests in the
/// two slots, direct effect starts, the request lock and the RAM effect's pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Request {
    /// Start a song: unless it was the last one started this way, or always (sound.md 8.1).
    Song { id: u8, always: bool },
    /// An effect request put in a slot, serviced on a later frame (sound.md 8.2).
    Slot {
        slot: Slot,
        channel: Channel,
        id: u8,
    },
    /// An effect started at once, without a slot.
    Direct { channel: Channel, id: u8 },
    /// The request lock, RAM $F7 (sound.md 8.2).
    Lock(u8),
    /// The period-low byte of the RAM effect's step (sound.md 8.3, tractor-beam hum).
    RamPitch(u8),
}

impl Request {
    /// A request in slot A.
    pub const fn a(channel: Channel, id: u8) -> Request {
        Request::Slot {
            slot: Slot::A,
            channel,
            id,
        }
    }

    /// A request in slot B.
    pub const fn b(channel: Channel, id: u8) -> Request {
        Request::Slot {
            slot: Slot::B,
            channel,
            id,
        }
    }
}

impl Sound {
    pub fn new(data: SoundData) -> Sound {
        Sound {
            driver: Driver::new(data),
            slots: [None; 2],
            lock: 0,
            frame: 0,
            paused: false,
            demo: false,
            started: None,
        }
    }

    /// After a load: takes the imported data back (a saved state holds none).
    pub(crate) fn rebind(&mut self, data: SoundData) {
        self.driver.rebind(data);
    }

    /// Starts or stops recording the effects that start (for the validation).
    pub fn record_starts(&mut self, on: bool) {
        self.started = on.then(Vec::new);
        self.driver.record_clears(on);
    }

    /// Takes the effects started since the last call.
    pub fn take_starts(&mut self) -> Vec<(Channel, u8)> {
        self.started
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Does what the game asked.
    pub fn apply(&mut self, request: Request) {
        match request {
            Request::Song { id, always } => {
                if always {
                    self.play_song(usize::from(id));
                } else {
                    self.play_song_if_new(id);
                }
            }
            Request::Slot { slot, channel, id } => self.request_effect(slot, channel, id),
            Request::Direct { channel, id } => self.start_effect(channel, id),
            Request::Lock(value) => self.lock_requests(value),
            Request::RamPitch(low) => self.driver.set_ram_effect_pitch(low),
        }
    }

    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    pub fn driver_mut(&mut self) -> &mut Driver {
        &mut self.driver
    }

    /// The writes made once at power-on (sound.md 1): $10 to $4017 down to $4000, then the DMC
    /// control, the 5-step frame counter sequence and the four channels enabled.
    pub fn power_on(&mut self, out: &mut Vec<RegisterWrite>) {
        for register in (0x4000..=0x4017).rev() {
            out.push(RegisterWrite {
                register,
                value: 0x10,
            });
        }
        out.extend([
            RegisterWrite {
                register: 0x4010,
                value: 0xC0,
            },
            RegisterWrite {
                register: 0x4017,
                value: 0x80,
            },
            RegisterWrite {
                register: 0x4015,
                value: 0x0F,
            },
        ]);
    }

    /// Starts `song` whatever is playing.
    pub fn play_song(&mut self, song: usize) {
        self.driver.start_song(song);
    }

    /// Starts `song` unless it was the last one started this way.
    pub fn play_song_if_new(&mut self, song: u8) {
        self.driver.start_song_if_new(song);
    }

    /// Starts `song` unless it is playing, and again whenever it ends by itself, until another
    /// song starts.
    pub fn play_song_looped(&mut self, song: u8) {
        self.driver.start_song_looped(song);
    }

    /// Stops the music, as the game does: by starting song 0.
    pub fn stop_music(&mut self) {
        self.driver.start_song(SILENCE);
    }

    /// Puts a request in a slot; a slot written again before it is serviced keeps only the
    /// last request.
    pub fn request_effect(&mut self, slot: Slot, channel: Channel, id: u8) {
        self.slots[slot as usize] = Some((channel, id));
    }

    /// Requests an effect on the channels the game uses for it (slot A for the first channel,
    /// B for the second), as listed in [`EFFECTS`]. Unknown ids are ignored.
    pub fn request_effect_as_the_game_does(&mut self, id: u8) {
        if let Some(effect) = EFFECTS.iter().find(|e| e.id == id) {
            for (&(channel, id), slot) in effect.starts.iter().zip([Slot::A, Slot::B]) {
                self.request_effect(slot, channel, id);
            }
        }
    }

    /// Starts an effect at once, without a slot (the shot and the text printer's click do).
    pub fn start_effect(&mut self, channel: Channel, id: u8) {
        if self.driver.start_effect(channel, id)
            && let Some(started) = self.started.as_mut()
        {
            started.push((channel, id));
        }
    }

    /// Sets the request lock (RAM $F7); the game sets $81 with its request of effect $32.
    pub fn lock_requests(&mut self, value: u8) {
        self.lock = value;
    }

    /// In the demo, requests through the slots are dropped.
    pub fn set_demo(&mut self, demo: bool) {
        self.demo = demo;
    }

    /// Pauses: silences the APU; the driver stops until [`Sound::resume`].
    pub fn pause(&mut self, out: &mut Vec<RegisterWrite>) {
        self.paused = true;
        out.push(RegisterWrite {
            register: 0x4015,
            value: 0x00,
        });
    }

    pub fn resume(&mut self, out: &mut Vec<RegisterWrite>) {
        self.paused = false;
        out.push(RegisterWrite {
            register: 0x4015,
            value: 0x0F,
        });
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// One video frame: services one request slot, then runs the driver unless paused.
    pub fn frame(&mut self, out: &mut Vec<RegisterWrite>) {
        self.service();
        self.update(out);
    }

    /// The driver's step alone, unless paused. The game's frame is an update, then its pass,
    /// which begins with [`Sound::service`] (game-flow.md 1.1).
    pub fn update(&mut self, out: &mut Vec<RegisterWrite>) {
        if !self.paused {
            self.driver.update(out);
            // The effects' own lock-clearing command rewrites the effect byte (effect $3A).
            let clears = self.driver.take_clears();
            if let Some(started) = self.started.as_mut() {
                started.extend(clears);
            }
        }
    }

    /// Sets the frame counter whose low bit picks the slot (RAM $5D) when the caller keeps
    /// the game's own count, which loading frames advance.
    pub fn set_frame(&mut self, frame: u8) {
        self.frame = frame;
    }

    /// Services one request slot (A on even frames, B on odd ones) and counts the frame.
    pub fn service(&mut self) {
        self.service_request();
        self.frame = self.frame.wrapping_add(1);
    }

    fn service_request(&mut self) {
        if self.lock != 0 {
            self.lock -= 1;
            if self.lock & 0x80 == 0 {
                self.slots = [None; 2];
                return;
            }
        }
        let slot = usize::from(self.frame & 1);
        if let Some((channel, id)) = self.slots[slot].take()
            && !self.demo
        {
            self.start_effect(channel, id);
        }
    }
}

/// How the game plays an effect: the (channel, id) pairs it starts together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectUse {
    /// The first id; the list is ordered by it.
    pub id: u8,
    pub starts: &'static [(Channel, u8)],
    pub description: &'static str,
}

const P1: Channel = Channel::Pulse1;
const P2: Channel = Channel::Pulse2;
const TRI: Channel = Channel::Triangle;
const NOISE: Channel = Channel::Noise;

/// The effects and the channels the game plays them on (sound.md 8.3; the shot and the
/// engine thrust as corrected by the analysts: $06 is the shot, $10 the thrust).
pub const EFFECTS: &[EffectUse] = &[
    use_of(0x02, &[(P1, 0x02), (P2, 0x04)], "explosion"),
    use_of(0x06, &[(P2, 0x06)], "shot"),
    use_of(0x08, &[(P1, 0x08), (P2, 0x08)], "unidentified"),
    use_of(0x0A, &[(NOISE, 0x0A)], "object hit"),
    use_of(0x0C, &[(NOISE, 0x0C)], "object destroyed"),
    use_of(0x0E, &[(P1, 0x0E)], "object collision"),
    use_of(0x10, &[(NOISE, 0x10)], "engine thrust"),
    use_of(0x12, &[(P1, 0x12), (P2, 0x14)], "tractor beam catches"),
    use_of(0x16, &[(P1, 0x16)], "unidentified"),
    use_of(0x18, &[(P2, 0x18)], "unidentified"),
    use_of(0x1A, &[(TRI, 0x1A)], "tractor-beam hum"),
    use_of(0x1C, &[(P1, 0x1C), (P2, 0x1C)], "warp level start"),
    use_of(0x1E, &[(P1, 0x1E), (P2, 0x1E)], "bonus count 1"),
    use_of(0x20, &[(P1, 0x20), (P2, 0x20)], "bonus count 2"),
    use_of(0x22, &[(P1, 0x22), (P2, 0x22)], "bonus count 3"),
    use_of(0x24, &[(P1, 0x24), (P2, 0x24)], "bonus count 4"),
    use_of(0x26, &[(P1, 0x26), (P2, 0x26)], "bonus count 5"),
    use_of(0x28, &[(P1, 0x28), (P2, 0x28)], "bonus count 6"),
    use_of(0x2A, &[(P1, 0x2A), (P2, 0x2A)], "bonus count 7"),
    use_of(0x2C, &[(P1, 0x2C), (P2, 0x2C)], "bonus count 8"),
    use_of(0x2E, &[(P1, 0x2E), (P2, 0x2E)], "bonus count 9"),
    // Spec gap (sound.md 8.3): $30 has no requester and its channel is a guess.
    use_of(0x30, &[(P1, 0x30)], "unused?"),
    use_of(0x32, &[(P1, 0x32)], "leaving a level?"),
    use_of(0x34, &[(NOISE, 0x34)], "unused?"),
    use_of(0x36, &[(NOISE, 0x36)], "unidentified"),
    use_of(0x38, &[(NOISE, 0x38)], "unused?"),
    use_of(0x3A, &[(NOISE, 0x3A)], "text printer"),
    use_of(0x3C, &[(P1, 0x3C), (P2, 0x3C)], "unidentified"),
    use_of(0x3E, &[(P1, 0x3E), (P2, 0x3E)], "pod hit"),
    use_of(0x40, &[(P1, 0x40), (P2, 0x40)], "unidentified"),
    use_of(0x42, &[(P1, 0x42)], "unidentified"),
    use_of(0x44, &[(P1, 0x44), (P2, 0x44)], "unidentified"),
];

const fn use_of(id: u8, starts: &'static [(Channel, u8)], description: &'static str) -> EffectUse {
    EffectUse {
        id,
        starts,
        description,
    }
}

/// What a song is for (sound.md 8.1).
pub fn song_description(song: usize) -> &'static str {
    match song {
        0 => "silence",
        1 => "title",
        2 => "pod destroyed",
        3 => "landing on a planet",
        4 => "mothership travelling",
        5 => "warp levels",
        6 => "unknown context",
        7..=29 if song % 2 == 1 => "approaching a planet",
        8..=30 => "planet",
        31 => "unknown context",
        32 => "probably unused",
        33 => "final level",
        _ => "",
    }
}

#[cfg(test)]
pub(crate) mod tests;
