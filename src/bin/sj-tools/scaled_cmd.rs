//! `sj-tools scaled-compare`.

use solar_jetman::tools::replay::{parse_capture, parse_replay};
use solar_jetman::tools::scaled::{
    BUDGET, Errors, Mapping, Run, SWITCHING, TEST_RATES, at_step_rate, passes, reference, run,
};
use std::fs;
use std::path::{Path, PathBuf};

/// `sj-tools scaled-compare [--rates R,R,...] [--demos] [--thrust-lag] [--against-30]
/// [--window STEPS] [--switch] [REPLAY...]`: plays the replays (or with
/// `--demos` the attract demos, flown by the Original's autopilot) in the Scaled engine at the
/// test rates and compares them with the Original within the error budgets (`docs/scaled.md`). The
/// Original's start states come from the captures `replay-compare` keeps in
/// `vendor/captures/replay/`. REPLAY is a path or the name of a file in `tools/mesen/replays/`;
/// by default all of them. `--thrust-lag` gives Scaled B one step late, as the Original's
/// thrust acts, which leaves the rules' own differences; `--against-30` compares with Scaled
/// itself at a whole step a tick instead of the Original, which leaves what the ticks add.
/// Scaled restarts from the Original's state every `--window` steps (30, one second, by
/// default; 0 never), as the lab measured. `--switch` changes the rate during each run.
pub fn run_command(repo: &Path, args: &[String]) -> Result<(), String> {
    let mut rates = TEST_RATES.to_vec();
    let mut demos = false;
    let mut mapping = Mapping::AsPlayed;
    let mut against_step_rate = false;
    let mut window = Some(30);
    let mut replays: Vec<PathBuf> = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demos" => demos = true,
            "--thrust-lag" => mapping = Mapping::ThrustLag,
            "--against-30" => against_step_rate = true,
            "--switch" => rates = vec![SWITCHING],
            "--window" => {
                let steps = args.next().ok_or("--window needs a number of steps")?;
                let steps: usize = steps.parse().map_err(|_| format!("bad window `{steps}`"))?;
                window = (steps > 0).then_some(steps);
            }
            "--rates" => {
                let list = args.next().ok_or("--rates needs a list")?;
                rates = list
                    .split(',')
                    .map(|r| r.parse().map_err(|_| format!("bad rate `{r}`")))
                    .collect::<Result<_, _>>()?;
            }
            _ if !arg.starts_with('-') => {
                let direct = PathBuf::from(arg);
                let folder = if arg.starts_with("demo-") {
                    "tools/mesen/demos"
                } else {
                    "tools/mesen/replays"
                };
                let named = repo.join(folder).join(format!("{arg}.replay"));
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
    println!(
        "budget: pod {} px, camera {} px, facing {}",
        BUDGET.pod, BUDGET.camera, BUDGET.facing
    );
    // Each replay on its own thread; the reports in the replays' order.
    let results: Vec<Result<Compared, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = replays
            .iter()
            .map(|path| {
                let (world, rates) = (&world, &rates);
                scope.spawn(move || {
                    compare_one(
                        repo,
                        world,
                        path,
                        rates,
                        How {
                            mapping,
                            against_step_rate,
                            window,
                        },
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a comparison thread panicked"))
            .collect()
    });
    let mut totals: Vec<Vec<Run>> = vec![Vec::new(); rates.len()];
    for result in results {
        let (name, note, runs) = result?;
        println!("{name}{note}");
        for (i, run) in runs.into_iter().enumerate() {
            println!("  {}", describe(&run));
            totals[i].push(run);
        }
    }
    println!("all replays:");
    for runs in &totals {
        println!("  {}", summarise(runs));
    }
    Ok(())
}

/// How the comparison runs (the command's options).
#[derive(Debug, Clone, Copy)]
struct How {
    mapping: Mapping,
    against_step_rate: bool,
    window: Option<usize>,
}

/// One replay's comparison at every rate: its name, a note on how far the Original played it,
/// and the runs.
type Compared = (String, String, Vec<Run>);

fn compare_one(
    repo: &Path,
    world: &solar_jetman::game::level::World,
    path: &Path,
    rates: &[u32],
    how: How,
) -> Result<Compared, String> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let replay = parse_replay(&text).map_err(|error| format!("{name}: {error}"))?;
    let log = repo
        .join("vendor/captures/replay")
        .join(format!("{name}.log"));
    let text = fs::read_to_string(&log).map_err(|error| {
        format!(
            "{}: {error} (`sj-tools replay-compare {name}` captures it)",
            log.display()
        )
    })?;
    let capture = parse_capture(&text).map_err(|error| format!("{}: {error}", log.display()))?;
    let reference = match reference(world, &replay, &capture, None, how.window) {
        Ok(reference) if how.against_step_rate => at_step_rate(world, &reference),
        Ok(reference) => reference,
        Err(why) => return Ok((name, format!(": not compared, {why}"), Vec::new())),
    };
    let steps: usize = reference.stretches.iter().map(|s| s.buttons.len()).sum();
    let mut note = format!(
        ": {steps} steps in {} stretch(es)",
        reference.stretches.len()
    );
    if let Some(stopped) = &reference.stopped {
        note += &format!(", the Original stopped at {stopped}");
    }
    let runs = rates
        .iter()
        .map(|&rate| run(world, &reference, rate, BUDGET, how.mapping))
        .collect();
    Ok((name, note, runs))
}

fn describe(run: &Run) -> String {
    let out = run.first_out.map_or_else(
        || "never out".to_owned(),
        |step| {
            format!(
                "first out at step {step} ({:.1} s)",
                run.seconds_in_budget()
            )
        },
    );
    let rate = if run.rate == SWITCHING {
        "switching".to_owned()
    } else {
        format!("{:>3} Hz", run.rate)
    };
    format!(
        "{rate}: {} steps, pod rms {:.2} p90 {:.2} max {:.2} px, camera rms {:.2} max {:.2} \
         px, facing max {}, in budget {:.1}%, {out}{}",
        run.steps,
        run.pod.rms(),
        run.pod.percentile(90),
        run.pod.max,
        run.camera.rms(),
        run.camera.max,
        run.facing.max,
        percent(run.in_budget, run.steps),
        if run.stopped_early > 0 {
            format!(", Scaled stopped {} steps early", run.stopped_early)
        } else {
            String::new()
        },
    )
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        100.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

/// One rate over all replays: the errors pooled, and the median time a run stays in budget.
fn summarise(runs: &[Run]) -> String {
    let pool = |pick: fn(&Run) -> &Errors| {
        let mut pooled = Errors::default();
        for run in runs {
            pooled.merge(pick(run));
        }
        pooled
    };
    let (pod, camera, facing) = (pool(|r| &r.pod), pool(|r| &r.camera), pool(|r| &r.facing));
    let steps: usize = runs.iter().map(|r| r.steps).sum();
    let in_budget: usize = runs.iter().map(|r| r.in_budget).sum();
    let mut seconds: Vec<f64> = runs.iter().map(Run::seconds_in_budget).collect();
    seconds.sort_by(f64::total_cmp);
    let median = seconds.get(seconds.len() / 2).copied().unwrap_or(0.0);
    let rate = runs.first().map_or(0, |r| r.rate);
    let rate = if rate == SWITCHING {
        "switching".to_owned()
    } else {
        format!("{rate:>3} Hz")
    };
    format!(
        "{rate}: {} runs, {steps} steps, pod rms {:.2} p50 {:.3} p90 {:.2} p99 {:.2} max \
         {:.2} px, camera rms {:.2} p90 {:.2} max {:.2} px, facing p99 {:.0} max {:.0}, in \
         budget {:.1}%, median {median:.1} s before the first step out: {}",
        runs.len(),
        pod.rms(),
        pod.percentile(50),
        pod.percentile(90),
        pod.percentile(99),
        pod.max,
        camera.rms(),
        camera.percentile(90),
        camera.max,
        facing.percentile(99),
        facing.max,
        percent(in_budget, steps),
        if passes(&pod, &camera, &facing, BUDGET) {
            "within the budget"
        } else {
            "OUTSIDE the budget"
        },
    )
}
