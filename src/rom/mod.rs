//! The player's ROM: checking it is the USA release, and importing its graphics, music, sound
//! effects and level data into an asset pack (`docs/spec/rom-map.md`, `docs/spec/graphics.md`,
//! `docs/spec/sound.md`, `docs/spec/levels.md`). No file I/O here: callers pass the file's bytes in and write the
//! pack's files out, or read them back into the graphics and sound the game uses.
#![forbid(unsafe_code)]

pub mod assets;
pub mod cartridge;
pub mod catalogue;
pub mod decode;
pub mod extract;
pub mod hex;
pub mod import;
pub mod pack;
pub mod sheet;
pub mod sound;

pub use assets::{AssetError, graphics, note_periods, sound_data, sprites, world};
pub use cartridge::{NesHeader, PRG_SHA1, Rom, RomError};
pub use import::{AssetPack, ImportError, import};
pub use pack::{GAME_DIR, IMPORTER_VERSION, MANIFEST_PATH, Manifest};
