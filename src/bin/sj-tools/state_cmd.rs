//! `sj-tools state`: loads a save state (the inflated bytes of a `slot-N.sav`), plays on from it
//! headless with held buttons and saves pictures, for looking into what a player saw.

use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::playfield::Frame;
use solar_jetman::game::scene::{Game, Start};
use std::fs;
use std::path::PathBuf;

const USAGE: &str = "sj-tools state FILE [--passes N] [--hold BUTTONS] [--input PASS-PASS:BUTTONS,...] [--fps F] [--shots P,P,...|all] [--out DIR] [ROM]
  FILE is a save state, deflated (slot-N.sav) or not. Runs N passes (60 a second) holding
  BUTTONS (joined by +: start select a b up down left right), or the --input ranges, and draws
  F times a second as the program does, printing for each drawn frame the pass, the blend, the
  camera and the pod's point on the screen. --shots saves the pictures drawn right after those
  passes (state-PASS-K.png, K the draw within the pass) in DIR (default vendor/compare/state).";

fn buttons(text: &str) -> Result<u8, String> {
    text.split('+')
        .filter(|b| !b.is_empty())
        .try_fold(0, |mask, b| {
            Ok(mask
                | match b {
                    "start" => Buttons::START,
                    "select" => Buttons::SELECT,
                    "a" => Buttons::A,
                    "b" => Buttons::B,
                    "up" => Buttons::UP,
                    "down" => Buttons::DOWN,
                    "left" => Buttons::LEFT,
                    "right" => Buttons::RIGHT,
                    _ => return Err(format!("bad button `{b}`")),
                })
        })
}

pub fn run(repo: &std::path::Path, args: &[String]) -> Result<(), String> {
    let mut file = None;
    let mut rom = None;
    let (mut passes, mut hold, mut fps) = (60u32, 0u8, 60.0f64);
    let mut inputs: Vec<(u32, u32, u8)> = Vec::new();
    let mut shots: Option<Vec<u32>> = Some(Vec::new());
    let mut out = repo.join("vendor/compare/state");
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--passes" => passes = value()?.parse().map_err(|_| "bad --passes")?,
            "--hold" => hold = buttons(value()?)?,
            "--fps" => fps = value()?.parse().map_err(|_| "bad --fps")?,
            "--out" => out = PathBuf::from(value()?),
            "--input" => {
                for item in value()?.split(',') {
                    let (range, held) = item.split_once(':').ok_or("input is FROM-TO:BUTTONS")?;
                    let (from, to) = range.split_once('-').unwrap_or((range, range));
                    let parse = |n: &str| n.parse::<u32>().map_err(|_| format!("bad `{item}`"));
                    inputs.push((parse(from)?, parse(to)?, buttons(held)?));
                }
            }
            "--shots" => {
                let v = value()?;
                shots = if v == "all" {
                    None
                } else {
                    Some(
                        v.split(',')
                            .map(|p| p.parse().map_err(|_| format!("bad pass `{p}`")))
                            .collect::<Result<_, String>>()?,
                    )
                };
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ if rom.is_none() => rom = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`\n{USAGE}")),
        }
    }
    let file = file.ok_or(USAGE)?;
    let mut bytes = fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    if let Ok(inflated) = miniz_oxide::inflate::decompress_to_vec(&bytes) {
        bytes = inflated;
    }
    let (_, rom) = solar_jetman::tools::load_rom(repo, rom)?;
    let assets = solar_jetman::tools::assets(&rom)?;
    let mut game = Game::new(&assets, Start::Cold).map_err(|error| error.to_string())?;
    game.load_state(&bytes, &assets)
        .map_err(|error| format!("{}: {error}", file.display()))?;
    fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let rgb = solar_jetman::game::ntsc::master_palette();
    let mut frame = Frame::default();
    // The program's clock: a pass every 1/60 s, a picture every 1/fps s.
    let (tick, draw) = (1.0 / 60.0, 1.0 / fps);
    let mut time = 0.0f64;
    let mut done = 0u32;
    while done < passes {
        let mut drawn = 0;
        while time < tick {
            let blend = (time / tick) as f32;
            game.draw(&mut frame, &assets, blend);
            if let Some(gameplay) = game.gameplay() {
                let (camera, pod) = gameplay.view_at(blend);
                println!(
                    "pass {done} blend {blend:.2} camera {} {} pod {} {} screen {} {} counter {} slot2 {} {:02X}",
                    camera.x,
                    camera.y,
                    pod.0,
                    pod.1,
                    pod.0 - i32::from(camera.x),
                    pod.1 - i32::from(camera.y),
                    0,
                    gameplay.objects.slots[2].state,
                    gameplay.objects.slots[2].frame
                );
            }
            if shots.as_ref().is_none_or(|shots| shots.contains(&done)) {
                crate::data_cmd::write_png(
                    &out.join(format!("state-{done}-{drawn}.png")),
                    &frame,
                    &rgb,
                )?;
            }
            drawn += 1;
            time += draw;
        }
        time -= tick;
        let held = inputs
            .iter()
            .filter(|&&(from, to, _)| (from..=to).contains(&done))
            .fold(hold, |held, &(_, _, mask)| held | mask);
        game.pass(&assets, Buttons(held))
            .map_err(|error| error.to_string())?;
        done += 1;
    }
    Ok(())
}
