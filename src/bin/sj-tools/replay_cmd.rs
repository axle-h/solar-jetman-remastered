//! `sj-tools replay-compare`.

use solar_jetman::tools::replay::{
    Report, ScreenCheck, compare_with_sound, parse_capture, parse_replay,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SCRIPT: &str = "tools/mesen/replay-capture.lua";
const PALETTE_SCRIPT: &str = "tools/mesen/palette-capture.lua";

/// Side-by-side pictures written per replay for the steps whose screens differ.
const KEEP_PICTURES: usize = 20;

/// `sj-tools replay-compare [--recapture] [--lockstep] [--demos] [--screens] [REPLAY...]`: plays the replays in the
/// original (Mesen, headless; captures are kept in `vendor/captures/replay/`) and in `src/game`,
/// and reports per field where the two first diverge and how many steps match. `--screens` also
/// compares the picture after every step with the original's (captures in
/// `vendor/captures/replay-shots/`; the first differing steps side by side, original, ours and
/// the differences, in `vendor/compare/replay/REPLAY/`). REPLAY is a
/// path or the name of a file in `tools/mesen/replays/`; by default all of them. `--demos` plays
/// the attract demos of `tools/mesen/demos/` instead, in lockstep (the pod and objects are taken
/// from the original after every step, so each step's autopilot bytes, waypoint and shield timer
/// are judged on their own).
pub fn run(repo: &Path, args: &[String]) -> Result<(), String> {
    let (mut recapture, mut lockstep, mut demos, mut screens) = (false, false, false, false);
    let mut replays: Vec<PathBuf> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--recapture" => recapture = true,
            "--lockstep" => lockstep = true,
            "--screens" => screens = true,
            "--demos" => {
                demos = true;
                lockstep = true;
            }
            _ if !arg.starts_with('-') => {
                let direct = PathBuf::from(arg);
                let named = repo
                    .join(if arg.starts_with("demo-") {
                        "tools/mesen/demos"
                    } else {
                        "tools/mesen/replays"
                    })
                    .join(format!("{arg}.replay"));
                replays.push(if direct.is_file() { direct } else { named });
            }
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    if replays.is_empty() {
        let dir = repo.join(if demos {
            "tools/mesen/demos"
        } else {
            "tools/mesen/replays"
        });
        let entries = fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        replays = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "replay"))
            .collect();
        replays.sort();
    }
    let world = solar_jetman::tools::default_world(repo)?;
    // The sound requests are compared through the driver's data, which needs the ROM.
    let sound = solar_jetman::tools::load_rom(repo, None)
        .and_then(|(_, rom)| solar_jetman::tools::sound::sound_data(&rom))
        .map_err(|error| eprintln!("sound requests are not compared: {error}"))
        .ok();
    // The original's rules on the warp levels 37 and 39 read bytes of the ROM itself.
    let validation = solar_jetman::tools::load_rom(repo, None)
        .and_then(|(_, rom)| solar_jetman::rom::extract::validation_input(&rom))
        .map_err(|error| eprintln!("the warp levels 37 and 39 are not validated: {error}"))
        .ok();
    let pictures = if screens {
        Some(screen_assets(repo)?)
    } else {
        None
    };
    let jobs: Vec<Job> = replays
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let shots = screens.then(|| repo.join("vendor/captures/replay-shots").join(&name));
            let log = match &shots {
                Some(shots) => shots.join("capture.log"),
                None => repo
                    .join("vendor/captures/replay")
                    .join(format!("{name}.log")),
            };
            Job {
                name,
                path,
                shots,
                log,
            }
        })
        .collect();
    capture_all(repo, &jobs, recapture)?;
    for job in &jobs {
        let Job {
            name, path, shots, ..
        } = job;
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let replay = parse_replay(&text).map_err(|error| format!("{name}: {error}"))?;
        let text = fs::read_to_string(&job.log)
            .map_err(|error| format!("{}: {error}", job.log.display()))?;
        let capture =
            parse_capture(&text).map_err(|error| format!("{}: {error}", job.log.display()))?;
        let mut check = match (&pictures, shots) {
            (Some((assets, palette)), Some(shots)) => {
                let out = repo.join("vendor/compare/replay").join(name);
                // Old pictures of this replay would be taken for new differences.
                let _ = fs::remove_dir_all(&out);
                fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
                let mut check =
                    ScreenCheck::new(&assets.graphics, &assets.layouts, *palette, shots.clone());
                check.out = Some(out);
                // `SJ_KEEP=N` keeps up to N pictures instead (a debugging aid).
                check.keep = std::env::var("SJ_KEEP")
                    .ok()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(KEEP_PICTURES);
                // `SJ_PICTURES=FROM-TO` writes the pictures of those steps instead (a debugging aid).
                check.only = std::env::var("SJ_PICTURES").ok().and_then(|range| {
                    let (from, to) = range.split_once('-')?;
                    Some((from.parse().ok()?, to.parse().ok()?))
                });
                Some(check)
            }
            _ => None,
        };
        let report = compare_with_sound(
            &world,
            &replay,
            &capture,
            lockstep,
            sound.as_ref(),
            validation.as_ref(),
            check.as_mut(),
        );
        print_report(name, lockstep, &report);
    }
    Ok(())
}

/// What drawing the remake's pictures needs (the imported graphics), and Mesen's colours,
/// measured once into `vendor/captures/mesen-palette.txt`.
fn screen_assets(
    repo: &Path,
) -> Result<
    (
        solar_jetman::game::scene::Assets,
        solar_jetman::tools::screen::MesenPalette,
    ),
    String,
> {
    let (_, rom) = solar_jetman::tools::load_rom(repo, None)?;
    let assets = solar_jetman::tools::assets(&rom)?;
    let path = repo.join("vendor/captures/mesen-palette.txt");
    if !path.is_file() {
        eprintln!("measuring Mesen's colours ...");
        let status = Command::new(repo.join("tools/mesen/run.sh"))
            .arg(repo.join(PALETTE_SCRIPT))
            .env("SJ_PALETTE_OUT", &path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("running Mesen: {error}"))?;
        if !status.success() {
            return Err(format!("measuring Mesen's colours failed ({status})"));
        }
    }
    Ok((assets, solar_jetman::tools::screen::read_palette(&path)?))
}

/// One replay to compare: its file, and where the original's log (and with `--screens` its
/// pictures) are kept.
struct Job {
    name: String,
    path: PathBuf,
    shots: Option<PathBuf>,
    log: PathBuf,
}

/// Runs the original in Mesen for every job whose log is missing or older than the replay or the
/// script (all with `recapture`), several at once.
fn capture_all(repo: &Path, jobs: &[Job], recapture: bool) -> Result<(), String> {
    let modified = |path: &Path| fs::metadata(path).and_then(|m| m.modified()).ok();
    let script = modified(&repo.join(SCRIPT));
    let stale: Vec<&Job> = jobs
        .iter()
        .filter(|job| {
            recapture
                || !matches!(
                    (modified(&job.log), modified(&job.path), script),
                    (Some(log), Some(replay), Some(script)) if log > replay && log > script
                )
        })
        .collect();
    // Each Mesen runs one core flat out; leave the machine some room.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get() / 2)
        .clamp(1, 6);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let errors = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers.min(stale.len()) {
            scope.spawn(|| {
                while let Some(job) =
                    stale.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    if let Err(error) = capture(repo, job) {
                        errors.lock().unwrap().push(error);
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// A fresh run of the original in Mesen for one job.
fn capture(repo: &Path, job: &Job) -> Result<(), String> {
    let dir = job.log.parent().unwrap_or(repo);
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    eprintln!("capturing {} in Mesen ...", job.name);
    let mut command = Command::new(repo.join("tools/mesen/run.sh"));
    if let Some(shots) = &job.shots {
        command.env("SJ_REPLAY_SHOTS", shots);
    }
    let status = command
        .arg(repo.join(SCRIPT))
        .env(
            "SJ_REPLAY_IN",
            job.path.canonicalize().map_err(|e| e.to_string())?,
        )
        .env("SJ_REPLAY_OUT", &job.log)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("running Mesen: {error}"))?;
    if !status.success() {
        return Err(format!("{}: Mesen failed ({status})", job.name));
    }
    Ok(())
}

fn print_divergence(report: &Report) {
    let Some(d) = &report.first_divergence else {
        return;
    };
    println!("  first divergence (other than rng), step {}:", d.step);
    let mut before = String::new();
    let mut original = String::new();
    let mut ours = String::new();
    for &(name, value) in &d.ours {
        if name == "rng" || name == "ctr" {
            continue;
        }
        let theirs = d.original.get(name).copied().unwrap_or(0);
        let prev = d.before.get(name).copied().unwrap_or(0);
        let mark = if theirs == value { ' ' } else { '*' };
        before += &format!(" {name}={prev:X}");
        original += &format!(" {name}={theirs:X}");
        ours += &format!(" {name}={value:X}{mark}");
    }
    println!("    before:  {before}\n    original:{original}\n    ours:    {ours}");
}

fn print_report(name: &str, lockstep: bool, report: &Report) {
    let mode = if lockstep { "lockstep" } else { "free-running" };
    println!(
        "{name} ({mode}): {} steps, {} compared, {} cut",
        report.steps, report.compared, report.cut
    );
    for (field, theirs, ours) in &report.init {
        println!("  start differs: {field} original {theirs:X}, ours {ours:X}");
    }
    let percent = |matched: usize, total: usize| 100.0 * matched as f64 / total.max(1) as f64;
    println!(
        "  every field: {}/{} steps ({:.1}%)",
        report.exact,
        report.compared,
        percent(report.exact, report.compared)
    );
    for (field, result) in &report.fields {
        if let Some((step, theirs, ours)) = result.first {
            println!(
                "  {field}: {}/{} ({:.1}%), first differs at step {step}: original {theirs:X}, ours {ours:X}",
                result.matched,
                result.total,
                percent(result.matched, result.total)
            );
        }
    }
    println!(
        "  objects: every field {}/{} steps ({:.1}%); pod, score and objects all {}/{} ({:.1}%)",
        report.objects_exact,
        report.compared,
        percent(report.objects_exact, report.compared),
        report.all_exact,
        report.compared,
        percent(report.all_exact, report.compared)
    );
    for (field, result) in &report.objects {
        if let Some((step, theirs, ours)) = result.first {
            let show = |v: u32| {
                if v == u32::MAX {
                    "absent".to_owned()
                } else {
                    format!("{v:X}")
                }
            };
            println!(
                "    {field}: {}/{} ({:.1}%), first differs at step {step} ({}): original {}, ours {}",
                result.matched,
                result.total,
                percent(result.matched, result.total),
                result.at,
                show(theirs),
                show(ours)
            );
        }
    }
    println!(
        "  attached slots 1-2: every field {}/{} steps ({:.1}%)",
        report.attached_exact,
        report.compared,
        percent(report.attached_exact, report.compared)
    );
    if let Some((step, field, theirs, ours)) = &report.attached_first {
        let show = |v: u32| {
            if v == u32::MAX {
                "absent".to_owned()
            } else {
                format!("{v:X}")
            }
        };
        println!(
            "    first differs at step {step} ({field}): original {}, ours {}",
            show(*theirs),
            show(*ours)
        );
    }
    if report.sound_compared > 0 {
        println!(
            "  sound requests: {}/{} steps ({:.1}%)",
            report.sound_exact,
            report.sound_compared,
            percent(report.sound_exact, report.sound_compared)
        );
        if let Some((step, theirs, ours)) = &report.sound_first {
            println!("    first differs at step {step}: original {theirs:?}, ours {ours:?}");
        }
    }
    let autopilot: Vec<_> = report
        .fields
        .iter()
        .filter(|(name, _)| matches!(*name, "wp" | "tm" | "ih" | "ip"))
        .collect();
    if let Some(least) = autopilot.iter().map(|(_, field)| field.matched).min() {
        println!(
            "  autopilot (waypoint, shield timer, held and pressed bytes): every one right in {}/{} steps",
            least, report.compared
        );
    }
    if let Some(ours) = report.our_transition.filter(|&(_, code)| code == 0) {
        println!("  the demo ended at step {} in ours", ours.0);
    }
    if report.screen_compared > 0 {
        println!(
            "  screens: {}/{} steps identical outside the masked lines ({:.1}%)",
            report.screen_exact,
            report.screen_compared,
            percent(report.screen_exact, report.screen_compared)
        );
        if report.screen_skipped > 0 {
            println!(
                "    {} steps around a warp not compared (the original's redraw settling)",
                report.screen_skipped
            );
        }
        // `SJ_SCREENS` in the environment lists every differing step (a debugging aid).
        let shown = if std::env::var_os("SJ_SCREENS").is_some() {
            usize::MAX
        } else {
            5
        };
        for (step, diff) in report.screen_steps.iter().take(shown) {
            println!(
                "    step {step}: {} pixels differ, in {:?} (left, top, right, bottom)",
                diff.unexplained,
                diff.area.unwrap_or_default()
            );
        }
        if report.screen_steps.len() > shown {
            println!(
                "    ... and {} more steps",
                report.screen_steps.len() - shown
            );
        }
    }
    print_divergence(report);
    if let Some(left) = report.original_left {
        match report.our_transition {
            Some((step, code)) if step == left => {
                println!("  the original left gameplay at step {left}, so did we (code {code})");
            }
            Some((step, code)) => println!(
                "  the original left gameplay at step {left}; we asked for code {code} at step {step}"
            ),
            None => println!("  the original left gameplay at step {left}; we did not"),
        }
    } else if let Some((step, code)) = report.our_transition
        && report.reloads.iter().all(|(s, _)| *s != step)
    {
        println!("  we asked for code {code} at step {step}; the original did not leave");
    }
    for (step, diffs) in &report.reloads {
        if diffs.is_empty() {
            println!("  reload after step {step}: all fields match");
        } else {
            let list: Vec<String> = diffs
                .iter()
                .map(|(name, theirs, ours)| format!("{name} {theirs:X}/{ours:X}"))
                .collect();
            println!(
                "  reload after step {step}: differs (original/ours): {}",
                list.join(" ")
            );
        }
    }
}
