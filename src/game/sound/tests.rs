//! Driver tests on made-up songs and effects.

use super::data::*;
use super::jukebox::Jukebox;
use super::player::Player;
use super::*;

/// A made-up period table: entry n is 0x700 − 24 n, except entry 24 = $01C4 as in the spec's
/// worked example.
pub(crate) fn periods() -> Vec<u16> {
    (0..NOTE_PERIODS as u16)
        .map(|n| match n {
            0 => 0,
            24 => 0x1C4,
            _ => 0x700 - 24 * n,
        })
        .collect()
}

/// Song 0 ends at once; song 1 has the given tracks (missing ones end at once); the others are
/// copies of song 0.
pub(crate) fn data_with(tempo: u8, tracks: Vec<Vec<Event>>, effects: Vec<Effect>) -> SoundData {
    let silent = Song {
        tempo: 0x80,
        tracks: vec![
            Track {
                events: vec![Event::End]
            };
            4
        ],
    };
    let mut song = silent.clone();
    song.tempo = tempo;
    for (channel, events) in tracks.into_iter().enumerate() {
        if !events.is_empty() {
            song.tracks[channel] = Track { events };
        }
    }
    let mut songs = vec![silent; SONG_COUNT];
    songs[1] = song;
    SoundData::new(songs, effects, periods()).unwrap()
}

fn w(register: u16, value: u8) -> RegisterWrite {
    RegisterWrite { register, value }
}

fn update(driver: &mut Driver) -> Vec<RegisterWrite> {
    let mut out = Vec::new();
    driver.update(&mut out);
    out
}

/// sound.md 9, second worked example (a made-up track).
#[test]
fn spec_worked_example() {
    let track = vec![
        Event::Setup {
            control: 0xB7,
            sweep: 0x43,
            high: 0x9A,
        },
        Event::Volume(0x08),
        Event::Note(0x18, 4),
        Event::Rest(2),
        Event::End,
    ];
    let mut driver = Driver::new(data_with(0x80, vec![track], vec![]));
    driver.start_song(1);
    let mut log = Vec::new();
    for _ in 0..15 {
        let writes = update(&mut driver);
        // Only pulse 1 matters; the other channels end at the first tick.
        log.push(
            writes
                .into_iter()
                .filter(|w| w.register < 0x4004)
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(log[0], [w(0x4000, 0x00), w(0x4002, 0x00)]);
    assert_eq!(
        log[1],
        [
            w(0x4001, 0x43),
            w(0x4000, 0xB8),
            w(0x4002, 0xC4),
            w(0x4003, 0x99)
        ]
    );
    for update in [2, 4, 6, 8] {
        assert_eq!(log[update], [w(0x4000, 0xB8), w(0x4002, 0xC4)], "{update}");
    }
    for update in [3, 5, 7, 11] {
        assert!(log[update].is_empty(), "{update}");
    }
    assert_eq!(
        log[9],
        [
            w(0x4001, 0x43),
            w(0x4000, 0xB8),
            w(0x4002, 0x00),
            w(0x4003, 0x98)
        ]
    );
    assert_eq!(log[10], [w(0x4000, 0xB8), w(0x4002, 0x00)]);
    assert_eq!(log[12], [w(0x4000, 0xB8), w(0x4002, 0x00)]);
    assert_eq!(log[13], [w(0x4000, 0x10)]);
    // Music has stopped: nothing more.
    assert!(log[14].is_empty());
}

#[test]
fn first_tick_comes_after_ceil_256_over_tempo_updates() {
    for tempo in [0x40u8, 0x55, 0x80, 0xFF] {
        let mut driver = Driver::new(data_with(tempo, vec![vec![Event::End]], vec![]));
        driver.start_song(1);
        let mut updates = 0;
        while driver.channel(Channel::Pulse1).music_active {
            update(&mut driver);
            updates += 1;
        }
        assert_eq!(updates, 256usize.div_ceil(usize::from(tempo)), "{tempo:#x}");
    }
}

/// A made-up effect on the noise channel: 2 steps of 3 frames each.
fn noise_effect() -> Effect {
    Effect {
        id: 0x0A,
        in_ram: false,
        control: 0xB0,
        frames_per_step: 2,
        sweep: 0x00,
        high: 0x08,
        steps: vec![EffectStep::Tone(5, 0x340), EffectStep::Tone(4, 0x341)],
    }
}

#[test]
fn effect_owns_the_channel_then_hands_it_back() {
    let noise_track = vec![
        Event::Setup {
            control: 0x30,
            sweep: 0x00,
            high: 0x00,
        },
        Event::Note(0x41 + 3, 0),
        Event::End,
    ];
    let data = data_with(
        0xFF,
        vec![vec![], vec![], vec![], noise_track],
        vec![noise_effect()],
    );
    let mut driver = Driver::new(data);
    driver.start_song(1);
    update(&mut driver);
    update(&mut driver); // first tick: the noise note starts (index 68 → value 3)
    driver.start_effect(Channel::Noise, 0x0A);
    let noise = |writes: Vec<RegisterWrite>| -> Vec<RegisterWrite> {
        writes
            .into_iter()
            .filter(|w| w.register >= 0x400C)
            .collect()
    };
    // Step 1 at the next update: R0, R1, R3 (period bits 8-10 changed), R2.
    assert_eq!(
        noise(update(&mut driver)),
        [
            w(0x400C, 0xB5),
            w(0x400D, 0x00),
            w(0x400F, 0x0B),
            w(0x400E, 0x40)
        ]
    );
    assert!(noise(update(&mut driver)).is_empty());
    // The same id again is ignored: the effect is not restarted.
    driver.start_effect(Channel::Noise, 0x0A);
    assert!(noise(update(&mut driver)).is_empty());
    // Step 2: period bits 8-10 unchanged, so no R3.
    assert_eq!(
        noise(update(&mut driver)),
        [w(0x400C, 0xB4), w(0x400D, 0x00), w(0x400E, 0x41)]
    );
    update(&mut driver);
    update(&mut driver);
    // End: the music's full state is written at once.
    assert_eq!(
        noise(update(&mut driver)),
        [
            w(0x400D, 0x00),
            w(0x400C, 0x30),
            w(0x400E, 0x03),
            w(0x400F, 0x00)
        ]
    );
    assert_eq!(driver.channel(Channel::Noise).effect_id, 0);
}

#[test]
fn effect_without_music_ends_with_silence_and_end_of_track_always_writes() {
    let mut driver = Driver::new(data_with(0x80, vec![], vec![noise_effect()]));
    driver.start_effect(Channel::Noise, 0x0A);
    let writes: Vec<_> = (0..7).flat_map(|_| update(&mut driver)).collect();
    assert_eq!(writes.last(), Some(&w(0x400C, 0x10)));
    // End of track writes $10 even under an effect.
    driver.start_song(1);
    driver.start_effect(Channel::Noise, 0x0A);
    update(&mut driver);
    let writes = update(&mut driver);
    assert!(writes.contains(&w(0x400C, 0x10)), "{writes:?}");
    assert!(driver.channel(Channel::Noise).effect_id != 0);
}

/// Runs to the first tick (tempo $FF: update 2), then stops ticks for 256 updates.
fn start_then_hold(driver: &mut Driver) {
    driver.start_song(1);
    update(driver);
    update(driver);
    driver.tempo = 1;
    driver.accumulator = 0;
}

/// Pulse 1's period after each update.
fn periods_over(driver: &mut Driver, updates: usize) -> Vec<u16> {
    (0..updates)
        .map(|_| {
            update(driver);
            let state = driver.channel(Channel::Pulse1);
            u16::from_le_bytes([state.period_low, state.period_high])
        })
        .collect()
}

#[test]
fn vibrato_follows_the_spec_example() {
    // Tempo 1: one tick in 256 frames, so the effects run every other frame.
    let track = vec![
        Event::Vibrato {
            length: 2,
            rate: 1,
            depth: 1,
        },
        Event::Note(0x10, 1),
        Event::End,
    ];
    let mut driver = Driver::new(data_with(0xFF, vec![track], vec![]));
    start_then_hold(&mut driver);
    let base = 0x700 - 24 * 16;
    let seen = periods_over(&mut driver, 14);
    let deltas: Vec<i32> = seen.iter().map(|&p| i32::from(p) - base).collect();
    // Every 2nd effect frame: −1 once, then +1, +1, −1, −1, +1, +1.
    assert_eq!(deltas, [0, -1, -1, 0, 0, 1, 1, 0, 0, -1, -1, 0, 0, 1]);
}

#[test]
fn slide_goes_back_first_and_stops_after_its_steps() {
    let track = vec![
        Event::Slide {
            up: true,
            delay: 2,
            rate: 0,
            steps: 4,
            amount: 3,
            back: 1,
        },
        Event::Note(0x10, 1),
        Event::End,
    ];
    let mut driver = Driver::new(data_with(0xFF, vec![track], vec![]));
    start_then_hold(&mut driver);
    let base = 0x700 - 24 * 16;
    let seen: Vec<i32> = periods_over(&mut driver, 7)
        .into_iter()
        .map(|p| i32::from(p) - base)
        .collect();
    // Delay 2: the first frame waits; then one step per frame: +3 (back), −3, −3, −3.
    assert_eq!(seen, [0, 3, 0, -3, -6, -6, -6]);
    assert_eq!(driver.channel(Channel::Pulse1).slide_interval, 0);
}

#[test]
fn volume_ramp_and_retrigger() {
    let track = vec![
        Event::Setup {
            control: 0x30,
            sweep: 0,
            high: 0,
        },
        Event::Volume(0x03),
        Event::Retrigger {
            delay: 1,
            rate: 0,
            target: 0x06,
        },
        Event::Note(0x10, 8),
        Event::Note(0x10, 8),
        Event::End,
    ];
    let mut driver = Driver::new(data_with(0x80, vec![track], vec![]));
    driver.start_song(1);
    let mut volumes = Vec::new();
    for _ in 0..34 {
        update(&mut driver);
        volumes.push(driver.channel(Channel::Pulse1).volume & 0x0F);
    }
    // Note at update 2; the ramp waits 1 effect frame, then +1 per effect frame up to 6; the
    // second note (update 18) restarts the ramp from 3.
    assert_eq!(
        &volumes[..18],
        [0, 3, 3, 3, 4, 4, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 3]
    );
    assert_eq!(volumes[22], 5);
}

#[test]
fn calls_repeat_and_return() {
    // Main: call the block at 3 twice, then a rest and end. Block: one note, return.
    let track = vec![
        Event::Call { times: 2, to: 3 },
        Event::Rest(1),
        Event::End,
        Event::Note(0x05, 1),
        Event::Return,
    ];
    let mut driver = Driver::new(data_with(0xFF, vec![track], vec![]));
    driver.start_song(1);
    let mut positions = Vec::new();
    for _ in 0..5 {
        update(&mut driver);
        positions.push(driver.channel(Channel::Pulse1).position);
    }
    assert_eq!(positions, [0, 4, 4, 2, 3]);
    assert!(!driver.channel(Channel::Pulse1).music_active);
}

#[test]
fn transposition_and_noise_values() {
    let pulse = vec![
        Event::TransposeAll(0xFD),
        Event::Note(0x03, 1), // transposes to index 0: a rest
        Event::End,
    ];
    let noise = vec![Event::Note(0x01, 1), Event::Note(0x44, 1), Event::End];
    let mut driver = Driver::new(data_with(0xFF, vec![pulse, vec![], vec![], noise], vec![]));
    driver.start_song(1);
    update(&mut driver);
    let writes = update(&mut driver);
    assert!(writes.contains(&w(0x4002, 0x00)), "{writes:?}");
    // Noise transposed by −3 too: index 1 − 3 = $7E → ($7E − 65) AND $7F = $3D.
    assert!(writes.contains(&w(0x400E, 0x3D)), "{writes:?}");
    let writes = update(&mut driver);
    // Index $44 − 3 = 65 → 0.
    assert!(writes.contains(&w(0x400E, 0x00)), "{writes:?}");
}

#[test]
fn song_start_resets_only_what_the_spec_says() {
    let track = vec![
        Event::Setup {
            control: 0x9F,
            sweep: 0x11,
            high: 0x08,
        },
        Event::Transpose(5),
        Event::Note(0x10, 0),
        Event::End,
    ];
    let mut driver = Driver::new(data_with(0xFF, vec![track], vec![]));
    driver.start_song(1);
    update(&mut driver);
    update(&mut driver);
    driver.start_song(0);
    let state = driver.channel(Channel::Pulse1);
    assert_eq!(
        (state.transpose, state.ticks_left, state.position),
        (0, 1, 0)
    );
    // Carried over.
    assert_eq!(
        (state.control, state.sweep, state.volume),
        (0x9F, 0x11, 0x1F)
    );
    // Before the first tick, the next update writes from the carried-over state.
    let writes = update(&mut driver);
    assert_eq!(writes[0], w(0x4000, 0x9F));
}

#[test]
fn start_if_new_remembers_the_last_song() {
    let mut driver = Driver::new(data_with(0x80, vec![], vec![]));
    driver.start_song_if_new(0);
    assert_eq!(driver.song(), (0, false));
    driver.start_song_if_new(1);
    assert_eq!(driver.song(), (1, true));
    update(&mut driver);
    update(&mut driver);
    assert_eq!(driver.song(), (1, false));
    driver.start_song_if_new(1);
    assert_eq!(driver.song(), (1, false));
}

#[test]
fn ram_effect_pitch_is_patched() {
    let mut hum = noise_effect();
    hum.id = RAM_EFFECT_ID;
    hum.in_ram = true;
    hum.steps.truncate(1);
    let mut driver = Driver::new(data_with(0x80, vec![], vec![hum]));
    driver.set_ram_effect_pitch(0x22);
    driver.start_effect(Channel::Triangle, RAM_EFFECT_ID);
    let writes = update(&mut driver);
    assert_eq!(writes.last(), Some(&w(0x400A, 0x22)));
}

#[test]
fn request_slots_alternate_and_the_lock_drops_requests() {
    let mut effects = vec![noise_effect()];
    let mut other = noise_effect();
    other.id = 0x0C;
    effects.push(other);
    let mut sound = Sound::new(data_with(0x80, vec![], effects));
    let mut out = Vec::new();
    sound.request_effect(Slot::B, Channel::Noise, 0x0A);
    sound.frame(&mut out); // frame 0 services slot A (empty)
    assert_eq!(sound.driver().channel(Channel::Noise).effect_id, 0);
    sound.frame(&mut out); // frame 1: slot B
    assert_eq!(sound.driver().channel(Channel::Noise).effect_id, 0x0A);

    // Lock $81: the next servicing proceeds, then 127 servicings drop everything.
    sound.lock_requests(0x81);
    sound.request_effect(Slot::A, Channel::Pulse1, 0x0C);
    sound.frame(&mut out); // frame 2, slot A
    assert_eq!(sound.driver().channel(Channel::Pulse1).effect_id, 0x0C);
    for _ in 0..128 {
        sound.request_effect(Slot::A, Channel::Pulse2, 0x0C);
        sound.request_effect(Slot::B, Channel::Pulse2, 0x0C);
        sound.frame(&mut out);
    }
    assert_eq!(sound.driver().channel(Channel::Pulse2).effect_id, 0);
    sound.request_effect(Slot::B, Channel::Pulse2, 0x0C);
    sound.frame(&mut out); // frame 131: slot B, no lock any more
    assert_eq!(sound.driver().channel(Channel::Pulse2).effect_id, 0x0C);

    // The demo drops slot requests; pause stops the driver but not servicing.
    sound.set_demo(true);
    sound.request_effect(Slot::A, Channel::Triangle, 0x0A);
    sound.frame(&mut out);
    sound.frame(&mut out);
    assert_eq!(sound.driver().channel(Channel::Triangle).effect_id, 0);
    sound.set_demo(false);
    out.clear();
    sound.pause(&mut out);
    assert_eq!(out, [w(0x4015, 0)]);
    let before = sound.driver().tempo();
    sound.play_song(1);
    sound.frame(&mut out);
    assert_eq!(sound.driver().tempo().1, 0, "{before:?}");
}

#[test]
fn power_on_writes() {
    let mut sound = Sound::new(data_with(0x80, vec![], vec![]));
    let mut out = Vec::new();
    sound.power_on(&mut out);
    assert_eq!(out.len(), 24 + 3);
    assert_eq!(out[0], w(0x4017, 0x10));
    assert_eq!(out[23], w(0x4000, 0x10));
    assert_eq!(
        out[24..],
        [w(0x4010, 0xC0), w(0x4017, 0x80), w(0x4015, 0x0F)]
    );
}

#[test]
fn player_makes_sound_from_a_song() {
    let track = vec![
        Event::Setup {
            control: 0xBF,
            sweep: 0x08,
            high: 0x08,
        },
        Event::Note(0x20, 0x40),
        Event::Jump(1),
    ];
    let mut player = Player::new(data_with(0x80, vec![track], vec![]), 44_100);
    player.sound_mut().play_song(1);
    let mut samples = vec![0.0; 44_100];
    player.fill(&mut samples);
    assert!(player.frames() >= 60);
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    assert!(rms > 0.02, "{rms}");
    player.set_muted(true);
    player.fill(&mut samples);
    assert!(samples[1000..].iter().all(|&s| s == 0.0));
}

#[test]
fn jukebox_wraps_and_describes() {
    let mut sound = Sound::new(data_with(0x80, vec![], vec![noise_effect()]));
    let mut jukebox = Jukebox::default();
    jukebox.change_song(&mut sound, -1);
    assert_eq!(jukebox.song(), 33);
    jukebox.change_song(&mut sound, 2);
    assert_eq!(jukebox.song(), 1);
    assert_eq!(sound.driver().song().0, 1);
    jukebox.change_effect(&mut sound, 1);
    assert_eq!(jukebox.effect().unwrap().id, 0x02);
    jukebox.change_effect(&mut sound, -1);
    assert_eq!(jukebox.effect().unwrap().id, 0x44);
    assert!(jukebox.toggle_mute());
    assert_eq!(
        jukebox.describe(),
        "song 1 (title), effect $44 (unidentified), muted"
    );
    // Effects table: sorted, even ids, one or two channels.
    for pair in EFFECTS.windows(2) {
        assert!(pair[0].id < pair[1].id);
    }
    assert!(
        EFFECTS
            .iter()
            .all(|e| e.id % 2 == 0 && (1..=2).contains(&e.starts.len()))
    );
}

#[test]
fn a_looped_song_starts_again_until_another_song_starts() {
    let tracks = vec![vec![Event::Note(0x18, 2), Event::End]];
    let mut sound = Sound::new(data_with(0xFF, tracks, vec![]));
    let mut out = Vec::new();
    let playing = |sound: &Sound| sound.driver().song() == (1, true);
    sound.play_song_if_new(1);
    for _ in 0..10 {
        sound.frame(&mut out);
    }
    assert!(!playing(&sound), "the song ends by itself");
    // Asked for again as the last song started, it stays silent; looped, it starts again.
    sound.play_song_if_new(1);
    assert!(!playing(&sound));
    sound.play_song_looped(1);
    assert!(playing(&sound));
    // It starts again at the update after the one where it ended.
    let mut restarts = 0;
    for _ in 0..100 {
        sound.frame(&mut out);
        if !playing(&sound) {
            restarts += 1;
            sound.frame(&mut out);
            assert!(playing(&sound));
        }
    }
    assert!(restarts > 1);
    // Asking again while it plays does not restart it.
    let tempo = sound.driver().tempo();
    sound.play_song_looped(1);
    assert_eq!(sound.driver().tempo(), tempo);
    // Another song ends the loop.
    sound.play_song(0);
    for _ in 0..10 {
        sound.frame(&mut out);
    }
    assert_eq!(sound.driver().song(), (0, false));
}
