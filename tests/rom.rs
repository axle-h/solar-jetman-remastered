//! Tests against the player's ROM: `$SJ_ROM`, or the first `vendor/rom/*.nes`. Each test
//! skips (passes with a message) when no ROM is found. Nothing read from the ROM is written
//! anywhere.

use solar_jetman::rom::cartridge::{BANK_SIZE, bank_offset};
use solar_jetman::rom::catalogue::{Format, PICTURES, TILE_SETS, palettes};
use solar_jetman::rom::decode::TILE_BYTES;
use solar_jetman::rom::pack::{
    AssetKind, FlightTables, GeneratedTileTables, LevelPalettes, PalettePresets, Picture,
    PictureFrames, ScreenPalettes, from_ron,
};
use solar_jetman::rom::sheet::decode_sheet;
use solar_jetman::rom::{AssetPack, MANIFEST_PATH, Manifest, PRG_SHA1, Rom, import};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn rom_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SJ_ROM") {
        return Some(PathBuf::from(path));
    }
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/rom");
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
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

/// The checked ROM, or `None` (after saying so) when there is none.
fn rom() -> Option<&'static Rom> {
    static ROM: OnceLock<Option<Rom>> = OnceLock::new();
    ROM.get_or_init(|| {
        let Some(path) = rom_path() else {
            eprintln!("skipped: no ROM (set SJ_ROM or put the .nes file in vendor/rom/)");
            return None;
        };
        let file = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        Some(
            Rom::from_nes_file(&file).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        )
    })
    .as_ref()
}

fn pack() -> Option<&'static AssetPack> {
    static PACK: OnceLock<Option<AssetPack>> = OnceLock::new();
    PACK.get_or_init(|| rom().map(|rom| import(rom).unwrap_or_else(|error| panic!("{error}"))))
        .as_ref()
}

fn text<'a>(pack: &'a AssetPack, path: &str) -> &'a str {
    std::str::from_utf8(&pack.files[path]).unwrap()
}

fn word(rom: &Rom, low: usize, high: usize) -> u16 {
    u16::from_le_bytes([rom.byte(low).unwrap(), rom.byte(high).unwrap()])
}

#[test]
fn rom_is_the_usa_release() {
    let Some(rom) = rom() else { return };
    assert_eq!(rom.prg().len(), 8 * BANK_SIZE);
    assert_eq!(solar_jetman::rom::cartridge::sha1_hex(rom.prg()), PRG_SHA1);
}

#[test]
fn every_blob_lies_in_one_bank() {
    let blobs = TILE_SETS.iter().map(|set| set.blob).chain(
        PICTURES
            .iter()
            .flat_map(|p| p.parts.iter().map(|part| part.blob)),
    );
    for blob in blobs {
        let first_bank = blob.offset as usize / BANK_SIZE;
        let last_bank = (blob.end() - 1) / BANK_SIZE;
        assert_eq!(first_bank, last_bank, "blob at 0x{:05X}", blob.offset);
        // None reaches the common block at the end of every bank.
        assert!(
            blob.end() <= first_bank * BANK_SIZE + 0x7F7B,
            "blob at 0x{:05X}",
            blob.offset
        );
    }
}

#[test]
fn import_produces_every_asset() {
    let Some(pack) = pack() else { return };
    let manifest: Manifest = toml::from_str(text(pack, MANIFEST_PATH)).unwrap();
    assert_eq!(manifest, pack.manifest);
    assert_eq!(manifest.rom_sha1, PRG_SHA1);
    assert_eq!(
        manifest.assets.len(),
        TILE_SETS.len() + PICTURES.len() + 7 + 34 + 1 + 6 + 30,
        "tile sets, pictures, attract frames, 3 palette files, generated-tile tables, the flight's \
         picture and tables, 34 songs, the effects, 6 level data files and 30 maps"
    );
    assert_eq!(pack.files.len(), manifest.assets.len() + 1);
    let mut paths: Vec<&str> = manifest.assets.iter().map(|a| a.path.as_str()).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), manifest.assets.len(), "asset paths are unique");

    // Every file reads back with its own format.
    for asset in &manifest.assets {
        let file = &pack.files[&asset.path];
        match asset.kind {
            AssetKind::Tiles => {
                decode_sheet(file, asset.tiles.unwrap() as usize).unwrap();
            }
            AssetKind::Picture => {
                from_ron::<Picture>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::PictureFrames => {
                from_ron::<PictureFrames>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::PalettePresets => {
                from_ron::<PalettePresets>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::LevelPalettes => {
                from_ron::<LevelPalettes>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::ScreenPalettes => {
                from_ron::<ScreenPalettes>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::GeneratedTiles => {
                from_ron::<GeneratedTileTables>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::FlightTables => {
                from_ron::<FlightTables>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::Song => {
                from_ron::<solar_jetman::game::sound::Song>(text(pack, &asset.path)).unwrap();
            }
            AssetKind::Effects => {
                from_ron::<solar_jetman::game::sound::data::Effects>(text(pack, &asset.path))
                    .unwrap();
            }
            // Read together below: the files of `game/` only make sense as a whole.
            AssetKind::GameData => {
                assert!(asset.path.starts_with("game/"), "{}", asset.path);
            }
        }
    }
    solar_jetman::rom::world(&pack.files).unwrap();
    solar_jetman::rom::sprites(&pack.files).unwrap();
    solar_jetman::rom::note_periods(&pack.files).unwrap();
}

#[test]
fn tile_sets_decode_to_whole_tiles_that_fit_chr_ram() {
    let (Some(rom), Some(pack)) = (rom(), pack()) else {
        return;
    };
    for set in TILE_SETS {
        let decoded = set.blob.decode(rom).unwrap();
        if let Some(expected) = set.blob.decoded {
            assert_eq!(decoded.len(), expected as usize, "{}", set.path);
        }
        match set.blob.format {
            Format::Raw => assert_eq!(decoded.len() % TILE_BYTES, 0, "{}", set.path),
            Format::Font => assert_eq!(decoded.len(), set.blob.length as usize * 16),
            Format::RunLength { bias } => {
                assert_eq!(bias, 0, "tile streams have no bias: {}", set.path);
                assert!(decoded.len() >= TILE_BYTES, "{}", set.path);
            }
        }
        // What one load writes in whole tiles (a final partial tile is padding; the ending's
        // $1DC0 stream ends one byte past CHR RAM): the blob, or one block of a sheet.
        let mut loaded = decoded.len() / TILE_BYTES * TILE_BYTES;
        if let Some(block) = set.block_tiles {
            let tiles = decoded.len() / TILE_BYTES;
            assert_eq!(tiles % usize::from(block), 0, "{}", set.path);
            loaded = usize::from(block) * TILE_BYTES;
        }
        for &destination in set.destinations {
            assert_eq!(destination % 16, 0, "{}", set.path);
            assert!(
                usize::from(destination) + loaded <= 0x2000,
                "{} overflows CHR RAM from ${destination:04X}",
                set.path
            );
        }
        // The sheet holds the whole stream, a partial last tile padded with zeros.
        let count = decoded.len().div_ceil(TILE_BYTES);
        let sheet = decode_sheet(&pack.files[set.path], count).unwrap();
        assert_eq!(sheet[..decoded.len()], decoded[..], "{}", set.path);
        assert!(
            sheet[decoded.len()..].iter().all(|&b| b == 0),
            "{}",
            set.path
        );
    }
}

#[test]
fn surface_tile_streams_stay_in_their_pattern_table() {
    let Some(rom) = rom() else { return };
    for set in TILE_SETS
        .iter()
        .filter(|set| set.path.starts_with("tiles/surface/"))
    {
        let decoded = set.blob.decode(rom).unwrap();
        let destination = usize::from(set.destinations[0]);
        let table_end = (destination / 0x1000 + 1) * 0x1000;
        assert!(
            destination + decoded.len() <= table_end,
            "{}: {:#X} bytes from ${destination:04X}",
            set.path,
            decoded.len()
        );
    }
}

#[test]
fn pictures_fit_the_nametable() {
    let Some(pack) = pack() else { return };
    for picture in PICTURES {
        let parsed: Picture = from_ron(text(pack, picture.path)).unwrap();
        assert_eq!(parsed.writes.len(), picture.parts.len());
        for (write, part) in parsed.writes.iter().zip(picture.parts) {
            assert_eq!(write.address, part.address);
            assert!(
                usize::from(write.address) + write.bytes.len() <= 0x2400,
                "{} at ${:04X}: {:#X} bytes",
                picture.path,
                write.address,
                write.bytes.len()
            );
            if let Some(expected) = part.blob.decoded {
                assert_eq!(write.bytes.len(), expected as usize, "{}", picture.path);
            }
        }
    }
}

/// The catalogue agrees with the location tables the original itself uses.
#[test]
fn catalogue_matches_the_roms_location_tables() {
    let Some(rom) = rom() else { return };
    let set = |path: &str| {
        TILE_SETS
            .iter()
            .find(|set| set.path == path)
            .unwrap_or_else(|| panic!("{path}"))
            .blob
    };
    let picture = |path: &str| {
        PICTURES
            .iter()
            .find(|p| p.path == path)
            .unwrap_or_else(|| panic!("{path}"))
            .parts
    };

    // Bank 5 surface picture tables: start low/high and length low/high, 7 entries each.
    let table = |start_low: u16, start_high: u16, len_low: u16, len_high: u16, picture: usize| {
        let at = |address: u16| bank_offset(5, address) + picture;
        let start = word(rom, at(start_low), at(start_high));
        let len = word(rom, at(len_low), at(len_high));
        (start, len)
    };
    for number in [0usize, 1, 2, 4, 5, 6] {
        let bank = if number == 6 { 7 } else { 5 };
        let checks = [
            (
                (0x811D, 0x8124, 0x812B, 0x8132),
                set(&format!("tiles/surface/picture-{number}-table-1.png")),
            ),
            (
                (0x8139, 0x8140, 0x8147, 0x814E),
                set(&format!("tiles/surface/picture-{number}-table-0.png")),
            ),
            (
                (0x8155, 0x815C, 0x8163, 0x816A),
                picture(&format!("pictures/surface-{number}.ron"))[0].blob,
            ),
            (
                (0x8171, 0x8178, 0x817F, 0x8186),
                picture(&format!("pictures/surface-{number}.ron"))[1].blob,
            ),
        ];
        for ((sl, sh, ll, lh), blob) in checks {
            let (start, len) = table(sl, sh, ll, lh, number);
            if start >> 8 == 0xFF {
                // Stored in bank 7; the bank 5 table does not locate it.
                assert_eq!(bank, 7, "only picture 6 is in bank 7");
                continue;
            }
            assert_eq!(
                bank_offset(5, start),
                blob.offset as usize,
                "picture {number}"
            );
            assert_eq!(usize::from(len), blob.stored_bytes(), "picture {number}");
        }
        let (start, len) = table(0x818D, 0x8194, 0x819B, 0x81A2, number);
        let extra = match number {
            1 => "tiles/surface/extra-picture-1.png",
            6 => "tiles/surface/extra-picture-6.png",
            _ => "tiles/surface/extra-shared.png",
        };
        assert_eq!(
            bank_offset(5, start),
            set(extra).offset as usize,
            "picture {number}"
        );
        assert_eq!(
            usize::from(len),
            set(extra).stored_bytes(),
            "picture {number}"
        );
    }

    // Bank 2 terrain stream table, by graphics type: 7 length-low, 7 length-high, 7
    // start-low, 7 start-high.
    let terrain = bank_offset(2, 0x94E4);
    for graphics_type in 0..7 {
        let len = word(rom, terrain + graphics_type, terrain + 7 + graphics_type);
        let start = word(
            rom,
            terrain + 14 + graphics_type,
            terrain + 21 + graphics_type,
        );
        let path = match graphics_type {
            0 | 4 => "tiles/terrain/types-0-4.png",
            1 | 6 => "tiles/terrain/types-1-6.png",
            _ => "tiles/terrain/types-2-3-5.png",
        };
        assert_eq!(
            bank_offset(2, start),
            set(path).offset as usize,
            "type {graphics_type}"
        );
        assert_eq!(
            usize::from(len),
            set(path).stored_bytes(),
            "type {graphics_type}"
        );
    }

    // Graphics type → surface picture number.
    assert_eq!(
        rom.bytes(bank_offset(2, 0x9567), 7).unwrap(),
        [1, 2, 0, 4, 5, 0, 6]
    );

    // Bank 4 launch table: start low, start high, length low, length high, 3 pod types each.
    let launch = bank_offset(4, 0x9590);
    for pod in 0..3 {
        let start = word(rom, launch + pod, launch + 3 + pod);
        let len = word(rom, launch + 6 + pod, launch + 9 + pod);
        let blob = set(&format!("tiles/launch/pod-type-{pod}.png"));
        assert_eq!(
            bank_offset(4, start),
            blob.offset as usize,
            "pod type {pod}"
        );
        assert_eq!(usize::from(len), blob.stored_bytes(), "pod type {pod}");
    }

    // Bank 5 equipment item → picture map.
    assert_eq!(
        rom.bytes(bank_offset(5, 0x81E0), 8).unwrap(),
        [0x00, 0xFF, 0xFF, 0x00, 0x01, 0xFF, 0x02, 0x02]
    );
}

#[test]
fn palette_presets_match_the_spec_table() {
    let (Some(rom), Some(pack)) = (rom(), pack()) else {
        return;
    };
    // graphics.md 7.2: data address, first entry, count.
    const PRESETS: [(u16, u8, u8); 28] = [
        (0xF526, 0, 32),
        (0xF546, 1, 16),
        (0xF556, 17, 15),
        (0xF565, 0, 32),
        (0xF585, 1, 3),
        (0xF588, 1, 31),
        (0xF5A7, 8, 8),
        (0xF5AF, 16, 4),
        (0xF5B3, 9, 7),
        (0xF5BA, 1, 11),
        (0xF5C5, 16, 8),
        (0xF5CD, 1, 7),
        (0xF5D4, 1, 27),
        (0xF5EF, 1, 27),
        (0xF60A, 1, 27),
        (0xF5D4, 1, 27),
        (0xF625, 1, 27),
        (0xF640, 1, 27),
        (0xF65B, 1, 23),
        (0xF676, 1, 31),
        (0xF695, 0, 32),
        (0xF6B5, 1, 15),
        (0xF6C4, 1, 31),
        (0xF6E3, 1, 15),
        (0xF6F2, 9, 7),
        (0xF6F9, 9, 7),
        (0xF700, 13, 3),
        (0xF703, 1, 27),
    ];
    let presets: PalettePresets = from_ron(text(pack, "palettes/presets.ron")).unwrap();
    for (index, (&(address, first, count), preset)) in
        PRESETS.iter().zip(&presets.presets).enumerate()
    {
        let pointer = palettes::PRESET_POINTERS + 2 * index;
        assert_eq!(word(rom, pointer, pointer + 1), address, "preset {index}");
        assert_eq!(preset.first_entry, first, "preset {index}");
        assert_eq!(preset.colours.len(), usize::from(count), "preset {index}");
        // NES colour numbers are 6-bit.
        assert!(preset.colours.iter().all(|&c| c < 0x40), "preset {index}");
    }
}

#[test]
fn level_headers_and_palettes() {
    let (Some(rom), Some(pack)) = (rom(), pack()) else {
        return;
    };
    // graphics.md 6.2: graphics type per level (header byte 11, bits 0-2).
    let mut types = [0u8; 42];
    for (graphics_type, levels) in [
        (3, &[0, 6][..]),
        (6, &[1]),
        (4, &[2, 9]),
        (2, &[3, 11, 16, 20, 23, 27, 28]),
        (0, &[4, 7, 17, 19, 21, 25]),
        (1, &[5, 8, 10, 13, 14, 15, 18, 22, 24, 26]),
        (5, &[12]),
    ] {
        for &level in levels {
            types[level] = graphics_type;
        }
    }
    for (level, &graphics_type) in types.iter().enumerate() {
        let address = word(
            rom,
            palettes::HEADER_POINTERS_LOW + level,
            palettes::HEADER_POINTERS_HIGH + level,
        );
        let header = bank_offset(2, address);
        assert_eq!(
            rom.byte(header + 11).unwrap() & 7,
            graphics_type,
            "level {level}"
        );
    }

    let levels: LevelPalettes = from_ron(text(pack, "palettes/levels.ron")).unwrap();
    assert_eq!(levels.levels.len(), 42);
    for (level, palette) in levels.levels.iter().enumerate() {
        assert_eq!(palette.gameplay.len(), 12, "level {level}");
        assert!(palette.gameplay.iter().all(|&c| c < 0x40), "level {level}");
        let planet = level < 14;
        assert_eq!(palette.approach_globe.is_some(), planet, "level {level}");
        assert_eq!(palette.surface.is_some(), planet, "level {level}");
        if let (Some(globe), Some(surface)) = (&palette.approach_globe, &palette.surface) {
            assert_eq!(globe.len(), 3);
            assert_eq!(surface.len(), 12);
            assert!(globe.iter().chain(surface.iter()).all(|&c| c < 0x40));
        }
    }

    let screens: ScreenPalettes = from_ron(text(pack, "palettes/screens.ron")).unwrap();
    for colours in screens
        .frames_screen
        .iter()
        .chain(&screens.backdrop_variants)
        .chain([&screens.backdrop_sprites])
        .chain(&screens.title_fade)
        .chain([&screens.title_fade_end, &screens.title_flash_white])
    {
        assert!(colours.iter().all(|&c| c < 0x40), "{colours:?}");
    }
}

/// sound.md 2: decoding every song and effect from the pointer tables reads every byte of
/// 0x22A17-0x27E4B; 3.3: the period table's ends.
#[test]
fn sound_decodes_and_reaches_all_its_data() {
    use solar_jetman::game::sound::data::{Event, RAM_EFFECT_ID};
    let Some(rom) = rom() else { return };
    let sound = solar_jetman::rom::sound::decode(rom).unwrap_or_else(|error| panic!("{error}"));
    let unread: Vec<usize> = sound
        .coverage
        .iter()
        .enumerate()
        .filter(|(_, read)| !**read)
        .map(|(i, _)| solar_jetman::rom::sound::at::DATA.0 + i)
        .collect();
    assert!(unread.is_empty(), "unread bytes at {unread:X?}");
    assert_eq!(sound.songs.len(), 34);
    assert_eq!(sound.effects.len(), 34);
    for track in &sound.songs[0].tracks {
        assert_eq!(track.events, [Event::End]);
    }
    let hum = sound
        .effects
        .iter()
        .find(|e| e.id == RAM_EFFECT_ID)
        .unwrap();
    assert!(hum.in_ram && hum.steps.len() == 1);
    // Commands the spec says are never used.
    for song in &sound.songs {
        for track in &song.tracks {
            for event in &track.events {
                assert!(
                    !matches!(
                        event,
                        Event::VibratoOff
                            | Event::VolumeRamp { .. }
                            | Event::VolumeWave { .. }
                            | Event::VolumeEffectOff
                            | Event::Tempo(_)
                            | Event::TempoAdd(_)
                            | Event::Transpose(_)
                            | Event::RetriggerOff
                    ),
                    "{event:?}"
                );
            }
        }
    }
    let periods = solar_jetman::rom::sound::note_periods(rom).unwrap();
    assert_eq!(periods.len(), 61);
    assert_eq!((periods[0], periods[1], periods[60]), (0, 0x06AE, 0x0038));
    assert!(periods[1..].windows(2).all(|pair| pair[0] > pair[1]));
}

#[test]
fn sound_reads_back_from_the_pack() {
    let (Some(rom), Some(pack)) = (rom(), pack()) else {
        return;
    };
    let periods = solar_jetman::rom::sound::note_periods(rom).unwrap();
    let sound =
        solar_jetman::rom::sound_data(&pack.files, periods).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(sound.songs.len(), 34);
    assert!(
        pack.manifest
            .assets
            .iter()
            .any(|a| a.path == "sound/effects.ron")
    );
}

#[test]
fn flight_data_is_the_picture_and_the_two_tables() {
    let (Some(rom), Some(pack)) = (rom(), pack()) else {
        return;
    };
    // Message 49 (text.md 3): picture mode, seven rows of the mothership and one attribute byte.
    let picture: Picture = from_ron(text(pack, "pictures/flight.ron")).unwrap();
    let rows: Vec<(u16, usize)> = picture
        .writes
        .iter()
        .map(|w| (w.address, w.bytes.0.len()))
        .collect();
    assert_eq!(
        rows,
        [
            (0x2006, 4),
            (0x2026, 4),
            (0x2043, 10),
            (0x2063, 10),
            (0x2083, 10),
            (0x20A2, 12),
            (0x20C1, 14),
            (0x23CA, 1)
        ]
    );
    let tables: FlightTables = from_ron(text(pack, "tables/flight.ron")).unwrap();
    assert_eq!(tables.window_colours.0, rom.prg()[0x00996..0x00996 + 12]);
    assert_eq!(tables.ring_sine.0, rom.prg()[0x38C74..0x38C74 + 128]);
    assert_eq!(tables.ring_sine.0[0], 0);
    assert_eq!(tables.ring_sine.0[64], 0x80);
}
