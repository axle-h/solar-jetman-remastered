//! Development tools: `sj-tools cleanroom`, `import`, `render`, `scene`, `render-audio`,
//! `sound-compare`, `replay-compare`, `scaled-compare`, `state`, `scan-release`.

mod cleanroom;
mod data_cmd;
mod import;
mod replay_cmd;
mod scaled_cmd;
mod scan_cmd;
mod scene_cmd;
mod sound_cmd;
mod state_cmd;

use std::process::ExitCode;

const USAGE: &str = "usage: sj-tools cleanroom
       sj-tools import [--out DIR] [ROM]
       sj-tools render --level N|all [--x X] [--y Y] [--play STEPS [--replay NAME]] [--out FILE.png|DIR] [ROM]
       sj-tools scene [--cold|--warm] --frames F,F,... [--input FRAME:BUTTONS,...] [--out DIR] [ROM]
       sj-tools render-audio (--song N | --effect ID) [--frames F] [--rate HZ] [--out FILE.wav] [ROM]
       sj-tools replay-compare [--recapture] [--lockstep] [REPLAY...]
       sj-tools scaled-compare [--rates R,R,...] [--demos] [--thrust-lag] [--against-30] [--window STEPS] [--switch]
                               [REPLAY...]
       sj-tools sound-compare [--songs all|N|N-M,...] [--no-effects] [--recapture] [--jobs N] [ROM]
       sj-tools state FILE [--passes N] [--hold BUTTONS] [--fps F] [--shots P,...|all] [--out DIR] [ROM]
       sj-tools scan-release [--rom ROM] [BINARY]";

fn main() -> ExitCode {
    let repo = solar_jetman::tools::repo_root();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("cleanroom") => {
            return if cleanroom::run(&repo) == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Some("import") => import::run(&repo, rest),
        Some("render") => data_cmd::render(&repo, rest),
        Some("scan-release") => scan_cmd::run(&repo, rest),
        Some("scene") => scene_cmd::run(&repo, rest),
        Some("render-audio") => sound_cmd::render_audio(&repo, rest),
        Some("replay-compare") => replay_cmd::run(&repo, rest),
        Some("scaled-compare") => scaled_cmd::run_command(&repo, rest),
        Some("state") => state_cmd::run(&repo, rest),
        Some("sound-compare") => sound_cmd::sound_compare(&repo, rest),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("sj-tools {}: {message}", args[0]);
            ExitCode::FAILURE
        }
    }
}
