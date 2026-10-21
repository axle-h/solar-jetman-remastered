//! Reading an asset pack back into the graphics the game draws with: every tile sheet as its
//! decoded tile stream (cut to the stream's exact length, graphics.md 1 "Writes run on"), the
//! palette presets and the per-level colours; the songs and effects as the sound driver's
//! data; and the level data of `game/`.

use crate::game::gameplay::objdata::OBJECTS_FILE;
use crate::game::gameplay::sprites::{SPRITES_FILE, SpriteFile};
use crate::game::graphics::{Graphics, PalettePreset, PictureWrite};
use crate::game::level::{BLOCKS_FILE, COLLISION_FILE, LEVELS_FILE, MAPS_DIR, World};
use crate::game::sound::data::{Effects, SONG_COUNT, SOUND_FILE, Song, SoundData, SoundTables};
use crate::rom::pack::{
    AssetKind, FlightTables, GAME_DIR, GeneratedTileTables, LevelPalettes, MANIFEST_PATH, Manifest,
    PalettePresets, Picture, ScreenPalettes, from_ron,
};
use crate::rom::sheet::{SheetError, decode_sheet};
use std::collections::BTreeMap;
use std::fmt;

const GENERATED_TILES_PATH: &str = "tables/generated-tiles.ron";
const PRESETS_PATH: &str = "palettes/presets.ron";
const SCREEN_PALETTES_PATH: &str = "palettes/screens.ron";
const LEVEL_PALETTES_PATH: &str = "palettes/levels.ron";

/// Why a pack cannot be read.
#[derive(Debug)]
pub enum AssetError {
    Missing(String),
    Text(String),
    Manifest(toml::de::Error),
    Ron {
        path: String,
        error: Box<ron::error::SpannedError>,
    },
    Sheet {
        path: String,
        error: SheetError,
    },
    /// A tile sheet entry without the counts needed to read it.
    Incomplete(String),
    /// The songs or effects do not fit together.
    Sound(String),
    /// A level data file does not read, or the files do not fit together.
    Game(String),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::Missing(path) => write!(f, "{path} is missing from the asset pack"),
            AssetError::Text(path) => write!(f, "{path} is not UTF-8 text"),
            AssetError::Manifest(error) => write!(f, "{MANIFEST_PATH}: {error}"),
            AssetError::Ron { path, error } => write!(f, "{path}: {error}"),
            AssetError::Sheet { path, error } => write!(f, "{path}: {error}"),
            AssetError::Sound(message) => write!(f, "{message}"),
            AssetError::Game(message) => write!(f, "{GAME_DIR}/{message}"),
            AssetError::Incomplete(path) => {
                write!(
                    f,
                    "{MANIFEST_PATH}: the entry of {path} lacks its tile count or source"
                )
            }
        }
    }
}

impl std::error::Error for AssetError {}

fn text<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> Result<&'a str, AssetError> {
    let bytes = files
        .get(path)
        .ok_or_else(|| AssetError::Missing(path.to_owned()))?;
    std::str::from_utf8(bytes).map_err(|_| AssetError::Text(path.to_owned()))
}

fn ron_file<T: for<'de> serde::Deserialize<'de>>(
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<T, AssetError> {
    from_ron(text(files, path)?).map_err(|error| AssetError::Ron {
        path: path.to_owned(),
        error: Box::new(error),
    })
}

/// The game's graphics from the files of a pack (path → bytes, as [`crate::rom::AssetPack`] holds
/// them or as read from the player's folder).
pub fn graphics(files: &BTreeMap<String, Vec<u8>>) -> Result<Graphics, AssetError> {
    let manifest: Manifest =
        toml::from_str(text(files, MANIFEST_PATH)?).map_err(AssetError::Manifest)?;
    let mut tile_streams = BTreeMap::new();
    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Tiles)
    {
        let (Some(tiles), Some(source)) = (asset.tiles, asset.sources.first()) else {
            return Err(AssetError::Incomplete(asset.path.clone()));
        };
        let png = files
            .get(&asset.path)
            .ok_or_else(|| AssetError::Missing(asset.path.clone()))?;
        let mut bytes = decode_sheet(png, tiles as usize).map_err(|error| AssetError::Sheet {
            path: asset.path.clone(),
            error,
        })?;
        bytes.truncate(source.decoded_bytes as usize);
        tile_streams.insert(asset.path.clone(), bytes);
    }
    let presets: PalettePresets = ron_file(files, PRESETS_PATH)?;
    let levels: LevelPalettes = ron_file(files, LEVEL_PALETTES_PATH)?;
    // Packs from before the gauge pieces were imported lack the table: the gauges then show
    // no partial cell.
    let generated: GeneratedTileTables = if files.contains_key(GENERATED_TILES_PATH) {
        ron_file(files, GENERATED_TILES_PATH)?
    } else {
        GeneratedTileTables::default()
    };
    let gauge_rows = generated.gauge_rows.0.clone();
    let mut pictures = BTreeMap::new();
    let mut based_pictures = BTreeMap::new();
    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Picture)
    {
        let picture: Picture = ron_file(files, &asset.path)?;
        let (based, plain): (Vec<_>, Vec<_>) = picture
            .writes
            .into_iter()
            .partition(|write| write.add_tile_base);
        let writes = |list: Vec<crate::rom::pack::NametableWrite>| -> Vec<PictureWrite> {
            list.into_iter()
                .map(|write| PictureWrite {
                    address: write.address,
                    bytes: write.bytes.0,
                })
                .collect()
        };
        if !based.is_empty() {
            based_pictures.insert(asset.path.clone(), writes(based));
        }
        pictures.insert(asset.path.clone(), writes(plain));
    }
    // Packs from before the title's colours were imported lack them.
    let screens: ScreenPalettes = if files.contains_key(SCREEN_PALETTES_PATH) {
        ron_file(files, SCREEN_PALETTES_PATH)?
    } else {
        ScreenPalettes::default()
    };
    // Packs from before the flight was imported lack its tables.
    let flight: FlightTables = if files.contains_key(crate::rom::catalogue::flight::TABLES_PATH) {
        ron_file(files, crate::rom::catalogue::flight::TABLES_PATH)?
    } else {
        FlightTables::default()
    };
    // Packs from before the attract frames were read lack them.
    let attract: Option<crate::rom::pack::PictureFrames> =
        if files.contains_key(crate::rom::catalogue::attract_frames::PATH) {
            Some(ron_file(
                files,
                crate::rom::catalogue::attract_frames::PATH,
            )?)
        } else {
            None
        };
    Ok(Graphics {
        attract_nametable: attract
            .as_ref()
            .map(|frames| frames.nametable.0.clone())
            .unwrap_or_default(),
        attract_attributes: attract
            .map(|frames| frames.attributes.0)
            .unwrap_or_default(),
        pictures,
        based_pictures,
        frames_screen: screens.frames_screen.iter().map(|c| c.0.clone()).collect(),
        flight_window: flight.window_colours.0,
        flight_sine: flight.ring_sine.0,
        title_fade: screens.title_fade.into_iter().map(|c| c.0).collect(),
        title_fade_end: screens.title_fade_end.0,
        backdrop: crate::game::graphics::Backdrop {
            sprite_colours: screens.backdrop_sprites.0.clone(),
            variants: screens
                .backdrop_variants
                .iter()
                .map(|c| c.0.clone())
                .collect(),
            bar_widths: generated.backdrop_bar_widths.0,
            bar_columns: generated.backdrop_bar_columns.0,
            phase_tiles: generated.backdrop_phase_tiles.0,
            spawn_x: generated.backdrop_spawn_x.0,
            spawn_types: generated.backdrop_spawn_types.0,
        },
        analysis_pairs: screens.analysis_pairs.iter().map(|c| c.0.clone()).collect(),
        title_white: screens.title_flash_white.0.first().copied().unwrap_or(0x30),
        gauge_rows,
        tile_streams,
        presets: presets
            .presets
            .into_iter()
            .map(|preset| PalettePreset {
                first_entry: preset.first_entry,
                colours: preset.colours.0,
            })
            .collect(),
        approach_globe: levels
            .levels
            .iter()
            .map(|level| {
                level
                    .approach_globe
                    .as_ref()
                    .map(|colours| colours.0.clone())
            })
            .collect(),
        surface_colours: levels
            .levels
            .iter()
            .map(|level| level.surface.as_ref().map(|colours| colours.0.clone()))
            .collect(),
        level_colours: levels
            .levels
            .into_iter()
            .map(|level| level.gameplay.0)
            .collect(),
    })
}

/// The level data of a pack's `game/` folder.
pub fn world(files: &BTreeMap<String, Vec<u8>>) -> Result<World, AssetError> {
    let game = |name: &str| text(files, &format!("{GAME_DIR}/{name}"));
    let prefix = format!("{GAME_DIR}/{MAPS_DIR}/");
    let mut maps = Vec::new();
    for path in files.keys() {
        if let Some(name) = path
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".ron"))
        {
            maps.push((name, text(files, path)?));
        }
    }
    World::parse(
        game(LEVELS_FILE)?,
        game(BLOCKS_FILE)?,
        game(COLLISION_FILE)?,
        game(OBJECTS_FILE)?,
        maps,
    )
    .map_err(|error| AssetError::Game(error.to_string()))
}

/// The sprite layouts of a pack (`game/sprites.ron`).
pub fn sprites(files: &BTreeMap<String, Vec<u8>>) -> Result<SpriteFile, AssetError> {
    let text = text(files, &format!("{GAME_DIR}/{SPRITES_FILE}"))?;
    SpriteFile::parse(text)
        .map_err(|message| AssetError::Game(format!("{SPRITES_FILE}: {message}")))
}

/// The sound driver's note periods of a pack (`game/sound.ron`).
pub fn note_periods(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u16>, AssetError> {
    let text = text(files, &format!("{GAME_DIR}/{SOUND_FILE}"))?;
    SoundTables::parse(text)
        .map(|tables| tables.note_periods)
        .map_err(|error| AssetError::Sound(error.to_string()))
}

/// The songs and effects of a pack, with the note periods ([`note_periods`]).
pub fn sound_data(
    files: &BTreeMap<String, Vec<u8>>,
    note_periods: Vec<u16>,
) -> Result<SoundData, AssetError> {
    let songs = (0..SONG_COUNT)
        .map(|number| ron_file::<Song>(files, &crate::rom::import::song_path(number)))
        .collect::<Result<Vec<_>, _>>()?;
    let effects: Effects = ron_file(files, crate::rom::import::EFFECTS_PATH)?;
    SoundData::new(songs, effects.effects, note_periods)
        .map_err(|error| AssetError::Sound(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::import::build;
    use crate::rom::import::tests::synthetic_rom;

    #[test]
    fn synthetic_pack_reads_back() {
        let rom = synthetic_rom();
        let pack = build(&rom, false).unwrap();
        let graphics = graphics(&pack.files).unwrap();
        // A raw set: exactly its ROM bytes.
        assert_eq!(
            graphics.tile_streams["tiles/hud/digits.png"],
            rom.prg()[0x08100..0x08220]
        );
        // A run-length set keeps its exact decoded length, partial tile included.
        let common = &graphics.tile_streams["tiles/terrain/common.png"];
        let stream = &rom.prg()[0x17159..0x17159 + 0x24C];
        assert_eq!(*common, crate::rom::decode::run_length(stream, 0));
        assert_eq!(graphics.presets.len(), 28);
        assert_eq!(graphics.level_colours.len(), 42);
        assert_eq!(graphics.level_colours[5], rom.prg()[0x115D4..0x115E0]);
    }

    #[test]
    fn synthetic_sound_reads_back() {
        let pack = build(&synthetic_rom(), false).unwrap();
        let periods = vec![0; crate::game::sound::data::NOTE_PERIODS];
        let sound = sound_data(&pack.files, periods.clone()).unwrap();
        assert_eq!(sound.songs.len(), SONG_COUNT);
        assert!(sound.effect(0x1A).unwrap().in_ram);
        assert!(sound.effect(0x02).is_some());
        assert!(sound_data(&pack.files, vec![0; 3]).is_err());
        let mut files = pack.files.clone();
        files.remove("sound/song-07.ron");
        assert!(matches!(
            sound_data(&files, periods),
            Err(AssetError::Missing(_))
        ));
    }

    #[test]
    fn missing_files_are_reported() {
        let mut pack = build(&synthetic_rom(), false).unwrap();
        pack.files.remove("palettes/presets.ron");
        assert!(matches!(graphics(&pack.files), Err(AssetError::Missing(_))));
        pack.files.remove(MANIFEST_PATH);
        assert!(matches!(graphics(&pack.files), Err(AssetError::Missing(_))));
    }
}
