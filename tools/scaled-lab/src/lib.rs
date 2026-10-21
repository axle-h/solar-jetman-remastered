//! Shared helpers for the variable-rate experiments: the original engine, driven headless.
use solar_jetman::game::gameplay::{Gameplay, Seed, input::Buttons, rng::Rng};
use solar_jetman::game::level::World;

/// The level data, imported from the ROM in `vendor/rom/` or `$SJ_ROM`.
pub fn world() -> World {
    let repo = solar_jetman::tools::repo_root();
    solar_jetman::tools::default_world(&repo).unwrap_or_else(|error| panic!("{error}"))
}

/// A fresh level as the original starts it (level 0's generator seed).
pub fn game(world: &World, level: usize) -> Gameplay {
    let seed = Seed { rng: Rng { out: 0x78, high: 0xC9, low: 0x03 }, ..Default::default() };
    Gameplay::new(world, level, seed)
}

/// A recorded replay's per-step buttons and equipment (tools/mesen/replays/NAME.replay).
pub fn replay(name: &str) -> (u8, Vec<Buttons>) {
    let path = format!("{}/../../tools/mesen/replays/{name}.replay", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("replay");
    let r = solar_jetman::tools::replay::parse_replay(&text).expect("parse");
    (r.equipment, r.steps)
}
pub mod contact;

pub mod motion;
pub mod camera;
pub mod tow;
pub mod enemies;
pub mod misc;
