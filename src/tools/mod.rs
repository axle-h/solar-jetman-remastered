//! Development tools: the clean-room check, the asset import, a headless renderer, and the
//! sound tools (rendering to WAV, comparing with the original).
//! The binary `sj-tools` runs them; the library lets tests use them.

pub mod replay;
pub mod scaled;
pub mod scan;
pub mod screen;
pub mod sound;

#[cfg(test)]
mod pilot_tests;

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root, where the crate is.
pub fn repo_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.canonicalize().unwrap_or(root)
}

/// The `.nes` files of `vendor/rom/` and `vendor/`, in name order.
fn vendor_roms(repo: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = ["vendor/rom", "vendor"]
        .iter()
        .filter_map(|dir| fs::read_dir(repo.join(dir)).ok())
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("nes"))
        })
        .collect();
    found.sort();
    found
}

/// The release of the ROM file at `path`, if it is one we accept.
fn release_of(path: &Path) -> Option<crate::rom::Release> {
    let file = fs::read(path).ok()?;
    crate::rom::Rom::from_nes_file(&file)
        .ok()
        .map(|rom| rom.release())
}

/// The ROM to use when none is given: `$SJ_ROM`, else the USA release among `vendor/rom/*.nes`
/// and `vendor/*.nes` (the tools compare with the original running the USA release), else the
/// first of those files.
pub fn default_rom(repo: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SJ_ROM") {
        return Some(PathBuf::from(path));
    }
    vendor_rom(repo, crate::rom::Release::Usa).or_else(|| vendor_roms(repo).into_iter().next())
}

/// The first of `vendor/rom/*.nes` and `vendor/*.nes` that is `release`.
pub fn vendor_rom(repo: &Path, release: crate::rom::Release) -> Option<PathBuf> {
    vendor_roms(repo)
        .into_iter()
        .find(|path| release_of(path) == Some(release))
}

/// Reads and checks a ROM: `path`, or the default one.
pub fn load_rom(repo: &Path, path: Option<PathBuf>) -> Result<(PathBuf, crate::rom::Rom), String> {
    let path = path
        .or_else(|| default_rom(repo))
        .ok_or("no ROM given, no $SJ_ROM and no vendor/rom/*.nes or vendor/*.nes")?;
    let file = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let rom = crate::rom::Rom::from_nes_file(&file)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok((path, rom))
}

/// What the game runs on, imported from a checked ROM as the app imports it: level data,
/// graphics and sprite layouts.
pub fn assets(rom: &crate::rom::Rom) -> Result<crate::game::scene::Assets, String> {
    let pack = crate::rom::import(rom).map_err(|error| error.to_string())?;
    let files = &pack.files;
    Ok(crate::game::scene::Assets {
        world: crate::rom::world(files).map_err(|error| error.to_string())?,
        graphics: crate::rom::graphics(files).map_err(|error| error.to_string())?,
        layouts: crate::rom::sprites(files).map_err(|error| error.to_string())?,
    })
}

/// The level data of the default ROM ([`default_rom`]), imported as the app imports it.
pub fn default_world(repo: &Path) -> Result<crate::game::level::World, String> {
    let (_, rom) = load_rom(repo, None)?;
    let pack = crate::rom::import(&rom).map_err(|error| error.to_string())?;
    crate::rom::world(&pack.files).map_err(|error| error.to_string())
}

/// For tests: the default ROM ([`default_rom`]), read and checked once, or `None` (after saying
/// so) when there is no ROM.
pub fn test_rom() -> Option<&'static crate::rom::Rom> {
    static ROM: std::sync::OnceLock<Option<crate::rom::Rom>> = std::sync::OnceLock::new();
    ROM.get_or_init(|| {
        let repo = repo_root();
        if default_rom(&repo).is_none() {
            eprintln!("skipped: no ROM (set SJ_ROM or put the .nes file in vendor/)");
            return None;
        }
        Some(
            load_rom(&repo, None)
                .unwrap_or_else(|error| panic!("{error}"))
                .1,
        )
    })
    .as_ref()
}

/// For tests: the level data of the default ROM, imported once, or `None` (after saying so)
/// when there is no ROM.
pub fn test_world() -> Option<&'static crate::game::level::World> {
    static WORLD: std::sync::OnceLock<Option<crate::game::level::World>> =
        std::sync::OnceLock::new();
    WORLD
        .get_or_init(|| {
            let rom = test_rom()?;
            let pack = crate::rom::import(rom).unwrap_or_else(|error| panic!("{error}"));
            Some(crate::rom::world(&pack.files).unwrap_or_else(|error| panic!("{error}")))
        })
        .as_ref()
}
