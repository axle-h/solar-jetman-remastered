//! `sj-tools scene`: plays the scenes headless and saves the screen at chosen frames, for
//! comparing the title and the password screen with the original (`tools/mesen/title-capture.lua`).

use solar_jetman::game::SCREEN_HEIGHT;
use solar_jetman::game::SCREEN_WIDTH;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::rng::Rng;
use solar_jetman::game::playfield::Frame;
use solar_jetman::game::scene::{Game, SoundRequest, Start};
use std::fs;
use std::path::PathBuf;

const USAGE: &str = "sj-tools scene [--cold|--warm|--story|--flight LEVEL [--rng OUT:HIGH:LOW] [--job N]|--launch LEVEL:COUNTER|--approach LEVEL|--surface LEVEL[:dock][:owned][:flag][:clear]|--shop LEVEL:SCORE:COUNTER:MAPPING|--equip LEVEL:OWNEDMASK(hex, bit = item):COUNTER|--map LEVEL:MAPPING:SUPER[:SCORE:LIVES:EQUIPMENT:WEAPON:DEVICE]|--interlude LEVEL:POD_Y:COUNTER:JOB|--badge LEVEL:GEMS:COUNTER|--lastpassword LEVEL|--message LEVEL:KIND:TEXT:ITEM[:CARGO[:COUNTER]] --offset N] [--compare DIR] [--original|--remake] --frames F,F,... [--input FRAME:BUTTONS,...] [--out DIR] [ROM]
  Frames count as in the capture script: the original's frame numbers from power-on (--cold,
  the default) or from the title's first frame, which the capture script shows as frame 533
  (--warm). --launch starts in the launch scene of a level with the frame counter at COUNTER;
  --gap FRAME:N (repeatable) adds the original's N loading frames after its frame FRAME, which
  the remake does not have (game-flow.md 1.1). --offset N says the scene's initial picture is the original's frame N + 1. BUTTONS joined by +: start select a b up down left right, held for the 2 frames
  FRAME and FRAME + 1 like the script does. --compare DIR compares each frame with the original's
  screenshot DIR/shot-FRAME.png (tools/mesen/title-capture.lua) and prints how many pixels differ. Writes scene-FRAME.png and scene-FRAME.idx (one NES colour
  number per pixel) in DIR (default vendor/compare/scene).";

/// Pass number `p` runs in the original's frame `p + offset`; the state after it is shown one
/// frame later.
const COLD_OFFSET: i64 = 20;
const WARM_OFFSET: i64 = 532;
/// The story's first picture is the original's frame 2451 of a power-on run (scenes.md 3).
const STORY_OFFSET: i64 = 2450;
/// The flight's first picture is the original's frame 1751 in the run of the password
/// NLDKHKGDMVLB typed in at the title (`tools/mesen/title-capture.lua`).
const FLIGHT_OFFSET: i64 = 1750;

pub fn run(repo: &std::path::Path, args: &[String]) -> Result<(), String> {
    let (mut frames, mut inputs) = (Vec::new(), Vec::new());
    let mut out = repo.join("vendor/compare/scene");
    let mut rom = None;
    let mut start = Start::Cold;
    let mut offset = None;
    let mut rng: Option<Rng> = None;
    let mut job: Option<u8> = None;
    let mut gaps: Vec<(i64, i64)> = Vec::new();
    let mut compare: Option<PathBuf> = None;
    // The original's rules (docs/deviations.md "Validation mode"): the default with --compare.
    let mut original: Option<bool> = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cold" => start = Start::Cold,
            "--warm" => start = Start::Warm,
            "--rng" => {
                let value = args.next().ok_or("--rng needs OUT:HIGH:LOW in hex")?;
                let bytes: Vec<u8> = value
                    .split(':')
                    .map(|b| u8::from_str_radix(b, 16).map_err(|_| format!("bad --rng `{value}`")))
                    .collect::<Result<_, _>>()?;
                let [out, high, low] = bytes[..] else {
                    return Err("--rng is OUT:HIGH:LOW (RAM $5A, $5B, $5C)".to_owned());
                };
                rng = Some(Rng { out, high, low });
            }
            "--job" => {
                let value = args.next().ok_or("--job needs a number 0-7 (RAM $60)")?;
                job = Some(value.parse().map_err(|_| format!("bad --job `{value}`"))?);
            }
            "--story" => start = Start::Story,
            "--flight" => {
                let level = args.next().ok_or("--flight needs a level")?;
                start = Start::Flight(level.parse().map_err(|_| format!("bad level `{level}`"))?);
            }
            "--launch" => {
                let value = args.next().ok_or("--launch needs LEVEL:COUNTER")?;
                let (level, counter) = value.split_once(':').ok_or("--launch is LEVEL:COUNTER")?;
                start = Start::Launch {
                    level: level.parse().map_err(|_| format!("bad level `{level}`"))?,
                    counter: counter
                        .parse()
                        .map_err(|_| format!("bad counter `{counter}`"))?,
                };
            }
            "--approach" => {
                let level = args.next().ok_or("--approach needs a level")?;
                start = Start::Approach(level.parse().map_err(|_| format!("bad level `{level}`"))?);
            }
            "--surface" => {
                // LEVEL, then optional :dock :owned :flag
                let value = args
                    .next()
                    .ok_or("--surface needs LEVEL[:dock][:owned][:flag]")?;
                let mut parts = value.split(':');
                let level = parts.next().unwrap_or("0");
                let level = level.parse().map_err(|_| format!("bad level `{level}`"))?;
                let rest: Vec<&str> = parts.collect();
                start = Start::Surface {
                    level,
                    dock: rest.contains(&"dock"),
                    owned: rest.contains(&"owned"),
                    flag: rest.contains(&"flag"),
                    clear: rest.contains(&"clear"),
                };
            }
            "--shop" => {
                let value = args
                    .next()
                    .ok_or("--shop needs LEVEL:SCORE:COUNTER:MAPPING")?;
                let n: Vec<u32> = value
                    .split(':')
                    .map(|v| v.parse().map_err(|_| format!("bad --shop `{value}`")))
                    .collect::<Result<_, _>>()?;
                let [level, score, counter, mapping] = n[..] else {
                    return Err("--shop is LEVEL:SCORE:COUNTER:MAPPING(0 or 1)".to_owned());
                };
                start = Start::Shop {
                    level: level as usize,
                    score,
                    counter,
                    mapping: mapping != 0,
                };
            }
            "--equip" => {
                let value = args.next().ok_or("--equip needs LEVEL:OWNED:COUNTER")?;
                let parts: Vec<&str> = value.split(':').collect();
                let [level, owned, counter] = parts[..] else {
                    return Err("--equip is LEVEL:OWNED(hex):COUNTER".to_owned());
                };
                start = Start::Equip {
                    level: level.parse().map_err(|_| format!("bad level `{level}`"))?,
                    owned: u16::from_str_radix(owned.trim_start_matches("0x"), 16)
                        .map_err(|_| format!("bad owned mask `{owned}`"))?,
                    counter: counter
                        .parse()
                        .map_err(|_| format!("bad counter `{counter}`"))?,
                };
            }
            "--map" => {
                let value = args.next().ok_or(
                    "--map needs LEVEL:MAPPING:SUPER[:SCORE:LIVES:EQUIPMENT:WEAPON:DEVICE]",
                )?;
                let n: Vec<u32> = value
                    .split(':')
                    .map(|v| v.parse().map_err(|_| format!("bad --map `{value}`")))
                    .collect::<Result<_, _>>()?;
                if n.len() < 3 {
                    return Err("--map is LEVEL:MAPPING(0 or 1):SUPER(0 or 1)[:SCORE:LIVES:EQUIPMENT:WEAPON:DEVICE]".to_owned());
                }
                let at = |i: usize, default: u32| n.get(i).copied().unwrap_or(default);
                start = Start::Map {
                    level: n[0] as usize,
                    mapping: n[1] != 0,
                    super_mapping: n[2] != 0,
                    score: at(3, 1000),
                    lives: at(4, 4) as u8,
                    equipment: at(5, 0xC0) as u8,
                    weapon: at(6, 0) as u8,
                    device: at(7, 0) as u8,
                };
            }
            "--interlude" => {
                let value = args
                    .next()
                    .ok_or("--interlude needs LEVEL:POD_Y:COUNTER:JOB")?;
                let n: Vec<u32> = value
                    .split(':')
                    .map(|v| v.parse().map_err(|_| format!("bad --interlude `{value}`")))
                    .collect::<Result<_, _>>()?;
                let [level, pod_y, counter, job] = n[..] else {
                    return Err("--interlude is LEVEL:POD_Y:COUNTER:JOB".to_owned());
                };
                start = Start::Interlude {
                    level: level as usize,
                    pod_y: pod_y as u16,
                    counter,
                    job: job as u8,
                };
            }
            "--lastpassword" => {
                let level = args.next().ok_or("--lastpassword needs a level")?;
                start = Start::LastPassword(level.parse().map_err(|_| "bad --lastpassword level")?);
            }
            "--badge" => {
                let value = args.next().ok_or("--badge needs LEVEL:GEMS:COUNTER")?;
                let n: Vec<u32> = value
                    .split(':')
                    .map(|v| v.parse().map_err(|_| format!("bad --badge `{value}`")))
                    .collect::<Result<_, _>>()?;
                let [level, gems, counter] = n[..] else {
                    return Err("--badge is LEVEL:GEMS:COUNTER".to_owned());
                };
                start = Start::Badge {
                    level: level as usize,
                    gems: gems as u8,
                    counter,
                };
            }
            "--message" => {
                let value = args
                    .next()
                    .ok_or("--message needs LEVEL:KIND:TEXT:ITEM[:CARGO]")?;
                let n: Vec<usize> = value
                    .split(':')
                    .map(|v| v.parse().map_err(|_| format!("bad --message `{value}`")))
                    .collect::<Result<_, _>>()?;
                if n.len() < 4 {
                    return Err("--message is LEVEL:KIND:TEXT:ITEM[:CARGO]".to_owned());
                }
                start = Start::Message {
                    level: n[0],
                    counter: n.get(5).copied().unwrap_or(0) as u32,
                    subject: solar_jetman::game::scene::message::Subject {
                        kind: n[1] as u8,
                        text: n[2] as u8,
                        item: n[3] as u8,
                        cargo: n.get(4).copied().unwrap_or(0) as u8,
                    },
                };
            }
            "--layout" => {
                // Prints the pieces of table B layout N (a development aid).
                let value = args.next().ok_or("--layout needs a frame number")?;
                let frame = usize::from_str_radix(value.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("bad frame `{value}`"))?;
                let (_, checked) = solar_jetman::tools::load_rom(repo, rom.clone())?;
                let layouts = solar_jetman::rom::extract::sprite_file(&checked)?;
                for piece in &layouts.table_b[frame] {
                    println!(
                        "dx {} dy {} tile {:02X} attributes {:02X}",
                        piece.dx, piece.dy, piece.tile, piece.attributes
                    );
                }
                return Ok(());
            }
            "--gap" => {
                let value = args.next().ok_or("--gap needs FRAME:N")?;
                let (at, n) = value.split_once(':').ok_or("--gap is FRAME:N")?;
                gaps.push((
                    at.parse().map_err(|_| "bad --gap frame")?,
                    n.parse().map_err(|_| "bad --gap count")?,
                ));
            }
            "--original" => original = Some(true),
            "--remake" => original = Some(false),
            "--compare" => {
                compare = Some(PathBuf::from(
                    args.next().ok_or("--compare needs a folder")?,
                ));
            }
            "--offset" => {
                let value = args.next().ok_or("--offset needs a number")?;
                offset = Some(value.parse::<i64>().map_err(|_| "bad --offset")?);
            }
            "--frames" => {
                let list = args.next().ok_or("--frames needs a list")?;
                frames = list
                    .split(',')
                    .map(|f| f.parse::<i64>().map_err(|_| format!("bad frame `{f}`")))
                    .collect::<Result<_, _>>()?;
            }
            "--input" => {
                for item in args.next().ok_or("--input needs a list")?.split(',') {
                    let (frame, buttons) = item.split_once(':').ok_or("input is FRAME:BUTTONS")?;
                    let frame: i64 = frame
                        .parse()
                        .map_err(|_| format!("bad frame in `{item}`"))?;
                    let mut mask = 0;
                    for name in buttons.split('+') {
                        mask |= match name {
                            "start" => Buttons::START,
                            "select" => Buttons::SELECT,
                            "a" => Buttons::A,
                            "b" => Buttons::B,
                            "up" => Buttons::UP,
                            "down" => Buttons::DOWN,
                            "left" => Buttons::LEFT,
                            "right" => Buttons::RIGHT,
                            other => return Err(format!("unknown button `{other}`")),
                        };
                    }
                    inputs.push((frame, mask));
                }
            }
            "--out" => out = PathBuf::from(args.next().ok_or("--out needs a folder")?),
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if rom.is_none() && !arg.starts_with('-') => rom = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`\n{USAGE}")),
        }
    }
    if frames.is_empty() {
        return Err(format!("--frames is required\n{USAGE}"));
    }
    let (_, rom) = solar_jetman::tools::load_rom(repo, rom)?;
    let assets = solar_jetman::tools::assets(&rom)?;
    let offset = offset.unwrap_or(match start {
        Start::Cold => COLD_OFFSET,
        Start::Story => STORY_OFFSET,
        Start::Flight(_) => FLIGHT_OFFSET,
        _ => WARM_OFFSET,
    });
    let original = original.unwrap_or(compare.is_some());
    let mut game = Game::with_rules(&assets, start, original).map_err(|error| error.to_string())?;
    if original {
        game.set_validation_input(solar_jetman::rom::extract::validation_input(&rom).ok());
    }
    if let Some(rng) = rng {
        game.set_random(rng);
    }
    if let Some(job) = job {
        game.set_job(job);
    }
    let rgb = solar_jetman::game::ntsc::master_palette();
    frames.sort_unstable();
    fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    gaps.sort_unstable();
    // The original frame in which pass `p` runs.
    let frame_of = |pass: i64| {
        let mut at = pass + offset;
        for &(after, count) in &gaps {
            if at > after {
                at += count;
            }
        }
        at
    };
    // The sound requests need the sound data from the ROM; the drivers' answers (effects
    // started, songs called) are what the original's log holds.
    let mut sound = solar_jetman::tools::replay::SoundModel::new(
        &solar_jetman::tools::sound::sound_data(&rom)
            .map_err(|error| format!("sound data: {error}"))?,
    );
    // What the scene's construction asked for was asked before its first picture: it is not
    // compared, but the driver knows which song is playing.
    for request in game.take_sound() {
        match request {
            SoundRequest::Request(request) => sound.sound.apply(request),
            SoundRequest::SongIfNew(id) | SoundRequest::SongLooped(id) => sound
                .sound
                .apply(solar_jetman::game::sound::Request::Song { id, always: false }),
            SoundRequest::Pause | SoundRequest::Resume => {}
        }
    }
    let mut sound_log: std::collections::BTreeMap<i64, Vec<String>> = Default::default();
    let mut log_sound = |game: &mut Game, pass_frame: i64, counter: u32| {
        let requests: Vec<_> = game
            .take_sound()
            .into_iter()
            .filter_map(|request| match request {
                SoundRequest::Request(request) => Some(request),
                // The original asks for the title song (looped here) as "start if different".
                SoundRequest::SongIfNew(id) | SoundRequest::SongLooped(id) => {
                    Some(solar_jetman::game::sound::Request::Song { id, always: false })
                }
                SoundRequest::Pause | SoundRequest::Resume => None,
            })
            .collect();
        if std::env::var_os("SJ_DIFF").is_some() && !requests.is_empty() {
            println!("  requests in the pass of frame {pass_frame}: {requests:?}");
        }
        let mut tokens = Vec::new();
        sound.pass(0, counter, requests, &mut tokens);
        if !tokens.is_empty() {
            // "e0.1.2A" -> "e.1.2A": the pass parity is not logged here.
            sound_log
                .entry(pass_frame)
                .or_default()
                .extend(tokens.iter().map(|t| format!("{}{}", &t[..1], &t[2..])));
        }
    };
    let mut passes = 0i64;
    let mut frame = Frame::default();
    let mut shots: Vec<(i64, Vec<u8>)> = Vec::new();
    for &shown in &frames {
        // The state shown at `shown` is the one left by the pass of frame `shown - 1` (or the
        // last pass before it, in a loading gap).
        while frame_of(passes + 1) < shown {
            passes += 1;
            let callback = frame_of(passes);
            let buttons = inputs
                .iter()
                .filter(|&&(from, _)| (from..from + 2).contains(&callback))
                .fold(0, |held, &(_, mask)| held | mask);
            let counter = game.sound_counter();
            game.pass(&assets, Buttons(buttons))
                .map_err(|error| error.to_string())?;
            log_sound(&mut game, callback, counter);
        }
        game.draw(&mut frame, &assets, 1.0);
        let base = out.join(format!("scene-{shown}"));
        write_png(&base.with_extension("png"), &frame, &rgb)?;
        let indices: Vec<u8> = frame.pixels.iter().map(|&c| c & 0x3F).collect();
        fs::write(base.with_extension("idx"), indices)
            .map_err(|error| format!("{}: {error}", base.display()))?;
        if let Some(screen) = game.current_screen() {
            // Tile memory then the page, as the capture script's ".chr" file.
            let mut dump = screen.chr.bytes().to_vec();
            dump.extend_from_slice(screen.page_bytes());
            fs::write(base.with_extension("chr"), dump)
                .map_err(|error| format!("{}: {error}", base.display()))?;
            if std::env::var_os("SJ_SPRITES").is_some() {
                println!("  palette {:02X?}", screen.palette.0);
                for s in &screen.sprites {
                    println!(
                        "  sprite x {} y {} tile {:02X} palette {} behind {} flip {}{}",
                        s.x, s.y, s.tile, s.palette, s.behind, s.flip_x, s.flip_y
                    );
                }
            }
        }
        println!("frame {shown}: {} ({})", base.display(), game.scene_name());
        shots.push((shown, frame.pixels.iter().map(|&c| c & 0x3F).collect()));
    }
    if let Some(dir) = compare {
        compare_with_original(&dir, &shots)?;
        compare_sound(&dir, &sound_log, frame_of(1), frame_of(passes))?;
    }
    Ok(())
}

fn write_png(path: &std::path::Path, frame: &Frame, rgb: &[[u8; 3]; 64]) -> Result<(), String> {
    let pixels: Vec<u8> = frame
        .pixels
        .iter()
        .flat_map(|&colour| rgb[usize::from(colour & 0x3F)])
        .collect();
    let file = fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

/// One screenshot of the original: 256 × 240 RGB pixels.
fn read_shot(path: &std::path::Path) -> Result<Vec<[u8; 3]>, String> {
    let file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut data = vec![0; reader.output_buffer_size().ok_or("screenshot too large")?];
    let info = reader
        .next_frame(&mut data)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let step = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("{}: unsupported {other:?}", path.display())),
    };
    if (info.width, info.height) != (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32) {
        return Err(format!("{}: not 256 × 240", path.display()));
    }
    Ok(data
        .chunks_exact(step)
        .map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// Lines of the original's picture where the console is known to differ from the remake: those
/// more than 8 sprites cross (it drops sprites there), from its sprite memory in
/// `DIR/shot-FRAME.txt` (the second line; the screenshot shows the memory of the frame before),
/// and the 8 topmost and bottommost lines (a sprite's y wraps round at the edges, so one
/// partly above the top is hidden, or shown at the bottom, as a whole).
fn console_lines(dir: &std::path::Path, frame: i64) -> Vec<bool> {
    let mut crowded: Vec<bool> = (0..SCREEN_HEIGHT)
        .map(|row| !(8..SCREEN_HEIGHT - 8).contains(&row))
        .collect();
    let Ok(text) = fs::read_to_string(dir.join(format!("shot-{}.txt", frame - 1))) else {
        return crowded;
    };
    let Some(line) = text.lines().nth(1) else {
        return crowded;
    };
    let bytes: Vec<u8> = line
        .split_whitespace()
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect();
    let mut count = vec![0; SCREEN_HEIGHT];
    for sprite in bytes.chunks(4).filter(|s| s.len() == 4 && s[0] < 0xEF) {
        let top = usize::from(sprite[0]) + 1;
        for lines in count.iter_mut().skip(top).take(8) {
            *lines += 1;
        }
    }
    for (row, (line, &n)) in crowded.iter_mut().zip(&count).enumerate() {
        *line = n > 8 || !(8..SCREEN_HEIGHT - 8).contains(&row);
    }
    crowded
}

/// Compares our frames with the original's screenshots. The two use different colour tables, so
/// each NES colour number is taken to be the RGB value it most often has in the original's
/// screenshots; a pixel differs when the original's value is another one. Differences on lines
/// that more than 8 sprites cross are counted apart: the console drops sprites there.
/// Compares the sound requests of the passes in the frames `first` to `last` with the original's
/// log ("sound.txt" of the capture script, lines "FRAME e CHANNEL.ID" and "FRAME m KIND.SONG").
/// A capture without the file is skipped.
fn compare_sound(
    dir: &std::path::Path,
    ours: &std::collections::BTreeMap<i64, Vec<String>>,
    first: i64,
    last: i64,
) -> Result<(), String> {
    use std::collections::BTreeMap;
    let Ok(text) = fs::read_to_string(dir.join("sound.txt")) else {
        return Ok(());
    };
    let mut theirs: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(frame), Some(kind), Some(what)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let frame: i64 = frame
            .parse()
            .map_err(|_| format!("bad sound line `{line}`"))?;
        if (first..=last).contains(&frame) {
            theirs
                .entry(frame)
                .or_default()
                .push(format!("{kind}.{what}"));
        }
    }
    let ours: BTreeMap<i64, Vec<String>> = ours
        .iter()
        .filter(|(frame, _)| (first..=last).contains(frame))
        .map(|(&frame, tokens)| (frame, tokens.clone()))
        .collect();
    let frames: std::collections::BTreeSet<i64> =
        theirs.keys().chain(ours.keys()).copied().collect();
    let (mut equal, mut shown) = (0, 0);
    for &frame in &frames {
        let (mut a, mut b) = (
            theirs.get(&frame).cloned().unwrap_or_default(),
            ours.get(&frame).cloned().unwrap_or_default(),
        );
        // Several requests of one pass are compared as a set (the order of an effect and a song
        // does not matter).
        a.sort();
        b.sort();
        if a == b {
            equal += 1;
        } else {
            shown += 1;
            println!("sound {frame}: original {a:?}, ours {b:?}");
        }
    }
    println!(
        "sound: {equal}/{} frames with requests match (frames {first}-{last}){}",
        frames.len(),
        if shown == 0 {
            ""
        } else {
            ", the others are listed"
        }
    );
    Ok(())
}

fn compare_with_original(dir: &std::path::Path, shots: &[(i64, Vec<u8>)]) -> Result<(), String> {
    use std::collections::BTreeMap;
    let mut originals = Vec::new();
    let mut votes: BTreeMap<u8, BTreeMap<[u8; 3], usize>> = BTreeMap::new();
    for (frame, ours) in shots {
        let theirs = read_shot(&dir.join(format!("shot-{frame}.png")))?;
        for (&colour, &rgb) in ours.iter().zip(&theirs) {
            *votes.entry(colour).or_default().entry(rgb).or_default() += 1;
        }
        originals.push(theirs);
    }
    let expected: BTreeMap<u8, [u8; 3]> = votes
        .into_iter()
        .filter_map(|(colour, counts)| {
            counts
                .into_iter()
                .max_by_key(|&(_, n)| n)
                .map(|(rgb, _)| (colour, rgb))
        })
        .collect();
    let mut unexplained_total = 0;
    for ((frame, ours), theirs) in shots.iter().zip(&originals) {
        let console = console_lines(dir, *frame);
        let (mut differing, mut unexplained) = (0, 0);
        for (i, (&colour, rgb)) in ours.iter().zip(theirs).enumerate() {
            if expected.get(&colour) != Some(rgb) {
                differing += 1;
                if std::env::var_os("SJ_DIFF").is_some() && !console[i / SCREEN_WIDTH] {
                    println!(
                        "  differs at x {} y {}: ours colour {colour:02X}",
                        i % SCREEN_WIDTH,
                        i / SCREEN_WIDTH
                    );
                }
                if !console[i / SCREEN_WIDTH] {
                    unexplained += 1;
                }
            }
        }
        unexplained_total += unexplained;
        if differing == 0 {
            println!("compare {frame}: identical");
        } else {
            println!(
                "compare {frame}: {differing} pixels differ, {unexplained} elsewhere than on crowded or edge lines"
            );
        }
    }
    println!("compare: {unexplained_total} unexplained pixels in all");
    Ok(())
}
