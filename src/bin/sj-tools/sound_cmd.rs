//! `sj-tools render-audio` and `sj-tools sound-compare`.

use solar_jetman::game::sound::data::SONG_COUNT;
use solar_jetman::game::sound::player::Player;
use solar_jetman::game::sound::{Channel, EFFECTS};
use solar_jetman::tools::sound::{
    Capture, Comparison, Happening, Length, compare, effect_pointers, effect_requests,
    effect_starts, parse_capture, song_length, song_starts, sound_data, wav, writes_text,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

fn number(text: Option<&String>, what: &str) -> Result<u32, String> {
    let text = text.ok_or(format!("{what} needs a number"))?;
    let parsed = if let Some(hex) = text.strip_prefix("0x").or(text.strip_prefix('$')) {
        u32::from_str_radix(hex, 16)
    } else {
        text.parse()
    };
    parsed.map_err(|_| format!("{what}: `{text}` is not a number"))
}

/// `sj-tools render-audio (--song N | --effect ID) [--frames F] [--rate HZ] [--out FILE.wav]
/// [ROM]`: plays a song or an effect through the driver and the synthesiser, headlessly, into
/// a WAV file (by default under `vendor/audio/`, as it is made of the ROM's music).
pub fn render_audio(repo: &Path, args: &[String]) -> Result<(), String> {
    let (mut song, mut effect, mut frames, mut out, mut rom) = (None, None, None, None, None);
    let mut rate = 48_000;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--song" => song = Some(number(args.next(), "--song")? as usize),
            "--effect" => effect = Some(number(args.next(), "--effect")? as u8),
            "--frames" => frames = Some(number(args.next(), "--frames")?),
            "--rate" => rate = number(args.next(), "--rate")?,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a path")?)),
            _ if rom.is_none() && !arg.starts_with('-') => rom = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let (_, rom) = solar_jetman::tools::load_rom(repo, rom)?;
    let data = sound_data(&rom)?;
    let mut player = Player::new(data.clone(), rate);
    let (name, frames) = match (song, effect) {
        (Some(song), None) if song < SONG_COUNT => {
            player.sound_mut().play_song(song);
            // By default: once through (to its end, or its loop point) and a second more.
            let frames =
                frames.unwrap_or_else(|| song_length(&data, song, 1, 36_000).frames() + 60);
            (format!("song-{song:02}"), frames)
        }
        (None, Some(id)) => {
            let known = EFFECTS.iter().find(|e| e.id == id);
            match known {
                Some(effect) => {
                    for &(channel, id) in effect.starts {
                        player.sound_mut().start_effect(channel, id);
                    }
                }
                None => player.sound_mut().start_effect(Channel::Pulse1, id),
            }
            (format!("effect-{id:02X}"), frames.unwrap_or(180))
        }
        _ => return Err(format!("give --song 0-{} or --effect ID", SONG_COUNT - 1)),
    };
    let mut samples = Vec::new();
    for _ in 0..frames {
        samples.extend_from_slice(player.run_frame().1);
    }
    let target = out.unwrap_or_else(|| repo.join("vendor/audio").join(format!("{name}.wav")));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(&target, wav(&samples, rate))
        .map_err(|error| format!("{}: {error}", target.display()))?;
    println!(
        "{name}: {frames} frames, {:.1} s at {rate} Hz: {}",
        samples.len() as f64 / f64::from(rate),
        target.display()
    );
    Ok(())
}

/// One run of the original.
struct Plan {
    name: String,
    /// What the capture covers.
    note: String,
    frames: u32,
    /// "frame tempo t0 t1 t2 t3" for the capture script.
    song: String,
    requests: String,
    effects: String,
    /// Copy the RAM effect's template at the song's frame.
    ram_effect: bool,
}

/// The frame of the title screen at which songs are started in captures: after the game has
/// started the title song itself (frame 519), with nothing else starting songs afterwards.
const INJECT_FRAME: u32 = 600;
/// The test runner stops after about 100 seconds.
const MAX_FRAMES: u32 = 40_000;

/// `sj-tools sound-compare [--songs all|N|N-M,...] [--no-effects] [--recapture] [--jobs N]
/// [ROM]`: captures the original's driver in Mesen (headless, `tools/mesen/sound-capture.lua`)
/// for every song and for runs of every effect, keeps the captures under `vendor/sound/`, and
/// replays each through our driver, comparing the register writes update by update.
pub fn sound_compare(repo: &Path, args: &[String]) -> Result<(), String> {
    let mut songs: Vec<usize> = (0..SONG_COUNT).collect();
    let (mut effects, mut recapture, mut jobs, mut rom_arg) = (true, false, 6usize, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--songs" => songs = song_list(args.next().ok_or("--songs needs a list")?)?,
            "--no-effects" => effects = false,
            "--recapture" => recapture = true,
            "--jobs" => jobs = number(args.next(), "--jobs")?.max(1) as usize,
            _ if rom_arg.is_none() && !arg.starts_with('-') => rom_arg = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let (rom_path, rom) = solar_jetman::tools::load_rom(repo, rom_arg)?;
    let data = sound_data(&rom)?;
    let starts = song_starts(&rom)?;
    let song_spec = |song: usize| {
        let (tempo, tracks) = starts[song];
        format!(
            "{INJECT_FRAME} {tempo} {:04X} {:04X} {:04X} {:04X}",
            tracks[0], tracks[1], tracks[2], tracks[3]
        )
    };

    let mut plans = Vec::new();
    for &song in &songs {
        // Past the end, or through the loop twice when that fits in a run.
        let length = song_length(&data, song, 2, MAX_FRAMES);
        let once = song_length(&data, song, 1, MAX_FRAMES);
        let frames = (INJECT_FRAME + length.frames() + 300).clamp(INJECT_FRAME + 1200, MAX_FRAMES);
        plans.push(Plan {
            name: format!("song-{song:02}"),
            note: match once {
                Length::Ends(at) => format!("ends after {at} frames"),
                Length::Loops(at) if frames - INJECT_FRAME > at => {
                    format!("loops after {at} frames, covered")
                }
                other => format!("{other:?}: loop NOT covered"),
            },
            frames,
            song: song_spec(song),
            requests: String::new(),
            effects: String::new(),
            ram_effect: false,
        });
    }
    if effects {
        let (first, spacing) = (INJECT_FRAME + 100, 170);
        let length = first + spacing * EFFECTS.len() as u32 + 300;
        // Through the request slots, as the game asks (until the title's demo starts and drops
        // requests, sound.md 8.2), then started directly over three songs.
        plans.push(Plan {
            name: "effects-requested".to_owned(),
            note: "requests through the slots".to_owned(),
            frames: length,
            song: song_spec(0),
            requests: effect_requests(first, spacing),
            effects: String::new(),
            ram_effect: false,
        });
        let pointers = effect_pointers(&rom)?;
        for song in [0, 4, 14] {
            plans.push(Plan {
                name: format!("effects-over-song-{song:02}"),
                note: "direct starts".to_owned(),
                frames: length,
                song: song_spec(song),
                requests: String::new(),
                effects: effect_starts(first, spacing, &pointers),
                ram_effect: true,
            });
        }
    }

    let folder = repo.join("vendor/sound/captures");
    fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let to_capture: Vec<&Plan> = plans
        .iter()
        .filter(|plan| recapture || !folder.join(format!("{}.log", plan.name)).exists())
        .collect();
    if !to_capture.is_empty() {
        println!(
            "Capturing {} runs of the original in Mesen ({jobs} at a time)...",
            to_capture.len()
        );
        capture_all(repo, &rom_path, &folder, &to_capture, jobs)?;
    }

    let mut report = String::new();
    let mut exact = Vec::new();
    for plan in &plans {
        let path = folder.join(format!("{}.log", plan.name));
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let capture =
            parse_capture(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let result = compare(&data, &starts, &capture);
        let line = summary(plan, &capture, &result);
        println!("{line}");
        report.push_str(&line);
        report.push('\n');
        report.push_str(&details(&capture, &result));
        if result.matching == result.updates && capture.end.is_some() {
            exact.push(plan.name.clone());
        }
    }
    let report_path = repo.join("vendor/sound/report.txt");
    fs::write(&report_path, &report)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;
    println!(
        "\n{} of {} runs match exactly. Details: {}",
        exact.len(),
        plans.len(),
        report_path.display()
    );
    Ok(())
}

fn song_list(text: &str) -> Result<Vec<usize>, String> {
    if text == "all" {
        return Ok((0..SONG_COUNT).collect());
    }
    let mut songs = Vec::new();
    for part in text.split(',') {
        let parse = |n: &str| {
            n.parse::<usize>()
                .ok()
                .filter(|&n| n < SONG_COUNT)
                .ok_or(format!("`{n}` is not a song number 0-{}", SONG_COUNT - 1))
        };
        match part.split_once('-') {
            Some((from, to)) => songs.extend(parse(from)?..=parse(to)?),
            None => songs.push(parse(part)?),
        }
    }
    Ok(songs)
}

fn capture_all(
    repo: &Path,
    rom: &Path,
    folder: &Path,
    plans: &[&Plan],
    jobs: usize,
) -> Result<(), String> {
    let script = repo.join("tools/mesen/sound-capture.lua");
    let runner = repo.join("tools/mesen/run.sh");
    let queue = Mutex::new(plans.iter());
    let failures = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let Some(plan) = queue.lock().expect("queue").next() else {
                        break;
                    };
                    let log = folder.join(format!("{}.log", plan.name));
                    let status = Command::new(&runner)
                        .arg(&script)
                        .arg(rom)
                        .env("SJ_SOUND_OUT", &log)
                        .env("SJ_SOUND_FRAMES", plan.frames.to_string())
                        .env("SJ_SOUND_SONG", &plan.song)
                        .env("SJ_SOUND_REQUESTS", &plan.requests)
                        .env("SJ_SOUND_EFFECTS", &plan.effects)
                        .env(
                            "SJ_SOUND_RAM_EFFECT",
                            if plan.ram_effect {
                                INJECT_FRAME.to_string()
                            } else {
                                String::new()
                            },
                        )
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    match status {
                        Ok(status) if status.success() => println!("  captured {}", plan.name),
                        other => failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}: {other:?}", plan.name)),
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().expect("failures");
    if failures.is_empty() {
        Ok(())
    } else {
        // Keep going with what was captured; runs cut short are reported as such.
        eprintln!(
            "some captures did not finish cleanly:\n  {}",
            failures.join("\n  ")
        );
        Ok(())
    }
}

fn summary(plan: &Plan, capture: &Capture, result: &Comparison) -> String {
    let verdict = if capture.end.is_none() {
        "INCOMPLETE CAPTURE"
    } else if result.matching == result.updates {
        "exact"
    } else {
        "DIFFERS"
    };
    format!(
        "{:<20} {verdict:<18} {}/{} updates match, {} frames, {} effect starts, {} unexplained RAM changes; {}",
        plan.name,
        result.matching,
        result.updates,
        capture.end.unwrap_or(0),
        result
            .happenings
            .iter()
            .filter(|(_, h)| matches!(h, Happening::EffectStart(..)))
            .count(),
        result.unexplained.len(),
        plan.note,
    )
}

fn details(capture: &Capture, result: &Comparison) -> String {
    let mut text = String::new();
    for (frame, happening) in &result.happenings {
        let what = match happening {
            Happening::SongStart(song) => format!("song {song} starts"),
            Happening::EffectStart(channel, id) => format!("effect ${id:02X} on {channel:?}"),
            Happening::RamEffect => "the RAM effect is (re)written".to_owned(),
        };
        text.push_str(&format!("    frame {frame}: {what}\n"));
    }
    for (frame, addresses) in result.unexplained.iter().take(10) {
        let list: Vec<String> = addresses.iter().map(|a| format!("{a:04X}")).collect();
        text.push_str(&format!(
            "    frame {frame}: unexplained {}\n",
            list.join(" ")
        ));
    }
    for (frame, theirs, ours) in &result.mismatches {
        text.push_str(&format!(
            "    frame {frame}: original {}\n    {:>w$}  ours     {}\n",
            writes_text(theirs),
            "",
            writes_text(ours),
            w = frame.to_string().len() + 6,
        ));
    }
    let outside: Vec<String> = capture
        .other_writes
        .iter()
        .filter(|(frame, ..)| *frame > 2)
        .take(10)
        .map(|(frame, register, value)| format!("{frame}:{register:04X}={value:02X}"))
        .collect();
    if !outside.is_empty() {
        text.push_str(&format!(
            "    writes outside the driver: {}\n",
            outside.join(" ")
        ));
    }
    text
}
