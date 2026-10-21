//! The import: turns a checked ROM into an in-memory asset pack (path → file bytes), graphics,
//! sound and level data. Writing it to disk is the caller's business.

use crate::rom::cartridge::{Rom, bank_offset, sha1_hex};
use crate::rom::catalogue::{
    self, Blob, Certainty, Format, PICTURES, TILE_SETS, attract_frames, generated, palettes,
};
use crate::rom::decode::TILE_BYTES;
use crate::rom::hex::HexRows;
use crate::rom::pack::{
    AssetEntry, AssetKind, FlightTables, GAME_DIR, GeneratedTileTables, IMPORTER_VERSION,
    LevelPalette, LevelPalettes, MANIFEST_PATH, Manifest, NametableWrite, PalettePreset,
    PalettePresets, Picture, PictureFrames, ScreenPalettes, SourceEntry, to_ron, to_ron_with_depth,
};
use crate::rom::sheet::{SheetError, encode_sheet};
use std::collections::BTreeMap;
use std::fmt;

/// Why an import failed. With a ROM that passed the check none of these should happen; they
/// point at a catalogue entry that disagrees with the ROM.
#[derive(Debug)]
pub enum ImportError {
    /// A blob or table lies past the end of the ROM.
    OutOfRange {
        offset: usize,
        len: usize,
    },
    /// A blob decoded to another size than the spec states.
    UnexpectedSize {
        path: String,
        offset: usize,
        expected: usize,
        actual: usize,
    },
    /// A palette preset or level header pointer leads outside its region.
    BadPointer {
        what: String,
        address: u16,
    },
    Sheet {
        path: String,
        error: SheetError,
    },
    Ron {
        path: String,
        error: ron::Error,
    },
    Toml(toml::ser::Error),
    /// The sound data does not decode as the spec describes.
    Sound(String),
    /// A printer message does not have the shape the spec describes.
    Message(String),
    /// The level data does not read as the spec describes.
    Game(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::OutOfRange { offset, len } => write!(
                f,
                "{len} bytes at ROM 0x{offset:05X} run past the end of the ROM"
            ),
            ImportError::UnexpectedSize {
                path,
                offset,
                expected,
                actual,
            } => write!(
                f,
                "{path}: blob at ROM 0x{offset:05X} decoded to {actual:#X} bytes, expected {expected:#X}"
            ),
            ImportError::BadPointer { what, address } => {
                write!(f, "{what} points outside its region (${address:04X})")
            }
            ImportError::Sheet { path, error } => write!(f, "{path}: {error}"),
            ImportError::Ron { path, error } => write!(f, "{path}: {error}"),
            ImportError::Toml(error) => write!(f, "{MANIFEST_PATH}: {error}"),
            ImportError::Sound(error) => write!(f, "sound: {error}"),
            ImportError::Message(error) => write!(f, "message: {error}"),
            ImportError::Game(error) => write!(f, "level data: {error}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// An imported asset pack: every file by its path relative to the pack's folder, including
/// `manifest.toml`, and the manifest itself.
#[derive(Debug, Clone)]
pub struct AssetPack {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Imports every graphics asset of the catalogue, the music and sound effects, and the level
/// data from a checked ROM.
pub fn import(rom: &Rom) -> Result<AssetPack, ImportError> {
    build(rom, true)
}

/// The import; `real` false is for tests on synthetic ROMs: it skips comparing decoded sizes
/// with the spec, as made-up streams decode to other sizes, and leaves the level data out, as
/// made-up tables do not hold together.
pub(crate) fn build(rom: &Rom, real: bool) -> Result<AssetPack, ImportError> {
    let check_sizes = real;
    let mut pack = Builder {
        rom,
        check_sizes,
        assets: Vec::new(),
        files: BTreeMap::new(),
    };
    for set in TILE_SETS {
        pack.tile_set(set)?;
    }
    for picture in PICTURES {
        pack.picture(picture)?;
    }
    pack.attract_frames()?;
    pack.palette_presets()?;
    pack.level_palettes()?;
    pack.screen_palettes()?;
    pack.generated_tiles()?;
    pack.flight()?;
    pack.sound()?;
    if real {
        pack.game_data()?;
    }

    let manifest = Manifest {
        importer_version: IMPORTER_VERSION,
        rom_sha1: sha1_hex(rom.prg()),
        assets: pack.assets,
    };
    let text = toml::to_string_pretty(&manifest).map_err(ImportError::Toml)?;
    let mut files = pack.files;
    files.insert(MANIFEST_PATH.to_owned(), text.into_bytes());
    Ok(AssetPack { manifest, files })
}

struct Builder<'a> {
    rom: &'a Rom,
    check_sizes: bool,
    assets: Vec<AssetEntry>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Builder<'_> {
    fn bytes(&self, offset: usize, len: usize) -> Result<&[u8], ImportError> {
        self.rom
            .bytes(offset, len)
            .ok_or(ImportError::OutOfRange { offset, len })
    }

    /// Decodes a blob and checks its size against the spec.
    fn decode(&self, path: &str, blob: &Blob) -> Result<(Vec<u8>, SourceEntry), ImportError> {
        let offset = blob.offset as usize;
        let decoded = blob.decode(self.rom).ok_or(ImportError::OutOfRange {
            offset,
            len: blob.stored_bytes(),
        })?;
        if let Some(expected) = blob.decoded
            && self.check_sizes
            && expected as usize != decoded.len()
        {
            return Err(ImportError::UnexpectedSize {
                path: path.to_owned(),
                offset,
                expected: expected as usize,
                actual: decoded.len(),
            });
        }
        let source = SourceEntry {
            rom_offset: format!("0x{offset:05X}"),
            format: blob.format.name().to_owned(),
            stored_bytes: blob.stored_bytes() as u32,
            decoded_bytes: decoded.len() as u32,
            bias: match blob.format {
                Format::RunLength { bias } if bias != 0 => Some(bias),
                _ => None,
            },
        };
        Ok((decoded, source))
    }

    fn add(&mut self, entry: AssetEntry, bytes: Vec<u8>) {
        self.files.insert(entry.path.clone(), bytes);
        self.assets.push(entry);
    }

    fn add_ron<T: serde::Serialize>(
        &mut self,
        entry: AssetEntry,
        value: &T,
    ) -> Result<(), ImportError> {
        let text = to_ron(value).map_err(|error| ImportError::Ron {
            path: entry.path.clone(),
            error,
        })?;
        self.add(entry, text.into_bytes());
        Ok(())
    }

    fn tile_set(&mut self, set: &catalogue::TileSet) -> Result<(), ImportError> {
        let (decoded, source) = self.decode(set.path, &set.blob)?;
        let png = encode_sheet(&decoded).map_err(|error| ImportError::Sheet {
            path: set.path.to_owned(),
            error,
        })?;
        let mut entry = entry(set.path, AssetKind::Tiles, set.purpose, set.certainty);
        entry.tiles = Some(decoded.len().div_ceil(TILE_BYTES) as u32);
        entry.block_tiles = set.block_tiles.map(u32::from);
        entry.destinations = set
            .destinations
            .iter()
            .map(|address| format!("${address:04X}"))
            .collect();
        entry.sources.push(source);
        self.add(entry, png);
        Ok(())
    }

    fn picture(&mut self, picture: &catalogue::Picture) -> Result<(), ImportError> {
        let mut entry = entry(
            picture.path,
            AssetKind::Picture,
            picture.purpose,
            picture.certainty,
        );
        let mut writes = Vec::new();
        for part in picture.parts {
            let (bytes, source) = self.decode(picture.path, &part.blob)?;
            entry.sources.push(source);
            writes.push(NametableWrite {
                address: part.address,
                add_tile_base: part.add_tile_base,
                bytes: HexRows(bytes),
            });
        }
        self.add_ron(entry, &Picture { writes })
    }

    fn attract_frames(&mut self) -> Result<(), ImportError> {
        let path = attract_frames::PATH;
        let (nametable, nametable_source) = self.decode(path, &attract_frames::NAMETABLE)?;
        let (attributes, attribute_source) = self.decode(path, &attract_frames::ATTRIBUTES)?;
        let mut entry = entry(
            path,
            AssetKind::PictureFrames,
            "attract sequence: animation frames of the floating Jetman picture (11 frames of 11 nametable rows of 16 bytes, and 12 attribute bytes per frame)",
            Certainty::Read,
        );
        entry.sources = vec![nametable_source, attribute_source];
        let frames = PictureFrames {
            frames: attract_frames::FRAMES as u32,
            nametable: HexRows(nametable),
            attributes: HexRows(attributes),
        };
        self.add_ron(entry, &frames)
    }

    fn palette_presets(&mut self) -> Result<(), ImportError> {
        use palettes::*;
        let mut presets = Vec::with_capacity(PRESETS);
        for preset in 0..PRESETS {
            let pointer = self.bytes(PRESET_POINTERS + 2 * preset, 2)?;
            let address = u16::from_le_bytes([pointer[0], pointer[1]]);
            let first_entry = self.bytes(PRESET_FIRST_ENTRY + preset, 1)?[0];
            let count = self.bytes(PRESET_COUNT + preset, 1)?[0];
            let start = bank_offset(0, address);
            let end = start + usize::from(count);
            if address < 0x8000
                || start < PRESET_DATA.0
                || end > PRESET_DATA.1
                || usize::from(first_entry) + usize::from(count) > 32
            {
                return Err(ImportError::BadPointer {
                    what: format!("palette preset {preset}"),
                    address,
                });
            }
            presets.push(PalettePreset {
                first_entry,
                colours: HexRows(self.bytes(start, usize::from(count))?.to_vec()),
            });
        }
        let mut entry = entry(
            "palettes/presets.ron",
            AssetKind::PalettePresets,
            "the 28 palette presets of bank 0, by preset number (graphics.md 7.2)",
            Certainty::Verified,
        );
        entry
            .sources
            .push(raw_source(PRESET_DATA.0, PRESET_DATA.1 - PRESET_DATA.0));
        self.add_ron(entry, &PalettePresets { presets })
    }

    fn level_palettes(&mut self) -> Result<(), ImportError> {
        use palettes::*;
        let low = self.bytes(HEADER_POINTERS_LOW, LEVELS)?.to_vec();
        let high = self.bytes(HEADER_POINTERS_HIGH, LEVELS)?.to_vec();
        let mut levels = Vec::with_capacity(LEVELS);
        for level in 0..LEVELS {
            let address = u16::from_le_bytes([low[level], high[level]]);
            let header = bank_offset(2, address);
            if address < 0x8000 || header < HEADERS.0 || header + 27 > HEADERS.1 {
                return Err(ImportError::BadPointer {
                    what: format!("level {level} header"),
                    address,
                });
            }
            let (from, to) = HEADER_COLOURS;
            let gameplay = self.bytes(header + from, to - from)?.to_vec();
            // Level 13 reads past the end of both 13-entry tables, as the original does.
            let planet = level < PLANETS;
            let approach_globe = planet
                .then(|| self.bytes(APPROACH + APPROACH_COLOURS * level, APPROACH_COLOURS))
                .transpose()?;
            let surface = planet
                .then(|| self.bytes(SURFACE + SURFACE_COLOURS * level, SURFACE_COLOURS))
                .transpose()?;
            levels.push(LevelPalette {
                gameplay: HexRows(gameplay),
                approach_globe: approach_globe.map(|bytes| HexRows(bytes.to_vec())),
                surface: surface.map(|bytes| HexRows(bytes.to_vec())),
            });
        }
        let mut entry = entry(
            "palettes/levels.ron",
            AssetKind::LevelPalettes,
            "colours per level, by level number: gameplay colours from the level header; approach globe and surface picture colours for the 14 planets (graphics.md 7.3)",
            Certainty::Verified,
        );
        entry.sources = vec![
            raw_source(HEADERS.0, HEADERS.1 - HEADERS.0),
            raw_source(APPROACH, APPROACH_COLOURS * PLANETS),
            raw_source(SURFACE, SURFACE_COLOURS * PLANETS),
        ];
        self.add_ron(entry, &LevelPalettes { levels })
    }

    fn screen_palettes(&mut self) -> Result<(), ImportError> {
        use palettes::*;
        let row = |offset: usize, len: usize| -> Result<HexRows<16>, ImportError> {
            Ok(HexRows(self.bytes(offset, len)?.to_vec()))
        };
        let frames_screen = FRAMES_SCREEN
            .iter()
            .map(|&offset| row(offset, FRAMES_SCREEN_COLOURS))
            .collect::<Result<_, _>>()?;
        let backdrop_variants = (0..BACKDROP_VARIANT_COUNT)
            .map(|i| {
                row(
                    BACKDROP_VARIANTS + i * BACKDROP_VARIANT_COLOURS,
                    BACKDROP_VARIANT_COLOURS,
                )
            })
            .collect::<Result<_, _>>()?;
        let palettes = ScreenPalettes {
            frames_screen,
            backdrop_sprites: row(BACKDROP, BACKDROP_COLOURS)?,
            backdrop_variants,
            title_fade: (0..TITLE_FADE_COLUMNS)
                .map(|i| row(TITLE_FADE + i * TITLE_FADE_STEPS, TITLE_FADE_STEPS))
                .collect::<Result<_, _>>()?,
            title_fade_end: HexRows(
                TITLE_FADE_END
                    .iter()
                    .map(|&offset| self.bytes(offset, 1).map(|b| b[0]))
                    .collect::<Result<_, _>>()?,
            ),
            title_flash_white: row(TITLE_FLASH_WHITE, 1)?,
            analysis_pairs: (0..ANALYSIS_PAIR_COUNT)
                .map(|i| row(ANALYSIS_PAIRS + 2 * i, 2))
                .collect::<Result<_, _>>()?,
        };
        let mut entry = entry(
            "palettes/screens.ron",
            AssetKind::ScreenPalettes,
            "fixed palettes of the screen of graphics.md 5.10 and of the bank 7 backdrop (graphics.md 7.3)",
            Certainty::Read,
        );
        entry.sources = vec![
            raw_source(FRAMES_SCREEN[0], 2 * FRAMES_SCREEN_COLOURS),
            raw_source(
                BACKDROP,
                BACKDROP_COLOURS + BACKDROP_VARIANT_COUNT * BACKDROP_VARIANT_COLOURS,
            ),
            raw_source(TITLE_FADE, TITLE_FADE_COLUMNS * TITLE_FADE_STEPS),
            raw_source(TITLE_FLASH_WHITE, 1),
            raw_source(ANALYSIS_PAIRS, 2 * ANALYSIS_PAIR_COUNT),
        ];
        self.add_ron(entry, &palettes)
    }

    /// The flight's mothership picture (printer message 49, text.md 2.1) and its two tables.
    fn flight(&mut self) -> Result<(), ImportError> {
        use catalogue::flight::*;
        let message = self.bytes(MESSAGE, MESSAGE_LENGTH)?.to_vec();
        let mut writes: Vec<NametableWrite> = Vec::new();
        let mut bytes = message.iter().copied();
        // Made-up test ROMs have no message: the picture is then empty.
        let picture_mode = bytes.next() == Some(0xFC);
        if !picture_mode && self.check_sizes {
            return Err(ImportError::Message(
                "the flight's message is not in picture mode".to_owned(),
            ));
        }
        while picture_mode && let Some(code) = bytes.next() {
            match code {
                0xFF => break,
                0xFE => {
                    let (Some(column), Some(row)) = (bytes.next(), bytes.next()) else {
                        break;
                    };
                    writes.push(NametableWrite {
                        address: 0x2000 + 32 * u16::from(row) + u16::from(column),
                        add_tile_base: false,
                        bytes: HexRows(Vec::new()),
                    });
                }
                tile => {
                    if let Some(write) = writes.last_mut() {
                        write.bytes.0.push(tile);
                    }
                }
            }
        }
        let mut picture_entry = entry(
            MESSAGE_PATH,
            AssetKind::Picture,
            "interplanetary flight: the mothership picture, printer message 49 (scenes.md 5)",
            Certainty::Verified,
        );
        picture_entry.sources = vec![raw_source(MESSAGE, MESSAGE_LENGTH)];
        self.add_ron(picture_entry, &Picture { writes })?;
        let tables = FlightTables {
            window_colours: HexRows(self.bytes(WINDOW, WINDOW_LENGTH)?.to_vec()),
            ring_sine: HexRows(self.bytes(SINE, SINE_LENGTH)?.to_vec()),
        };
        let mut entry = entry(
            TABLES_PATH,
            AssetKind::FlightTables,
            "the lit window's colours and the ring's sine table (scenes.md 5.1, 5.4)",
            Certainty::Verified,
        );
        entry.sources = vec![
            raw_source(WINDOW, WINDOW_LENGTH),
            raw_source(SINE, SINE_LENGTH),
        ];
        self.add_ron(entry, &tables)
    }

    fn generated_tiles(&mut self) -> Result<(), ImportError> {
        use generated::*;
        let tables = GeneratedTileTables {
            gauge_rows: HexRows(self.bytes(GAUGE_ROWS, LENGTH)?.to_vec()),
            star_column_masks: HexRows(self.bytes(STAR_MASKS, LENGTH)?.to_vec()),
            backdrop_bar_widths: HexRows(self.bytes(BAR_WIDTHS, BARS)?.to_vec()),
            backdrop_bar_columns: HexRows(self.bytes(BAR_COLUMNS, BARS)?.to_vec()),
            backdrop_phase_tiles: HexRows(self.bytes(PHASE_TILES, PHASE_TILES_LENGTH)?.to_vec()),
            backdrop_spawn_x: HexRows(self.bytes(SPAWN_X, SPAWN_ENTRIES)?.to_vec()),
            backdrop_spawn_types: HexRows(self.bytes(SPAWN_TYPES, SPAWN_ENTRIES)?.to_vec()),
        };
        let mut entry = entry(
            PATH,
            AssetKind::GeneratedTiles,
            "byte tables the game builds the gauge and twinkling-star tiles from (graphics.md 3)",
            Certainty::Read,
        );
        entry.sources = vec![
            raw_source(GAUGE_ROWS, LENGTH),
            raw_source(STAR_MASKS, LENGTH),
            raw_source(BAR_WIDTHS, BARS),
            raw_source(BAR_COLUMNS, BARS),
            raw_source(PHASE_TILES, PHASE_TILES_LENGTH),
            raw_source(SPAWN_X, SPAWN_ENTRIES),
            raw_source(SPAWN_TYPES, SPAWN_ENTRIES),
        ];
        self.add_ron(entry, &tables)
    }
}

impl Builder<'_> {
    /// The songs and effects (sound.md; format in [`crate::rom::sound`]).
    fn sound(&mut self) -> Result<(), ImportError> {
        let sound = crate::rom::sound::decode(self.rom).map_err(ImportError::Sound)?;
        let data = crate::rom::sound::at::DATA;
        for (number, song) in sound.songs.iter().enumerate() {
            let path = song_path(number);
            let mut entry = entry(
                &path,
                AssetKind::Song,
                &format!("song {number}: tempo and four tracks of events (sound.md 3.1, 4.1)"),
                Certainty::Verified,
            );
            let events: usize = song.tracks.iter().map(|track| track.events.len()).sum();
            entry.sources = vec![
                raw_source(crate::rom::sound::at::TEMPOS + number, 1),
                SourceEntry {
                    rom_offset: format!("0x{:05X}", data.0),
                    format: "music".to_owned(),
                    stored_bytes: (data.1 - data.0) as u32,
                    decoded_bytes: events as u32,
                    bias: None,
                },
            ];
            self.add_ron_with(entry, song, SOUND_RON_DEPTH)?;
        }
        let mut entry = entry(
            EFFECTS_PATH,
            AssetKind::Effects,
            "the sound effects $02-$44, $1A from its RAM template (sound.md 3.2, 4.2, 4.3)",
            Certainty::Verified,
        );
        entry.sources = vec![
            SourceEntry {
                rom_offset: format!("0x{:05X}", data.0),
                format: "effects".to_owned(),
                stored_bytes: (data.1 - data.0) as u32,
                decoded_bytes: sound.effects.len() as u32,
                bias: None,
            },
            raw_source(
                crate::rom::sound::at::RAM_EFFECT_TEMPLATE,
                crate::rom::sound::RAM_EFFECT_BYTES,
            ),
        ];
        let effects = crate::game::sound::data::Effects {
            effects: sound.effects,
        };
        self.add_ron_with(entry, &effects, SOUND_RON_DEPTH)
    }

    fn add_ron_with<T: serde::Serialize>(
        &mut self,
        entry: AssetEntry,
        value: &T,
        depth: usize,
    ) -> Result<(), ImportError> {
        let text = to_ron_with_depth(value, depth).map_err(|error| ImportError::Ron {
            path: entry.path.clone(),
            error,
        })?;
        self.add(entry, text.into_bytes());
        Ok(())
    }
}

impl Builder<'_> {
    /// The level data, sprite layouts and note periods (levels.md, objects.md 6, sound.md 3.3).
    fn game_data(&mut self) -> Result<(), ImportError> {
        let files = crate::rom::extract::game_files(self.rom).map_err(ImportError::Game)?;
        for (name, text) in files {
            let path = format!("{GAME_DIR}/{name}");
            let entry = entry(
                &path,
                AssetKind::GameData,
                &game_purpose(&name),
                Certainty::Verified,
            );
            self.add(entry, text.into_bytes());
        }
        Ok(())
    }
}

/// What a file of `game/` holds, by its name there.
fn game_purpose(name: &str) -> String {
    use crate::game::gameplay::objdata::OBJECTS_FILE;
    use crate::game::gameplay::sprites::SPRITES_FILE;
    use crate::game::level::{BLOCKS_FILE, COLLISION_FILE, LEVELS_FILE};
    use crate::game::sound::data::SOUND_FILE;
    match name {
        LEVELS_FILE => "the level headers without their colours (levels.md 3)".to_owned(),
        BLOCKS_FILE => "the block sets and chunks (levels.md 4.3)".to_owned(),
        COLLISION_FILE => "block shape classes and their line segments (levels.md 6)".to_owned(),
        OBJECTS_FILE => "placements, object type and state tables, sizes and the demo's waypoints \
             (objects.md 3, 4.1, 8; game-flow.md 7.1)"
            .to_owned(),
        SPRITES_FILE => {
            "sprite layouts and the tile sets each level loads (objects.md 6; graphics.md 6.4)"
                .to_owned()
        }
        SOUND_FILE => "the sound driver's note periods (sound.md 3.3)".to_owned(),
        map => format!("map {map}: chunk numbers and palette map (levels.md 4)"),
    }
}

/// Sound files are written one event or step per line.
const SOUND_RON_DEPTH: usize = 4;

/// Path of song `number` in the pack.
pub fn song_path(number: usize) -> String {
    format!("sound/song-{number:02}.ron")
}

/// Path of the effects in the pack.
pub const EFFECTS_PATH: &str = "sound/effects.ron";

fn entry(path: &str, kind: AssetKind, purpose: &str, certainty: Certainty) -> AssetEntry {
    AssetEntry {
        path: path.to_owned(),
        kind,
        purpose: purpose.to_owned(),
        certainty: certainty.name().to_owned(),
        tiles: None,
        block_tiles: None,
        destinations: Vec::new(),
        sources: Vec::new(),
    }
}

fn raw_source(offset: usize, len: usize) -> SourceEntry {
    SourceEntry {
        rom_offset: format!("0x{offset:05X}"),
        format: Format::Raw.name().to_owned(),
        stored_bytes: len as u32,
        decoded_bytes: len as u32,
        bias: None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rom::cartridge::PRG_SIZE;
    use crate::rom::pack::from_ron;
    use crate::rom::sheet::decode_sheet;

    /// A synthetic PRG ROM: a byte pattern, with preset and level tables that point where
    /// the spec says they should.
    pub(crate) fn synthetic_rom() -> Rom {
        let mut prg: Vec<u8> = (0..PRG_SIZE).map(|i| (i % 251) as u8).collect();
        // Palette presets: all at the start of the preset data, 4 entries from entry 1.
        for p in 0..palettes::PRESETS {
            prg[palettes::PRESET_POINTERS + 2 * p] = 0x26;
            prg[palettes::PRESET_POINTERS + 2 * p + 1] = 0xF5;
            prg[palettes::PRESET_FIRST_ENTRY + p] = 1;
            prg[palettes::PRESET_COUNT + p] = 4;
        }
        // Level headers: all at the first header, bank 2 $95C8.
        for level in 0..palettes::LEVELS {
            prg[palettes::HEADER_POINTERS_LOW + level] = 0xC8;
            prg[palettes::HEADER_POINTERS_HIGH + level] = 0x95;
        }
        crate::rom::sound::tests::add_sound(&mut prg);
        Rom::unchecked(prg)
    }

    fn text<'a>(pack: &'a AssetPack, path: &str) -> &'a str {
        std::str::from_utf8(&pack.files[path]).unwrap()
    }

    #[test]
    fn import_of_synthetic_rom() {
        let rom = synthetic_rom();
        let pack = build(&rom, false).unwrap();
        let manifest: Manifest = toml::from_str(text(&pack, MANIFEST_PATH)).unwrap();
        assert_eq!(manifest, pack.manifest);
        assert_eq!(manifest.importer_version, IMPORTER_VERSION);
        assert_eq!(manifest.rom_sha1, sha1_hex(rom.prg()));
        assert_eq!(pack.files.len(), manifest.assets.len() + 1);

        let digits = manifest
            .assets
            .iter()
            .find(|a| a.path == "tiles/hud/digits.png")
            .unwrap();
        assert_eq!(digits.kind, AssetKind::Tiles);
        assert_eq!(digits.tiles, Some(18));
        assert_eq!(digits.sources[0].rom_offset, "0x08100");
        let tiles = decode_sheet(&pack.files[&digits.path], 18).unwrap();
        assert_eq!(tiles, rom.prg()[0x08100..0x08220]);

        let warp = manifest
            .assets
            .iter()
            .find(|a| a.path == "tiles/warp/backgrounds.png")
            .unwrap();
        assert_eq!((warp.tiles, warp.block_tiles), (Some(12 * 17), Some(17)));
        assert_eq!(warp.destinations, vec!["$1810".to_owned()]);

        let title: Picture = from_ron(text(&pack, "pictures/title.ron")).unwrap();
        assert_eq!(title.writes.len(), 2);
        assert_eq!(title.writes[0].address, 0x2000);
        let stream = &rom.prg()[0x12B99..0x12B99 + 0x112];
        assert_eq!(
            title.writes[0].bytes.0,
            crate::rom::decode::run_length(stream, 0x64)
        );
        assert_eq!(title.writes[1].address, 0x23C0);

        let menu: Picture = from_ron(text(&pack, "pictures/menu-a.ron")).unwrap();
        assert!(menu.writes[0].add_tile_base);
        assert!(!menu.writes[1].add_tile_base);

        let presets: PalettePresets = from_ron(text(&pack, "palettes/presets.ron")).unwrap();
        assert_eq!(presets.presets.len(), 28);
        assert_eq!(presets.presets[0].first_entry, 1);
        assert_eq!(presets.presets[0].colours.0, rom.prg()[0x07526..0x0752A]);

        let levels: LevelPalettes = from_ron(text(&pack, "palettes/levels.ron")).unwrap();
        assert_eq!(levels.levels.len(), 42);
        assert_eq!(levels.levels[5].gameplay.0, rom.prg()[0x115D4..0x115E0]);
        assert_eq!(
            levels.levels[13].approach_globe.as_ref().unwrap().0,
            rom.prg()[0x0748A..0x0748D]
        );
        assert_eq!(
            levels.levels[13].surface.as_ref().unwrap().0,
            rom.prg()[0x07526..0x07532]
        );
        assert!(levels.levels[14].surface.is_none());

        let screens: ScreenPalettes = from_ron(text(&pack, "palettes/screens.ron")).unwrap();
        assert_eq!(screens.frames_screen.len(), 2);
        assert_eq!(screens.backdrop_variants.len(), 3);
        assert_eq!(screens.backdrop_variants[2].0, rom.prg()[0x38342..0x38346]);
        assert_eq!(screens.title_fade.len(), 6);
        assert_eq!(screens.title_fade[1].0, rom.prg()[0x077BA..0x077C1]);
        assert_eq!(
            screens.title_fade_end.0,
            [rom.prg()[0x276], rom.prg()[0x27A], rom.prg()[0x27E]]
        );
        assert_eq!(screens.title_flash_white.0, [rom.prg()[0x19F]]);
        assert_eq!(screens.analysis_pairs.len(), 17);
        assert_eq!(screens.analysis_pairs[3].0, rom.prg()[0x0559F..0x055A1]);

        let frames: PictureFrames = from_ron(text(&pack, attract_frames::PATH)).unwrap();
        assert_eq!(frames.nametable.len(), 11 * 11 * 16);
        assert_eq!(frames.attributes.len(), 11 * 12);

        let generated: GeneratedTileTables = from_ron(text(&pack, generated::PATH)).unwrap();
        assert_eq!(generated.gauge_rows.0, rom.prg()[0x01973..0x0197B]);
    }

    #[test]
    fn size_mismatch_is_reported() {
        let rom = Rom::unchecked(vec![0u8; PRG_SIZE]);
        let error = import(&rom).unwrap_err();
        assert!(
            matches!(error, ImportError::UnexpectedSize { .. }),
            "{error}"
        );
    }

    #[test]
    fn bad_preset_pointer_is_reported() {
        let mut prg = synthetic_rom().prg().to_vec();
        prg[palettes::PRESET_POINTERS + 2] = 0x00;
        prg[palettes::PRESET_POINTERS + 3] = 0x80;
        let error = build(&Rom::unchecked(prg), false).unwrap_err();
        assert!(matches!(error, ImportError::BadPointer { .. }), "{error}");
    }
}
