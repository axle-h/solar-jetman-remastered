//! The game's sound driver (`docs/spec/sound.md`, sections 5-7): run once per video frame, it
//! advances the songs and sound effects and produces the APU register writes of that frame.
//!
//! State is kept per channel as the spec's section 5 lists it, including what carries over
//! from one song to the next. Where the original shares one byte between two uses (the volume
//! effect's ramp and oscillation), so do we, as the sharing is audible when a song switches
//! from one to the other.

use super::data::{Effect, EffectStep, Event, RAM_EFFECT_ID, SoundData};

/// The four channels the driver uses.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Channel {
    Pulse1,
    Pulse2,
    Triangle,
    Noise,
}

impl Channel {
    pub const ALL: [Channel; 4] = [
        Channel::Pulse1,
        Channel::Pulse2,
        Channel::Triangle,
        Channel::Noise,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The channel's first register: $4000, $4004, $4008 or $400C.
    pub fn first_register(self) -> u16 {
        0x4000 + 4 * self as u16
    }
}

/// One APU register write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegisterWrite {
    pub register: u16,
    pub value: u8,
}

/// Commands read without meeting a note before the driver gives up on a track: only a track
/// that loops without a note gets there (the original would hang).
const MAX_EVENTS_PER_NOTE: usize = 0x10000;

/// One channel's state (sound.md 5). Field comments give the original's RAM byte for channel 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChannelState {
    /// $0638.
    pub music_active: bool,
    /// $0639: ticks left on the current note.
    pub ticks_left: u8,
    /// $063A-$063B: index of the next event of the track.
    pub position: usize,
    /// $0648: id of the effect playing, 0 = none.
    pub effect_id: u8,
    /// Which effect's data plays (the id at its start, bit 0 clear).
    effect_data: u8,
    /// $0649: bits 4-7 frames left in this step, bits 0-3 frames per step − 1.
    pub effect_counter: u8,
    /// $064A-$064B: index of the next step.
    pub effect_position: usize,
    /// $0658, $0659, $065A, $065B.
    pub control: u8,
    pub sweep: u8,
    pub period_low: u8,
    pub period_high: u8,
    /// $0668, $0669, $066B: the effect's register shadows.
    pub effect_first: u8,
    pub effect_sweep: u8,
    pub effect_high: u8,
    /// $0678-$067A: the call being run (index of its event) and its repeats left.
    pub call_at: usize,
    pub repeats: u8,
    /// $067B (signed).
    pub vibrato_delta: u8,
    /// $0689: bit 4 constant-volume flag, bits 0-3 volume.
    pub volume: u8,
    /// $068A: ramp delay counter (ramp) or half period (oscillation).
    pub volume_timer: u8,
    /// $068B.
    pub transpose: u8,
    /// $0698: vibrato steps per half cycle, 0 = off.
    pub vibrato_length: u8,
    /// $0699, $069A, $069B.
    pub vibrato_left: u8,
    pub vibrato_interval: u8,
    pub vibrato_counter: u8,
    /// $06A8: bit 7 on, bits 0-6 start volume.
    pub retrigger: u8,
    /// $06AA.
    pub volume_counter: u8,
    /// $06AB, $06B8, $06B9, $06BA (signed), $06BB.
    pub slide_counter: u8,
    pub slide_steps: u8,
    pub slide_delay: u8,
    pub slide_delta: u8,
    pub slide_back: u8,
    /// $06C8: ramp target, or oscillation delta.
    pub volume_goal: u8,
    /// $06C9: initial ramp delay, or oscillation reversal counter.
    pub volume_reload: u8,
    /// $06CA: 0 none, 1 ramp, 2 oscillation.
    pub volume_mode: u8,
    /// $06CB.
    pub volume_interval: u8,
    /// $06D8: 0 = slide off.
    pub slide_interval: u8,
    /// Not the original's: jumps taken back to an earlier event since the song started (how
    /// often the track has looped), for tools.
    pub loops: u32,
}

impl ChannelState {
    fn period(&self) -> u16 {
        u16::from_le_bytes([self.period_low, self.period_high])
    }

    /// Adds a signed step to the 16-bit period, carrying into the length-index bits.
    fn add_to_period(&mut self, delta: u8) {
        let period = self.period().wrapping_add_signed(i16::from(delta as i8));
        [self.period_low, self.period_high] = period.to_le_bytes();
    }

    fn first_register_value(&self) -> u8 {
        (self.control & 0xE0) | self.volume
    }
}

/// The driver.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Driver {
    /// Not part of a saved state: [`Driver::rebind`] puts the imported data back.
    #[serde(skip)]
    data: SoundData,
    /// The effect built in RAM, whose pitch the game changes (sound.md 4.3). Saved as its
    /// pitch only; [`Driver::rebind`] rebuilds the effect from the data.
    #[serde(with = "ram_effect_saved")]
    ram_effect: Option<Effect>,
    /// RAM $F8, $F9.
    pub(super) accumulator: u8,
    pub(super) tempo: u8,
    /// The song whose tracks the channels read.
    song: usize,
    /// RAM $0637: the last song started with [`Driver::start_song_if_new`].
    last_requested: u8,
    /// A song that starts again when it ends by itself, until another song starts (an
    /// addition: the original plays its "once" songs once).
    #[serde(default)]
    looped: Option<u8>,
    channels: [ChannelState; 4],
    /// When recording: the effect bytes written by the effects' own lock-clearing command
    /// (channel, new id), for the validation.
    clears: Option<Vec<(Channel, u8)>>,
}

impl Driver {
    /// A driver at power-on: all state 0, no song.
    pub fn new(data: SoundData) -> Driver {
        let ram_effect = data.effect(RAM_EFFECT_ID).cloned();
        Driver {
            data,
            ram_effect,
            accumulator: 0,
            tempo: 0,
            song: 0,
            last_requested: 0,
            looped: None,
            channels: [ChannelState::default(); 4],
            clears: None,
        }
    }

    /// After a load: takes the imported data back (a saved state holds none) and rebuilds the
    /// RAM effect from it with the saved pitch.
    pub(super) fn rebind(&mut self, data: SoundData) {
        let pitch = self
            .ram_effect
            .as_ref()
            .and_then(|e| match e.steps.first() {
                Some(EffectStep::Tone(_, period)) => Some(*period),
                _ => None,
            });
        self.ram_effect = data.effect(RAM_EFFECT_ID).cloned();
        if let (Some(period), Some(EffectStep::Tone(_, own))) = (
            pitch,
            self.ram_effect
                .as_mut()
                .and_then(|effect| effect.steps.first_mut()),
        ) {
            *own = period;
        }
        self.data = data;
    }

    /// Starts or stops recording the writes of the effects' lock-clearing command.
    pub fn record_clears(&mut self, on: bool) {
        self.clears = on.then(Vec::new);
    }

    /// Takes the writes recorded since the last call.
    pub fn take_clears(&mut self) -> Vec<(Channel, u8)> {
        self.clears.as_mut().map(std::mem::take).unwrap_or_default()
    }

    pub fn data(&self) -> &SoundData {
        &self.data
    }

    pub fn channel(&self, channel: Channel) -> &ChannelState {
        &self.channels[channel.index()]
    }

    /// The song the channels read, and whether any channel is still playing it.
    pub fn song(&self) -> (usize, bool) {
        (self.song, self.channels.iter().any(|c| c.music_active))
    }

    pub fn tempo(&self) -> (u8, u8) {
        (self.tempo, self.accumulator)
    }

    /// Starts `song` (sound.md 8.1, "start always"). Out-of-range numbers are ignored.
    pub fn start_song(&mut self, song: usize) {
        let Some(data) = self.data.songs.get(song) else {
            return;
        };
        self.tempo = data.tempo;
        self.accumulator = 0;
        self.song = song;
        self.looped = None;
        for channel in &mut self.channels {
            channel.music_active = true;
            channel.ticks_left = 1;
            channel.position = 0;
            channel.transpose = 0;
            channel.vibrato_length = 0;
            channel.slide_interval = 0;
            channel.loops = 0;
        }
    }

    /// Starts `song` unless it is the last song started this way (sound.md 8.1, "start if
    /// different").
    pub fn start_song_if_new(&mut self, song: u8) {
        if song != self.last_requested {
            self.last_requested = song;
            self.start_song(usize::from(song));
        }
    }

    /// Starts `song` unless it is playing, and starts it again whenever it ends by itself until
    /// another song starts.
    pub fn start_song_looped(&mut self, song: u8) {
        if self.song() != (usize::from(song), true) {
            self.last_requested = song;
            self.start_song(usize::from(song));
        }
        self.looped = Some(song);
    }

    /// Starts effect `id` on `channel` (sound.md 7.1): nothing happens if that id is already
    /// playing there. Writes no register. Returns whether the effect started.
    pub fn start_effect(&mut self, channel: Channel, id: u8) -> bool {
        let state = &mut self.channels[channel.index()];
        if id == state.effect_id {
            return false;
        }
        let header = if id == 0 {
            // Spec gap (sound.md 3.2): id 0 has no data; starting it only stops the effect.
            None
        } else {
            let effect = if id & 0xFE == RAM_EFFECT_ID {
                self.ram_effect.as_ref()
            } else {
                self.data.effect(id)
            };
            // Ids without data are not requested by the game; ignore them.
            let Some(effect) = effect else { return false };
            Some((
                effect.control,
                effect.frames_per_step,
                effect.sweep,
                effect.high,
            ))
        };
        state.effect_id = id;
        state.effect_data = id & 0xFE;
        state.effect_position = 0;
        state.vibrato_length = 0;
        if let Some((control, frames, sweep, high)) = header {
            state.effect_counter = frames & 0x0F;
            state.effect_first = control & 0xF0;
            state.effect_sweep = sweep;
            state.effect_high = high;
        }
        true
    }

    /// Sets the period-low byte of the RAM effect's step (sound.md 8.3, tractor-beam hum).
    pub fn set_ram_effect_pitch(&mut self, period_low: u8) {
        if let Some(EffectStep::Tone(_, period)) = self
            .ram_effect
            .as_mut()
            .and_then(|effect| effect.steps.first_mut())
        {
            *period = (*period & 0x700) | u16::from(period_low);
        }
    }

    /// Replaces the RAM effect (as the game does when it copies its template, sound.md 4.3).
    pub fn set_ram_effect(&mut self, effect: Effect) {
        self.ram_effect = Some(effect);
    }

    /// One frame's update (sound.md 6): appends the register writes to `out`.
    pub fn update(&mut self, out: &mut Vec<RegisterWrite>) {
        if let Some(song) = self.looped
            && !self.song().1
        {
            self.start_song_looped(song);
        }
        let (sum, tick) = self.accumulator.overflowing_add(self.tempo);
        self.accumulator = sum;
        for channel in Channel::ALL {
            if self.channels[channel.index()].music_active {
                if tick {
                    self.music_tick(channel, out);
                } else {
                    self.music_effects(channel, out);
                }
            }
            if self.channels[channel.index()].effect_id != 0 {
                self.effect_update(channel, out);
            }
        }
    }

    fn music_tick(&mut self, channel: Channel, out: &mut Vec<RegisterWrite>) {
        let state = &mut self.channels[channel.index()];
        state.ticks_left = state.ticks_left.wrapping_sub(1);
        if state.ticks_left != 0 {
            return;
        }
        for _ in 0..MAX_EVENTS_PER_NOTE {
            let track = &self.data.songs[self.song].tracks[channel.index()].events;
            let state = &mut self.channels[channel.index()];
            let Some(&event) = track.get(state.position) else {
                // Past the end of a track (not in the data): stop like an end of track.
                self.end_of_track(channel, out);
                return;
            };
            state.position += 1;
            match event {
                Event::Note(key, ticks) => {
                    let index = key.wrapping_add(state.transpose) & 0x7F;
                    self.start_note(channel, index, ticks, out);
                    return;
                }
                Event::Rest(ticks) => {
                    self.start_note(channel, 0, ticks, out);
                    return;
                }
                Event::End => {
                    self.end_of_track(channel, out);
                    return;
                }
                Event::Jump(to) => {
                    if usize::from(to) < state.position {
                        state.loops += 1;
                    }
                    state.position = usize::from(to);
                }
                Event::Call { times, to } => {
                    state.call_at = state.position - 1;
                    state.repeats = times;
                    state.position = usize::from(to);
                }
                Event::Return => {
                    state.repeats = state.repeats.wrapping_sub(1);
                    match track.get(state.call_at) {
                        Some(&Event::Call { to, .. }) if state.repeats != 0 => {
                            state.position = usize::from(to);
                        }
                        // A return with no call made in this song does not occur in the data;
                        // it continues after the remembered event.
                        _ => state.position = state.call_at + 1,
                    }
                }
                Event::ClearEffectLock => state.effect_id &= 0xFE,
                Event::TransposeAll(value) => {
                    for other in &mut self.channels {
                        other.transpose = value;
                    }
                }
                Event::Tempo(value) => self.tempo = value,
                Event::TempoAdd(value) => self.tempo = self.tempo.wrapping_add(value),
                other => apply_setting(state, other),
            }
        }
        self.end_of_track(channel, out);
    }

    fn end_of_track(&mut self, channel: Channel, out: &mut Vec<RegisterWrite>) {
        self.channels[channel.index()].music_active = false;
        // Written even while an effect plays on the channel (sound.md 4.1, $00).
        out.push(RegisterWrite {
            register: channel.first_register(),
            value: 0x10,
        });
    }

    /// Starts a note of period-table `index` (0 = rest) lasting `ticks` (sound.md 6.2).
    fn start_note(&mut self, channel: Channel, index: u8, ticks: u8, out: &mut Vec<RegisterWrite>) {
        let state = &mut self.channels[channel.index()];
        if state.retrigger & 0x80 != 0 {
            state.volume = state.retrigger & 0x7F;
            state.volume_mode = 1;
            state.volume_timer = state.volume_reload;
        }
        state.vibrato_left = state.vibrato_length.div_ceil(2);
        if channel == Channel::Noise {
            state.period_low = match index {
                0 | 64 => 0,
                _ => index.wrapping_sub(65) & 0x7F,
            };
            state.period_high &= 0xF8;
        } else {
            // The data never goes past entry 60 (sound.md 4.1); anything beyond plays a rest.
            let period = self
                .data
                .note_periods
                .get(usize::from(index))
                .copied()
                .unwrap_or(0);
            state.period_low = (period & 0xFF) as u8;
            state.period_high = (state.period_high & 0xF8) | ((period >> 8) & 7) as u8;
        }
        if state.effect_id == 0 {
            full_write(channel, state, out);
        }
        state.ticks_left = ticks;
    }

    /// A frame that is not a tick frame (sound.md 6.3).
    fn music_effects(&mut self, channel: Channel, out: &mut Vec<RegisterWrite>) {
        let state = &mut self.channels[channel.index()];
        let high_before = state.period_high;
        slide(state);
        vibrato(state);
        volume_effect(state);
        if state.effect_id == 0 {
            let first = channel.first_register();
            out.push(RegisterWrite {
                register: first,
                value: state.first_register_value(),
            });
            out.push(RegisterWrite {
                register: first + 2,
                value: state.period_low,
            });
            if state.period_high != high_before {
                out.push(RegisterWrite {
                    register: first + 3,
                    value: state.period_high,
                });
            }
        }
    }

    /// Sound.md 7.2.
    fn effect_update(&mut self, channel: Channel, out: &mut Vec<RegisterWrite>) {
        let state = &mut self.channels[channel.index()];
        if state.effect_counter & 0xF0 != 0 {
            state.effect_counter -= 0x10;
            return;
        }
        let per_step = state.effect_counter & 0x0F;
        state.effect_counter = (per_step << 4) | per_step;
        let effect = if state.effect_data == RAM_EFFECT_ID {
            self.ram_effect.as_ref()
        } else {
            self.data.effect(state.effect_data)
        };
        let steps = effect.map_or(&[][..], |effect| &effect.steps[..]);
        let first = channel.first_register();
        loop {
            match steps.get(state.effect_position) {
                None => {
                    state.effect_id = 0;
                    if state.music_active {
                        full_write(channel, state, out);
                    } else {
                        out.push(RegisterWrite {
                            register: first,
                            value: 0x10,
                        });
                    }
                    return;
                }
                Some(EffectStep::ClearLock) => {
                    state.effect_id &= 0xFE;
                    state.effect_position += 1;
                    if let Some(clears) = self.clears.as_mut() {
                        clears.push((channel, state.effect_id));
                    }
                }
                Some(&EffectStep::Tone(volume, period)) => {
                    state.effect_position += 1;
                    state.effect_first = (state.effect_first & 0xF0) | (volume & 0x0F);
                    out.push(RegisterWrite {
                        register: first,
                        value: state.effect_first,
                    });
                    out.push(RegisterWrite {
                        register: first + 1,
                        value: state.effect_sweep,
                    });
                    let high = ((period >> 8) & 7) as u8;
                    if high != state.effect_high & 7 {
                        state.effect_high = (state.effect_high & 0xF8) | high;
                        out.push(RegisterWrite {
                            register: first + 3,
                            value: state.effect_high,
                        });
                    }
                    out.push(RegisterWrite {
                        register: first + 2,
                        value: (period & 0xFF) as u8,
                    });
                    return;
                }
            }
        }
    }
}

/// The commands that only change the channel's own settings (sound.md 4.1). Wherever a
/// command sets an interval, its counter is set to the same value.
fn apply_setting(state: &mut ChannelState, event: Event) {
    let retrigger_on = |state: &ChannelState| state.retrigger & 0x80 != 0;
    match event {
        Event::Setup {
            control,
            sweep,
            high,
        } => {
            state.control = control;
            state.volume = control & 0x1F;
            state.sweep = sweep;
            state.period_high = high;
            if retrigger_on(state) {
                state.retrigger = (control & 0x1F) | 0x80;
            }
        }
        Event::Vibrato {
            length,
            rate,
            depth,
        } => {
            state.vibrato_length = length;
            state.vibrato_interval = rate.wrapping_add(1);
            state.vibrato_counter = state.vibrato_interval;
            state.vibrato_delta = depth.wrapping_neg();
            state.vibrato_left = length / 2;
        }
        Event::VibratoOff => state.vibrato_length = 0,
        Event::Slide {
            up,
            delay,
            rate,
            steps,
            amount,
            back,
        } => {
            state.slide_delay = delay;
            state.slide_interval = rate.wrapping_add(1);
            state.slide_counter = state.slide_interval;
            state.slide_steps = steps;
            state.slide_delta = if up { amount.wrapping_neg() } else { amount };
            state.slide_back = back;
        }
        Event::Volume(value) => {
            state.volume_mode = 0;
            state.volume = value | 0x10;
            if retrigger_on(state) {
                state.retrigger = value | 0x90;
            }
        }
        Event::VolumeRamp {
            delay,
            rate,
            target,
        } => volume_ramp(state, delay, rate, target),
        Event::VolumeWave { half, rate, delta } => {
            state.volume_mode = 2;
            state.volume_timer = half;
            state.volume_reload = half / 2;
            state.volume_interval = rate.wrapping_add(1);
            state.volume_counter = state.volume_interval;
            state.volume_goal = delta;
            state.volume |= 0x10;
            if retrigger_on(state) {
                state.retrigger = state.volume | 0x80;
            }
        }
        Event::VolumeEffectOff => state.volume_mode = 0,
        Event::Transpose(value) => state.transpose = value,
        Event::TransposeAdd(value) => state.transpose = state.transpose.wrapping_add(value),
        // Spec gap (sound.md 4.1, $15): only "retrigger off"; the start volume is cleared too,
        // which nothing can observe since every command that turns it on sets it.
        Event::RetriggerOff => state.retrigger = 0,
        Event::Retrigger {
            delay,
            rate,
            target,
        } => {
            state.retrigger = state.volume | 0x90;
            volume_ramp(state, delay, rate, target);
        }
        // Notes, flow, tempo and all-channel commands are handled by the caller.
        _ => {}
    }
}

fn volume_ramp(state: &mut ChannelState, delay: u8, rate: u8, target: u8) {
    state.volume_mode = 1;
    state.volume_timer = delay.wrapping_add(1);
    state.volume_reload = state.volume_timer;
    state.volume_interval = rate.wrapping_add(1);
    state.volume_counter = state.volume_interval;
    state.volume_goal = target | 0x10;
    state.volume |= 0x10;
    if state.retrigger & 0x80 != 0 {
        state.retrigger = state.volume | 0x80;
    }
}

fn slide(state: &mut ChannelState) {
    if state.slide_interval == 0 {
        return;
    }
    if state.slide_delay != 0 {
        state.slide_delay -= 1;
        if state.slide_delay != 0 {
            return;
        }
    }
    state.slide_counter = state.slide_counter.wrapping_sub(1);
    if state.slide_counter != 0 {
        return;
    }
    state.slide_counter = state.slide_interval;
    state.slide_steps = state.slide_steps.wrapping_sub(1);
    if state.slide_steps == 0 {
        state.slide_interval = 0;
    }
    let mut delta = state.slide_delta;
    if state.slide_back != 0 {
        delta = delta.wrapping_neg();
        state.slide_back -= 1;
    }
    state.add_to_period(delta);
}

fn vibrato(state: &mut ChannelState) {
    if state.vibrato_length == 0 {
        return;
    }
    state.vibrato_counter = state.vibrato_counter.wrapping_sub(1);
    if state.vibrato_counter != 0 {
        return;
    }
    state.vibrato_counter = state.vibrato_interval;
    if state.period() & 0x7FF == 0 {
        return;
    }
    state.add_to_period(state.vibrato_delta);
    state.vibrato_left = state.vibrato_left.wrapping_sub(1);
    if state.vibrato_left == 0 {
        state.vibrato_left = state.vibrato_length;
        state.vibrato_delta = state.vibrato_delta.wrapping_neg();
    }
}

fn volume_effect(state: &mut ChannelState) {
    let mode = state.volume_mode;
    if mode == 0 {
        return;
    }
    if mode == 1 && state.volume_timer != 1 {
        state.volume_timer = state.volume_timer.wrapping_sub(1);
        return;
    }
    state.volume_counter = state.volume_counter.wrapping_sub(1);
    if state.volume_counter != 0 {
        return;
    }
    state.volume_counter = state.volume_interval;
    if mode == 1 {
        state.volume = if state.volume < state.volume_goal {
            state.volume.wrapping_add(1)
        } else {
            state.volume.wrapping_sub(1)
        };
        if state.volume & 0x0F == state.volume_goal & 0x0F {
            state.volume_mode = 0;
        }
    } else {
        let next = state.volume.wrapping_add(state.volume_goal);
        if next & 0x10 != 0 {
            state.volume = next;
        }
        state.volume_reload = state.volume_reload.wrapping_sub(1);
        if state.volume_reload == 0 {
            state.volume_reload = state.volume_timer;
            state.volume_goal = state.volume_goal.wrapping_neg();
        }
    }
}

/// Sound.md 6.5, "full write".
fn full_write(channel: Channel, state: &ChannelState, out: &mut Vec<RegisterWrite>) {
    let first = channel.first_register();
    out.extend([
        RegisterWrite {
            register: first + 1,
            value: state.sweep,
        },
        RegisterWrite {
            register: first,
            value: state.first_register_value(),
        },
        RegisterWrite {
            register: first + 2,
            value: state.period_low,
        },
        RegisterWrite {
            register: first + 3,
            value: state.period_high,
        },
    ]);
}

/// Saves the RAM effect as the period of its first step; loading gives a placeholder effect
/// that [`Driver::rebind`] replaces.
mod ram_effect_saved {
    use super::{Effect, EffectStep, RAM_EFFECT_ID};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(effect: &Option<Effect>, s: S) -> Result<S::Ok, S::Error> {
        let period = effect.as_ref().and_then(|e| match e.steps.first() {
            Some(EffectStep::Tone(_, period)) => Some(*period),
            _ => None,
        });
        period.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Effect>, D::Error> {
        Ok(Option::<u16>::deserialize(d)?.map(|period| Effect {
            id: RAM_EFFECT_ID,
            in_ram: true,
            control: 0,
            frames_per_step: 0,
            sweep: 0,
            high: 0,
            steps: vec![EffectStep::Tone(0, period)],
        }))
    }
}
