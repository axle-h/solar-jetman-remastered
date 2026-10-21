//! Gameplay against the original: level 0, imported from the player's ROM, starts as the
//! original does, and every replay captured under `vendor/captures/replay/` (by `sj-tools
//! replay-compare`) matches `src/game` field for field. Skips what is not there, the ROM
//! included.

use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::{Gameplay, Seed};
use solar_jetman::game::level::Point;
use solar_jetman::tools::replay::{compare, compare_with_sound, parse_capture, parse_replay};
use std::fs;

#[test]
fn level_zero_starts_as_the_original_does() {
    let Some(world) = solar_jetman::tools::test_world() else {
        return;
    };
    let game = Gameplay::new(world, 0, Seed::default());
    // Seen in Mesen at the first gameplay frame (levels.md 5.2, player.md 3).
    assert_eq!(game.camera.position, Point { x: 480, y: 96 });
    let snapshot: std::collections::BTreeMap<_, _> = game.snapshot().into_iter().collect();
    assert_eq!((snapshot["x"], snapshot["y"]), (0x027B, 0x00BF));
    assert_eq!(
        (snapshot["vw"], snapshot["vf"], snapshot["vd"]),
        (1, 0x80, 0)
    );
    assert_eq!(snapshot["fuel"], 0xE000);
}

#[test]
fn a_step_without_input_matches_the_original() {
    // From a capture of the first step of free flight: gravity 8 (a = 2) slows the launch
    // speed 1.80 up to 1.7E, and the pod moves 1.7E up from y 191.
    let Some(world) = solar_jetman::tools::test_world() else {
        return;
    };
    let mut game = Gameplay::new(world, 0, Seed::default());
    game.step(world, Buttons(0));
    let snapshot: std::collections::BTreeMap<_, _> = game.snapshot().into_iter().collect();
    assert_eq!((snapshot["y"], snapshot["yf"]), (0xBD, 0x82));
    assert_eq!((snapshot["vw"], snapshot["vf"]), (1, 0x7E));
    assert_eq!(snapshot["ctr"], 0x02_0000);
}

#[test]
fn captured_replays_match() {
    let repo = solar_jetman::tools::repo_root();
    let Some(world) = solar_jetman::tools::test_world() else {
        return;
    };
    // The sound requests are compared too where the ROM gives the driver's data.
    let sound = solar_jetman::tools::load_rom(&repo, None)
        .and_then(|(_, rom)| solar_jetman::tools::sound::sound_data(&rom))
        .ok();
    let validation = solar_jetman::tools::load_rom(&repo, None)
        .and_then(|(_, rom)| solar_jetman::rom::extract::validation_input(&rom))
        .ok();
    let mut checked = 0;
    let Ok(entries) = fs::read_dir(repo.join("vendor/captures/replay")) else {
        eprintln!("skipped: no captures (run `sj-tools replay-compare`)");
        return;
    };
    for log in entries.flatten().map(|entry| entry.path()) {
        let Some(name) = log
            .extension()
            .is_some_and(|ext| ext == "log")
            .then(|| log.file_stem().unwrap().to_string_lossy().into_owned())
        else {
            continue;
        };
        if name.starts_with("demo-") {
            continue;
        }
        let replay_path = repo.join(format!("tools/mesen/replays/{name}.replay"));
        let Ok(replay) = fs::read_to_string(&replay_path) else {
            continue;
        };
        let replay = parse_replay(&replay).unwrap();
        let capture = parse_capture(&fs::read_to_string(&log).unwrap()).unwrap();
        let report = compare_with_sound(
            world,
            &replay,
            &capture,
            false,
            sound.as_ref(),
            validation.as_ref(),
            None,
        );
        assert!(report.compared > 0, "{name}");
        assert_eq!(
            report.sound_exact, report.sound_compared,
            "{name}: sound requests first differ {:?}",
            report.sound_first
        );
        assert!(
            report.init.is_empty(),
            "{name}: start differs {:?}",
            report.init
        );
        // Quirk Q45 (`docs/deviations.md`): on level 11 the terrain test's list runs over RAM $05CE,
        // the F gauge's display copy, which the remake does not reproduce.
        let known_q45 = name == "level11"
            && report
                .fields
                .iter()
                .all(|(field, result)| result.first.is_none() || *field == "fs");
        if !known_q45 {
            assert_eq!(
                report.all_exact, report.compared,
                "{name}: {:?}, objects {:?}",
                report.first_divergence, report.objects
            );
        }
        checked += 1;
    }
    eprintln!("{checked} replays compared");
}

/// The attract demos (`sj-tools replay-compare --demos` captures them): in lockstep, with the pod
/// and the objects taken from the original after every step, every field of the pod, the score
/// and the objects (special weapons included) is right in every step of all three demos, as are
/// the autopilot's input bytes, its waypoint and its shield timer, and each demo ends where the
/// original's does.
#[test]
fn the_autopilot_reproduces_the_originals_demos() {
    let repo = solar_jetman::tools::repo_root();
    let Some(world) = solar_jetman::tools::test_world() else {
        return;
    };
    let mut checked = 0;
    for n in 1..=3 {
        let log = repo.join(format!("vendor/captures/replay/demo-{n}.log"));
        let Ok(text) = fs::read_to_string(&log) else {
            eprintln!("skipped demo {n}: no capture (run `sj-tools replay-compare --demos`)");
            continue;
        };
        let replay = parse_replay(
            &fs::read_to_string(repo.join(format!("tools/mesen/demos/demo-{n}.replay"))).unwrap(),
        )
        .unwrap();
        let capture = parse_capture(&text).unwrap();
        let report = compare(world, &replay, &capture, true);
        assert!(
            report.compared > 1000,
            "demo {n}: {} steps",
            report.compared
        );
        for (name, field) in &report.fields {
            assert_eq!(
                field.matched, field.total,
                "demo {n}: {name} first differs {:?}",
                field.first
            );
        }
        assert_eq!(
            report.all_exact, report.compared,
            "demo {n}: objects {:?}",
            report.objects
        );
        let left = report.original_left.expect("the original leaves the demo");
        assert_eq!(report.our_transition, Some((left, 0)), "demo {n} ends");
        checked += 1;
    }
    eprintln!("{checked} demos compared");
}
