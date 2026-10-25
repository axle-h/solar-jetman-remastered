//! `sj-tools render`.

use solar_jetman::game::SCREEN_HEIGHT;
use solar_jetman::game::graphics::LevelGraphics;
use solar_jetman::game::level::{LEVEL_COUNT, Point};
use solar_jetman::game::playfield::{Frame, draw_playfield};
use std::fs;
use std::path::{Path, PathBuf};

/// `sj-tools render --level N|all [--x X] [--y Y] [--play STEPS [--replay NAME]] [--aspect W:H] [--out FILE|DIR] [ROM]`: draws
/// a level's playfield headlessly into an RGB PNG (by default under `vendor/renders/`, as the
/// picture is made of the ROM's graphics). Without `--x`/`--y` the camera starts where the
/// level does. With `--play STEPS` the game runs that many logic steps with no input and the
/// picture is the game's, with the pod. `--aspect` draws a wider view (`docs/widescreen.md`).
pub fn render(repo: &Path, args: &[String]) -> Result<(), String> {
    let mut levels = Vec::new();
    let (mut x, mut y) = (None, None);
    let mut out = None;
    let mut rom = None;
    let mut play = None;
    let mut replay = None;
    let mut width = solar_jetman::game::view::ORIGINAL_WIDTH;
    let number = |value: Option<&String>, what: &str| -> Result<u16, String> {
        value
            .ok_or(format!("{what} needs a number"))?
            .parse()
            .map_err(|_| format!("{what} needs a number"))
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--level" => {
                let value = args.next();
                if value.is_some_and(|v| v == "all") {
                    levels = (0..LEVEL_COUNT).collect();
                } else {
                    levels.push(usize::from(number(value, "--level")?));
                }
            }
            "--x" => x = Some(number(args.next(), "--x")?),
            "--y" => y = Some(number(args.next(), "--y")?),
            "--play" => play = Some(usize::from(number(args.next(), "--play")?)),
            "--replay" => replay = Some(args.next().ok_or("--replay needs a name")?.clone()),
            "--aspect" => width = aspect_width(args.next())?,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a path")?)),
            _ if rom.is_none() && !arg.starts_with('-') => rom = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    if levels.is_empty() {
        return Err("--level N (or all) is required".to_owned());
    }
    let (_, rom) = solar_jetman::tools::load_rom(repo, rom)?;
    let assets = solar_jetman::tools::assets(&rom)?;
    let (world, graphics, layouts) = (&assets.world, &assets.graphics, &assets.layouts);
    let rgb = solar_jetman::game::ntsc::master_palette();
    for number in levels {
        let level = world
            .levels
            .get(number)
            .ok_or(format!("no level {number}"))?;
        let camera = level.clamp_camera(Point {
            x: x.unwrap_or(level.camera_start.x),
            y: y.unwrap_or(level.camera_start.y),
        });
        let level_graphics =
            LevelGraphics::new(graphics, level).map_err(|error| error.to_string())?;
        let mut frame = Frame::new(usize::from(width));
        let mut camera = camera;
        if let Some(steps) = play {
            let sprites =
                solar_jetman::game::gameplay::sprites::Sprites::new(graphics, layouts, number, 0)
                    .map_err(|error| error.to_string())?;
            // The original's state at the start of level 0 (the random generator's seed).
            let seed = solar_jetman::game::gameplay::Seed {
                rng: solar_jetman::game::gameplay::rng::Rng {
                    out: 0x78,
                    high: 0xC9,
                    low: 0x03,
                },
                view_width: width,
                ..Default::default()
            };
            let mut game = solar_jetman::game::gameplay::Gameplay::new(world, number, seed);
            let inputs = match &replay {
                Some(name) => {
                    let path = repo.join(format!("tools/mesen/replays/{name}.replay"));
                    let text = std::fs::read_to_string(&path)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    let replay = solar_jetman::tools::replay::parse_replay(&text)?;
                    game.pod.equipment.owned = replay.equipment;
                    replay.steps
                }
                None => Vec::new(),
            };
            for step in 0..steps {
                let buttons = inputs.get(step).copied().unwrap_or_default();
                game.step(world, buttons);
            }
            game.draw(&mut frame, world, &level_graphics, &sprites, layouts, 1.0);
            camera = game.camera.position;
        } else {
            draw_playfield(
                &mut frame,
                world,
                level,
                &level_graphics,
                solar_jetman::game::view::Viewport::around(level, camera, width),
            );
        }
        let file_name = format!("level-{number:02}-{}-{}.png", camera.x, camera.y);
        let target = match &out {
            Some(path) if path.extension().is_some_and(|ext| ext == "png") => path.clone(),
            Some(dir) => dir.join(file_name),
            None => repo.join("vendor/renders").join(file_name),
        };
        write_png(&target, &frame, &rgb)?;
        println!(
            "level {number} at ({}, {}): {}",
            camera.x,
            camera.y,
            target.display()
        );
    }
    Ok(())
}

/// The view width of `--aspect W:H` at 240 lines.
pub(crate) fn aspect_width(value: Option<&String>) -> Result<u16, String> {
    let value = value.ok_or("--aspect needs W:H")?;
    let parse = |n: &str| n.parse::<f64>().ok().filter(|n| *n > 0.0);
    let (w, h) = value
        .split_once(':')
        .and_then(|(w, h)| Some((parse(w)?, parse(h)?)))
        .ok_or(format!("bad --aspect `{value}`: W:H, as 16:9"))?;
    Ok(solar_jetman::game::view::width_for(
        w / h,
        SCREEN_HEIGHT as u32,
    ))
}

pub(crate) fn write_png(path: &Path, frame: &Frame, rgb: &[[u8; 3]; 64]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let pixels: Vec<u8> = frame
        .pixels
        .iter()
        .flat_map(|&colour| rgb[usize::from(colour & 0x3F)])
        .collect();
    let file = fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        frame.width as u32,
        SCREEN_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    writer
        .finish()
        .map_err(|error| format!("{}: {error}", path.display()))
}
