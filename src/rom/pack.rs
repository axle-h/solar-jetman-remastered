//! The files of an imported asset pack, as written into the player's `data/` folder:
//! `manifest.toml`, tile sheets (`tiles/**.png`, see [`crate::rom::sheet`]), RON files for
//! pictures, palettes and small tables, and the level data in `game/` (formats in
//! [`crate::game::level`]). The types here both write and read them.

use crate::rom::hex::HexRows;
use serde::{Deserialize, Serialize};

/// Bump when the import's output changes, so that older packs are imported again.
pub const IMPORTER_VERSION: u32 = 8;

/// Path of the manifest inside a pack.
pub const MANIFEST_PATH: &str = "manifest.toml";

/// Folder of the level data inside a pack ([`crate::rom::extract`]).
pub const GAME_DIR: &str = "game";

/// `manifest.toml`: what was imported, from which ROM, by which importer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub importer_version: u32,
    /// SHA-1 of the PRG ROM the pack was imported from.
    pub rom_sha1: String,
    #[serde(rename = "asset", default)]
    pub assets: Vec<AssetEntry>,
}

/// What kind of file an asset is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    /// A tile sheet (PNG).
    Tiles,
    /// A nametable picture ([`Picture`]).
    Picture,
    /// Animation frames of a picture ([`PictureFrames`]).
    PictureFrames,
    /// Palette presets ([`PalettePresets`]).
    PalettePresets,
    /// Per-level colours ([`LevelPalettes`]).
    LevelPalettes,
    /// Palettes of single screens ([`ScreenPalettes`]).
    ScreenPalettes,
    /// Tables the game builds tiles from ([`GeneratedTileTables`]).
    GeneratedTiles,
    /// The interplanetary flight's tables ([`FlightTables`]).
    FlightTables,
    /// A song ([`crate::game::sound::Song`], format in [`crate::rom::sound`]).
    Song,
    /// The sound effects ([`crate::game::sound::data::Effects`]).
    Effects,
    /// A level data file of `game/` ([`crate::game::level::World`], the sprite layouts or the
    /// note periods).
    GameData,
}

/// One file of the pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetEntry {
    pub path: String,
    pub kind: AssetKind,
    pub purpose: String,
    /// How sure the spec is of this entry ("verified", "hypothesis", ...).
    pub certainty: String,
    /// Tile sheets: number of tiles, counting a partial last tile (its exact length is the
    /// source's `decoded_bytes`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiles: Option<u32>,
    /// Tile sheets of equal blocks: tiles per block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_tiles: Option<u32>,
    /// PPU addresses the original loads the tiles to, as `$hhhh`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub destinations: Vec<String>,
    #[serde(rename = "source", default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<SourceEntry>,
}

/// Where in the ROM an asset came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEntry {
    /// PRG ROM offset, as `0xhhhhh`.
    pub rom_offset: String,
    /// "raw", "run-length", "font", or for sound "music" or "effects" (the data region the
    /// events were decoded from; `decoded_bytes` is then the number of events or effects).
    pub format: String,
    /// Bytes read from the ROM.
    pub stored_bytes: u32,
    /// Bytes produced. For tiles this may end a few bytes into a tile; the sheet holds that tile
    /// padded with zero bytes, and the original writes only these bytes (graphics.md 1).
    pub decoded_bytes: u32,
    /// Run-length bias added to every byte, when not 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bias: Option<u8>,
}

/// `pictures/*.ron`: bytes written to the nametable, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picture {
    pub writes: Vec<NametableWrite>,
}

/// Consecutive bytes written to the nametable from a PPU address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NametableWrite {
    /// PPU address of the first byte, $2000-$23FF ($23C0 and up are attributes).
    pub address: u16,
    /// The original adds a tile base chosen at run time to every byte before writing it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub add_tile_base: bool,
    /// The bytes, 32 per row.
    pub bytes: HexRows<32>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// `pictures/attract-jetman-frames.ron`: animation frames of a picture, in ROM order. Where
/// in the nametable the rows go is not specified yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureFrames {
    pub frames: u32,
    /// Nametable bytes, rows of 16 (11 rows per frame).
    pub nametable: HexRows<16>,
    /// Attribute bytes, 12 per frame.
    pub attributes: HexRows<12>,
}

/// `palettes/presets.ron`: the palette presets, by preset number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PalettePresets {
    pub presets: Vec<PalettePreset>,
}

/// Colours a preset writes into the 32 palette entries, from `first_entry` on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PalettePreset {
    pub first_entry: u8,
    /// NES colour numbers.
    pub colours: HexRows<16>,
}

/// `palettes/levels.ron`: colours chosen per level, by level number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelPalettes {
    pub levels: Vec<LevelPalette>,
}

/// One level's colours (graphics.md 7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelPalette {
    /// Bytes 12-23 of the level header: background palettes 2-3 then sprite palettes 2-3,
    /// colours 1-3 of each.
    pub gameplay: HexRows<16>,
    /// Planets only: the 3 colours of the globe on the approach screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approach_globe: Option<HexRows<16>>,
    /// Planets only: the 12 colours for entries 0-11 on the surface picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<HexRows<16>>,
}

/// `palettes/screens.ron`: fixed palettes of single screens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenPalettes {
    /// Screen of graphics.md 5.10: two sets of 15 colours for entries 1-15.
    pub frames_screen: Vec<HexRows<16>>,
    /// Bank 7 backdrop: 16 colours for entries 16-31.
    pub backdrop_sprites: HexRows<16>,
    /// Bank 7 backdrop: 3 sets of 4 colours for entries 12-15.
    pub backdrop_variants: Vec<HexRows<16>>,
    /// Title (graphics.md 7.4): the six 7-step columns of the logo fade-in.
    #[serde(default)]
    pub title_fade: Vec<HexRows<16>>,
    /// Title: the 3 colours entries 9-11 take after the last fade step.
    #[serde(default)]
    pub title_fade_end: HexRows<16>,
    /// Title: the colour of the sun's flicker (one byte).
    #[serde(default)]
    pub title_flash_white: HexRows<16>,
    /// Analysis screen (graphics.md 7.4): the two colours for entries 9 and 10, by list.
    #[serde(default)]
    pub analysis_pairs: Vec<HexRows<16>>,
}

/// `tables/generated-tiles.ron`: byte tables the original builds tiles from at run time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedTileTables {
    /// The 8 values gauge tiles are filled with.
    pub gauge_rows: HexRows<8>,
    /// One bit per x position, for the twinkling star.
    pub star_column_masks: HexRows<8>,
    /// The backdrop interlude's tables (scenes.md 11.1), absent from older packs.
    #[serde(default)]
    pub backdrop_bar_widths: HexRows<16>,
    #[serde(default)]
    pub backdrop_bar_columns: HexRows<16>,
    #[serde(default)]
    pub backdrop_phase_tiles: HexRows<16>,
    #[serde(default)]
    pub backdrop_spawn_x: HexRows<16>,
    #[serde(default)]
    pub backdrop_spawn_types: HexRows<16>,
}

/// `tables/flight.ron`: numbers the interplanetary flight reads (scenes.md 5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlightTables {
    /// The lit window's colour by step (12 colour numbers).
    pub window_colours: HexRows<16>,
    /// The half-turn sine table of the ring of big stars (128 values).
    pub ring_sine: HexRows<16>,
}

/// Writes a pack file as pretty RON.
pub fn to_ron<T: Serialize>(value: &T) -> Result<String, ron::Error> {
    let config = ron::ser::PrettyConfig::new()
        .indentor("  ".to_owned())
        .struct_names(true);
    ron::ser::to_string_pretty(value, config)
}

/// Writes a pack file as pretty RON, with everything deeper than `depth` on one line.
pub fn to_ron_with_depth<T: Serialize>(value: &T, depth: usize) -> Result<String, ron::Error> {
    let config = ron::ser::PrettyConfig::new()
        .indentor("  ".to_owned())
        .struct_names(true)
        .depth_limit(depth);
    ron::ser::to_string_pretty(value, config)
}

/// Reads a pack file written by [`to_ron`].
pub fn from_ron<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, ron::error::SpannedError> {
    ron::from_str(text)
}
