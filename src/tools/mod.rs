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

/// The ROM to use when none is given: `$SJ_ROM`, else the first `vendor/rom/*.nes`.
pub fn default_rom(repo: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SJ_ROM") {
        return Some(PathBuf::from(path));
    }
    let mut found: Vec<PathBuf> = fs::read_dir(repo.join("vendor/rom"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("nes"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Reads and checks a ROM: `path`, or the default one.
pub fn load_rom(repo: &Path, path: Option<PathBuf>) -> Result<(PathBuf, crate::rom::Rom), String> {
    let path = path
        .or_else(|| default_rom(repo))
        .ok_or("no ROM given, no $SJ_ROM and no vendor/rom/*.nes")?;
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
            eprintln!("skipped: no ROM (set SJ_ROM or put the .nes file in vendor/rom/)");
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
