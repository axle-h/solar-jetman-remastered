//! Level data extraction (`docs/spec/levels.md`): reads the level headers, object lists, maps,
//! palette maps, block sets, chunks and collision shapes from a checked ROM into our own
//! [`World`], whose files the import puts into the pack's `game/` folder together with the
//! sprite layouts and the sound driver's note period table. Header bytes 12-23 (colours) are
//! left out here: the import reads them as graphics.

use crate::game::gameplay::ValidationInput;
use crate::game::gameplay::objdata::{
    DEMO_WAYPOINTS, FLAG_ENTRIES_HIGH, FLAG_ENTRIES_LOW, HIT_POINT_ENTRIES, ObjectData,
    SHIMMER_BYTES, SIZED_FRAMES, TYPE_COUNT,
};
use crate::game::gameplay::sprites::{
    LAYOUTS_A, LAYOUTS_B, ObjectSets, POD_LAYOUTS, Piece, SPRITES_FILE, SetRecord, SpriteFile,
};
use crate::game::hex::{DigitGrid, HexGrid, HexRows};
use crate::game::level::{
    BLOCK_SETS, BLOCKS_PER_SET, Blocks, CHUNKS, ChunkMap, Collision, LEVEL_COUNT, Level, Line,
    Placement, Point, SHAPE_CLASSES, Segment, ShapeClass, World,
};
use crate::game::sound::data::{SOUND_FILE, SoundTables};
use crate::rom::Rom;
use crate::rom::cartridge::bank_offset;
use std::collections::BTreeMap;

/// Where the level data is (levels.md 3, 4.1, 6.2), as bank and CPU address.
mod at {
    pub const HEADER_POINTERS_LOW: (u8, u16) = (2, 0x956E);
    pub const HEADER_POINTERS_HIGH: (u8, u16) = (2, 0x9598);
    pub const HEADERS: (u8, u16, u16) = (2, 0x95C8, 0x9A35);
    pub const HEADER_BYTES: usize = 27;
    pub const SURFACE_PICTURES: (u8, u16) = (2, 0x9567);
    pub const GRAPHICS_TYPES: usize = 7;
    pub const OBJECT_LISTS: (u8, u16, u16) = (2, 0x9A36, 0xA09D);
    pub const OBJECT_BYTES: usize = 5;
    pub const MAP_POINTERS: (u8, u16) = (3, 0x859A);
    pub const MAPS: (u8, u16, u16) = (3, 0x870B, 0xBA01);
    pub const BLOCK_SET_POINTERS_LOW: (u8, u16) = (3, 0x8594);
    pub const BLOCK_SET_POINTERS_HIGH: (u8, u16) = (3, 0x8597);
    pub const CHUNKS: (u8, u16) = (3, 0xC1FA);
    pub const PALETTE_MAP_POINTERS_LOW: (u8, u16) = (0, 0xE1D5);
    pub const PALETTE_MAP_POINTERS_HIGH: (u8, u16) = (0, 0xE1FF);
    pub const PALETTE_MAPS: (u8, u16, u16) = (0, 0xE5F5, 0xF1E8);
    pub const CLASS_MAP_POINTERS_LOW: (u8, u16) = (2, 0x95C2);
    pub const CLASS_MAP_POINTERS_HIGH: (u8, u16) = (2, 0x95C5);
    pub const CLASS_POINTERS_LOW: (u8, u16) = (2, 0xF8EA);
    pub const CLASS_POINTERS_HIGH: (u8, u16) = (2, 0xF91F);
    pub const CLASS_RECORDS: (u8, u16, u16) = (2, 0xF954, 0xFACB);
    /// The pod's sprite layout table P (objects.md 6.1): low and high address bytes.
    pub const POD_LAYOUTS_LOW: (u8, u16) = (1, 0xA859);
    pub const POD_LAYOUTS_HIGH: (u8, u16) = (1, 0xA879);
    /// Layout tables A and B (objects.md 6.1) and the range their layouts lie in.
    pub const LAYOUTS_A_LOW: (u8, u16) = (1, 0xA899);
    pub const LAYOUTS_A_HIGH: (u8, u16) = (1, 0xA95A);
    pub const LAYOUTS_B_LOW: (u8, u16) = (1, 0xAA1B);
    pub const LAYOUTS_B_HIGH: (u8, u16) = (1, 0xAAF5);
    pub const LAYOUT_RANGE: (u16, u16) = (0xA54C, 0xC88B);
    /// Object sprite sets: per-level lists, and the records of list A and B (graphics.md 6.4).
    pub const SET_LISTS: (u8, u16, u16) = (1, 0xC980, 0xCAB3);
    pub const SET_RECORDS_A: (u8, u16) = (1, 0xCAB4);
    pub const SET_RECORDS_B: (u8, u16) = (1, 0xCB13);
    pub const SET_RECORDS_A_COUNT: usize = 19;
    pub const SET_RECORDS_B_COUNT: usize = 10;
    /// Placement data (objects.md 3.1, 4.1).
    pub const RECORDS_LOW: (u8, u16) = (4, 0x80F3);
    pub const RECORDS_HIGH: (u8, u16) = (4, 0x811D);
    pub const OBJECT_COUNTS: (u8, u16) = (4, 0x8147);
    pub const TYPE_LISTS_LOW: (u8, u16) = (4, 0x8171);
    pub const TYPE_LISTS_HIGH: (u8, u16) = (4, 0x819B);
    pub const TYPE_FRAMES: (u8, u16) = (4, 0xA25D);
    pub const TYPE_STATES: (u8, u16) = (4, 0xA2BE);
    /// Hit points per state (bank 7), flag bytes and sizes (bank 1), objects.md 2.4, 8.
    pub const HIT_POINTS: (u8, u16) = (7, 0xAB07);
    pub const FLAGS_LOW: (u8, u16) = (1, 0x87F8);
    pub const FLAGS_HIGH: (u8, u16) = (1, 0x881D);
    pub const HALF_WIDTHS: (u8, u16) = (1, 0x8685);
    pub const HALF_HEIGHTS: (u8, u16) = (1, 0x8746);
    /// The demo's waypoint lists (game-flow.md 7.1): 88 entries of 5 bytes.
    pub const DEMO_WAYPOINTS: (u8, u16) = (6, 0x8036);
    /// The warp levels' mesh helper tables (objects.md 10.6): flags, then the sources of
    /// slot 23's x and y and of slot 24's, 13 bytes each.
    pub const WARP_MESH: (u8, u16) = (0, 0xA86E);
    /// The first source table, from where a byte-sized index reads on (objects.md 10.6, Q38).
    /// The run goes past the tables into the program, so it is read at run time for the
    /// validation and never written to the pack.
    pub const WARP_SHIMMER: (u8, u16) = (0, 0xA87B);
}

/// End of a list in a shape class record.
const END: u8 = 0xFF;

/// Reads the ROM through bank-relative addresses, reporting what is out of place.
struct Reader<'a> {
    rom: &'a Rom,
}

impl Reader<'_> {
    fn bytes(&self, bank: u8, address: u16, len: usize) -> Result<&[u8], String> {
        let start = bank_offset(bank, address);
        // Data never crosses into the next bank.
        if address < 0x8000 || usize::from(address - 0x8000) + len > 0x8000 {
            return Err(format!(
                "{len} bytes at bank {bank}:${address:04X} leave the bank"
            ));
        }
        self.rom
            .bytes(start, len)
            .ok_or_else(|| format!("bank {bank}:${address:04X} is past the end of the ROM"))
    }

    fn byte(&self, (bank, address): (u8, u16), index: usize) -> Result<u8, String> {
        Ok(self.bytes(bank, address + index as u16, 1)?[0])
    }

    /// A 16-bit address from tables of low and high bytes.
    fn split_pointer(&self, low: (u8, u16), high: (u8, u16), index: usize) -> Result<u16, String> {
        Ok(u16::from_le_bytes([
            self.byte(low, index)?,
            self.byte(high, index)?,
        ]))
    }

    /// A 16-bit little-endian address from a table of words.
    fn word_pointer(&self, table: (u8, u16), index: usize) -> Result<u16, String> {
        Ok(u16::from_le_bytes([
            self.byte(table, 2 * index)?,
            self.byte(table, 2 * index + 1)?,
        ]))
    }
}

fn check_within(
    what: &str,
    address: u16,
    (bank, first, last): (u8, u16, u16),
) -> Result<(), String> {
    if (first..=last).contains(&address) {
        Ok(())
    } else {
        Err(format!(
            "{what} at bank {bank}:${address:04X} is outside ${first:04X}-${last:04X}"
        ))
    }
}

/// Reads all level data from the ROM.
pub fn extract(rom: &Rom) -> Result<World, String> {
    let read = Reader { rom };
    let surface_pictures = read
        .bytes(
            at::SURFACE_PICTURES.0,
            at::SURFACE_PICTURES.1,
            at::GRAPHICS_TYPES,
        )?
        .to_vec();

    let mut levels = Vec::with_capacity(LEVEL_COUNT);
    let mut maps: BTreeMap<String, ChunkMap> = BTreeMap::new();
    for number in 0..LEVEL_COUNT {
        let (level, map) = level(&read, number, &surface_pictures)
            .map_err(|message| format!("level {number}: {message}"))?;
        // Levels with the same map share one file, named after the first of them.
        let name = match maps.iter().find(|(_, known)| **known == map) {
            Some((name, _)) => name.clone(),
            None => {
                let name = format!("level-{number:02}");
                maps.insert(name.clone(), map);
                name
            }
        };
        levels.push(Level { map: name, ..level });
    }

    let mut block_sets = Vec::with_capacity(BLOCK_SETS);
    let mut block_classes = Vec::with_capacity(BLOCK_SETS);
    for set in 0..BLOCK_SETS {
        let address =
            read.split_pointer(at::BLOCK_SET_POINTERS_LOW, at::BLOCK_SET_POINTERS_HIGH, set)?;
        block_sets.push(HexRows(
            read.bytes(3, address, 4 * BLOCKS_PER_SET)?.to_vec(),
        ));
        let address =
            read.split_pointer(at::CLASS_MAP_POINTERS_LOW, at::CLASS_MAP_POINTERS_HIGH, set)?;
        block_classes.push(HexRows(read.bytes(2, address, BLOCKS_PER_SET)?.to_vec()));
    }
    let chunk_bytes = read.bytes(at::CHUNKS.0, at::CHUNKS.1, CHUNKS * 64)?;
    let chunks = chunk_bytes
        .chunks(64)
        .map(|chunk| HexRows(chunk.to_vec()))
        .collect();

    let mut classes = Vec::with_capacity(SHAPE_CLASSES);
    for class in 0..SHAPE_CLASSES {
        let address = read.split_pointer(at::CLASS_POINTERS_LOW, at::CLASS_POINTERS_HIGH, class)?;
        check_within(&format!("shape class {class}"), address, at::CLASS_RECORDS)?;
        let end = usize::from(at::CLASS_RECORDS.2 - address) + 1;
        let record = read.bytes(2, address, end)?;
        let segments = parse_shape_class(record)
            .map_err(|message| format!("shape class {class}: {message}"))?;
        classes.push(ShapeClass { segments });
    }

    Ok(World {
        surface_picture_by_graphics_type: HexRows(surface_pictures),
        levels,
        maps,
        blocks: Blocks { block_sets, chunks },
        collision: Collision {
            block_classes,
            classes,
        },
        objects: object_data(rom)?,
    })
}

/// One level's header fields, object list and map (with its palettes). The level's `map` name
/// is left empty for the caller.
fn level(
    read: &Reader,
    number: usize,
    surface_pictures: &[u8],
) -> Result<(Level, ChunkMap), String> {
    let address = read.split_pointer(at::HEADER_POINTERS_LOW, at::HEADER_POINTERS_HIGH, number)?;
    check_within("header", address, at::HEADERS)?;
    let header = read.bytes(2, address, at::HEADER_BYTES)?;
    let big_endian = |i: usize| u16::from_be_bytes([header[i], header[i + 1]]);
    let width = header[8].wrapping_add(1);
    let height = header[9].wrapping_add(1);
    if header[8] % 2 == 0 {
        return Err(format!("width byte {} is not odd", header[8]));
    }
    let graphics_type = header[11] & 7;
    let picture = *surface_pictures
        .get(usize::from(graphics_type))
        .ok_or_else(|| format!("graphics type {graphics_type}"))?;

    let count = usize::from(header[24]);
    let list = u16::from_le_bytes([header[25], header[26]]);
    check_within("object list", list, at::OBJECT_LISTS)?;
    let list_bytes = read.bytes(2, list, count * at::OBJECT_BYTES)?;
    let objects = list_bytes
        .chunks(at::OBJECT_BYTES)
        .map(|entry| Placement {
            x: u16::from_le_bytes([entry[0], entry[1]]),
            y: u16::from_le_bytes([entry[2], entry[3]]),
            kind: entry[4],
        })
        .collect();

    // levels.md 4.2: H + 1 rows of S bytes; S = 2 × ((byte 8 >> 1) + 1).
    let stride = 2 * (usize::from(header[8] >> 1) + 1);
    let rows = usize::from(height) + 1;
    let map_address = read.word_pointer(at::MAP_POINTERS, number)?;
    check_within("map", map_address, at::MAPS)?;
    let chunks = read.bytes(3, map_address, rows * stride)?.to_vec();

    // levels.md 4.5: H / 4 + 1 rows of S bytes, 2 bits per chunk row.
    let palette_address = read.split_pointer(
        at::PALETTE_MAP_POINTERS_LOW,
        at::PALETTE_MAP_POINTERS_HIGH,
        number,
    )?;
    check_within("palette map", palette_address, at::PALETTE_MAPS)?;
    let palette_rows = usize::from(height) / 4 + 1;
    let palette_bytes = read.bytes(0, palette_address, palette_rows * stride)?;
    let palettes = (0..rows)
        .flat_map(|cy| {
            (0..stride).map(move |cx| (palette_bytes[cy / 4 * stride + cx] >> (2 * (cy % 4))) & 3)
        })
        .collect();

    let level = Level {
        number: number as u8,
        camera_start: Point {
            x: big_endian(0),
            y: big_endian(2),
        },
        home: Point {
            x: big_endian(4),
            y: big_endian(6),
        },
        width,
        height,
        object_margin: header[10],
        graphics_type,
        gravity: (header[11] & 0xF8) as i8,
        block_set: picture & 3,
        map: String::new(),
        objects,
    };
    let map = ChunkMap {
        chunks: HexGrid::new(stride, chunks),
        palettes: DigitGrid::new(stride, palettes),
    };
    Ok((level, map))
}

/// Parses a shape class record (levels.md 6.2): the directions, $FF, then the segments, with
/// the next direction and $FF between two segments and $FF after the last.
pub fn parse_shape_class(record: &[u8]) -> Result<Vec<Segment>, String> {
    let mut rest = record;
    let mut take = |n: usize| take_bytes(&mut rest, n);
    let mut directions = Vec::new();
    loop {
        let byte = take(1)?[0];
        if byte == END {
            break;
        }
        if byte > 15 {
            return Err(format!("direction {byte}"));
        }
        directions.push(byte);
    }
    let mut segments = Vec::with_capacity(directions.len());
    for (i, &direction) in directions.iter().enumerate() {
        if i > 0 {
            let separator = take(2)?;
            if separator != [direction, END] {
                return Err(format!(
                    "separator {:02X} {:02X} before segment {i}, expected {direction:02X} FF",
                    separator[0], separator[1]
                ));
            }
        }
        let size = if matches!(direction, 0 | 7 | 8 | 9) {
            3
        } else {
            4
        };
        let b: Vec<i8> = take(size)?.iter().map(|&byte| byte as i8).collect();
        let line = match direction {
            0 | 9 => Line::Flat {
                x0: b[0],
                width: b[1],
                y: b[2],
            },
            7 | 8 => Line::Wall {
                x: b[0],
                y0: b[1],
                height: b[2],
            },
            1..=6 => Line::Floor {
                y_low: b[0],
                x0: b[1],
                width: b[2],
                y_high: b[3],
            },
            _ => Line::Ceiling {
                y_top: b[0],
                x0: b[1],
                width: b[2],
                y_bottom: b[3],
            },
        };
        segments.push(Segment { direction, line });
    }
    if !directions.is_empty() && take(1)?[0] != END {
        return Err("no end marker after the last segment".to_owned());
    }
    Ok(segments)
}

fn take_bytes<'a>(rest: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if rest.len() < n {
        return Err("record ends early".to_owned());
    }
    let (head, tail) = rest.split_at(n);
    *rest = tail;
    Ok(head)
}

/// The files the extraction produces, by path inside the pack's `game/` folder.
pub fn game_files(rom: &Rom) -> Result<BTreeMap<String, String>, String> {
    let mut files = extract(rom)?
        .to_files()
        .map_err(|error| format!("writing RON: {error}"))?;
    // The note period table: tuning constants (sound.md 2, 3.3).
    let tables = SoundTables {
        note_periods: crate::rom::sound::note_periods(rom)?,
    };
    files.insert(
        SOUND_FILE.to_owned(),
        crate::game::level::to_ron(&tables).map_err(|error| format!("writing RON: {error}"))?,
    );
    files.insert(
        SPRITES_FILE.to_owned(),
        crate::game::level::to_ron(&sprite_file(rom)?)
            .map_err(|error| format!("writing RON: {error}"))?,
    );
    Ok(files)
}

/// One sprite layout at a bank 1 address (objects.md 6.2).
fn layout(reader: &Reader, address: u16) -> Result<Vec<Piece>, String> {
    let count = usize::from(reader.bytes(1, address, 1)?[0]);
    if count == 0 {
        // One sprite at the object point: tile, attributes.
        let bytes = reader.bytes(1, address + 1, 2)?;
        return Ok(vec![Piece {
            dx: 0,
            dy: 0,
            tile: bytes[0],
            attributes: bytes[1],
        }]);
    }
    Ok(reader
        .bytes(1, address + 1, count * 4)?
        .chunks(4)
        .map(|b| Piece {
            dx: b[0] as i8,
            dy: b[1] as i8,
            tile: b[2],
            attributes: b[3],
        })
        .collect())
}

/// `count` layouts of a table of low and high address bytes. Entries whose address lies outside
/// the layout area are unused and come out empty (objects.md 6.1).
fn layout_table(
    reader: &Reader,
    low: (u8, u16),
    high: (u8, u16),
    count: usize,
) -> Result<Vec<Vec<Piece>>, String> {
    let mut table = Vec::with_capacity(count);
    for index in 0..count {
        let address = reader.split_pointer(low, high, index)?;
        if (at::LAYOUT_RANGE.0..=at::LAYOUT_RANGE.1).contains(&address) {
            table.push(layout(reader, address)?);
        } else {
            table.push(Vec::new());
        }
    }
    Ok(table)
}

/// The sprite sets records at `table` (graphics.md 6.4).
fn set_records(reader: &Reader, table: (u8, u16), count: usize) -> Result<Vec<SetRecord>, String> {
    Ok(reader
        .bytes(table.0, table.1, 5 * count)?
        .chunks(5)
        .map(|b| SetRecord {
            first: b[0],
            end: b[1],
            address: u16::from_le_bytes([b[2], b[3]]),
            count: b[4],
        })
        .collect())
}

/// The sprite layouts and which tile sets each level loads (objects.md 5.2, 6; graphics.md
/// 6.4).
pub fn sprite_file(rom: &Rom) -> Result<SpriteFile, String> {
    let reader = Reader { rom };
    let mut pod = Vec::new();
    for index in 0..POD_LAYOUTS {
        let address = reader.split_pointer(at::POD_LAYOUTS_LOW, at::POD_LAYOUTS_HIGH, index)?;
        pod.push(layout(&reader, address)?);
    }
    let mut table_a = layout_table(&reader, at::LAYOUTS_A_LOW, at::LAYOUTS_A_HIGH, LAYOUTS_A)?;
    // objects.md 6.5a: A$BF's count byte says 12 sprites but its picture has 10; the last two
    // are read from the next layout (a data slip), so they are left out.
    table_a[0xBF].truncate(10);
    let table_b = layout_table(&reader, at::LAYOUTS_B_LOW, at::LAYOUTS_B_HIGH, LAYOUTS_B)?;

    // The per-level lists: for each level a count and that many bytes (list A), then the same
    // for list B.
    let (bank, first, last) = at::SET_LISTS;
    let bytes = reader.bytes(bank, first, usize::from(last - first) + 1)?;
    let mut at_byte = 0;
    let mut take = || -> Result<Vec<u8>, String> {
        let count = usize::from(*bytes.get(at_byte).ok_or("the sprite set lists end early")?);
        let list = bytes
            .get(at_byte + 1..at_byte + 1 + count)
            .ok_or("the sprite set lists end early")?
            .to_vec();
        at_byte += 1 + count;
        Ok(list)
    };
    let (mut lists_a, mut lists_b) = (Vec::new(), Vec::new());
    for _ in 0..LEVEL_COUNT {
        lists_a.push(HexRows(take()?));
        lists_b.push(HexRows(take()?));
    }
    if at_byte != bytes.len() {
        return Err(format!(
            "the sprite set lists end at {at_byte}, expected {}",
            bytes.len()
        ));
    }
    let records_a = set_records(&reader, at::SET_RECORDS_A, at::SET_RECORDS_A_COUNT)?;
    let records_b = set_records(&reader, at::SET_RECORDS_B, at::SET_RECORDS_B_COUNT)?;
    Ok(SpriteFile {
        pod,
        table_a,
        table_b,
        sets: ObjectSets {
            lists_a,
            lists_b,
            records_a,
            records_b,
        },
    })
}

/// The placement list of one level (objects.md 3.2): `count` positions from the record list,
/// types from the type list.
fn placements(reader: &Reader, level: usize) -> Result<(Vec<Placement>, Vec<Option<u8>>), String> {
    let records = reader.split_pointer(at::RECORDS_LOW, at::RECORDS_HIGH, level)?;
    let types = reader.split_pointer(at::TYPE_LISTS_LOW, at::TYPE_LISTS_HIGH, level)?;
    let count = usize::from(reader.byte(at::OBJECT_COUNTS, level)?);
    let types = reader.bytes(4, types, count)?;
    // Records are read byte by byte; a group entry is x, y, k and k offset pairs.
    let mut cursor = records;
    let mut next = |len: usize| -> Result<Vec<u8>, String> {
        let bytes = reader.bytes(4, cursor, len)?.to_vec();
        cursor += len as u16;
        Ok(bytes)
    };
    let mut points: Vec<(u16, u16)> = Vec::with_capacity(count);
    // The second byte of each group member's offset pair (the scan's carry-in byte).
    let mut group_dy: Vec<Option<u8>> = Vec::with_capacity(count);
    let mut counted = true;
    while points.len() < count {
        let b = next(4)?;
        let (x, y) = (
            u16::from_le_bytes([b[0], b[1]]),
            u16::from_le_bytes([b[2], b[3]]),
        );
        points.push((x, y));
        group_dy.push(None);
        if !counted {
            continue;
        }
        let k = usize::from(next(1)?[0]);
        if k == 0 {
            counted = false;
        }
        for pair in next(2 * k)?.chunks(2) {
            let dx = i16::from(pair[0] as i8);
            let dy = i16::from(pair[1] as i8);
            points.push((x.wrapping_add_signed(dx), y.wrapping_add_signed(dy)));
            group_dy.push(Some(pair[1]));
        }
    }
    if points.len() != count {
        return Err(format!(
            "{} objects decoded, the level has {count}",
            points.len()
        ));
    }
    let list = points
        .into_iter()
        .zip(types)
        .map(|((x, y), &kind)| Placement { x, y, kind })
        .collect();
    Ok((list, group_dy))
}

/// The bytes only the validation mode reads, straight from the user's ROM at run time. They
/// run past the warp tables into the program's own code, so they are never written to the pack.
pub fn validation_input(rom: &Rom) -> Result<ValidationInput, String> {
    let reader = Reader { rom };
    let bytes = reader.bytes(at::WARP_SHIMMER.0, at::WARP_SHIMMER.1, SHIMMER_BYTES)?;
    Ok(ValidationInput {
        warp_shimmer: bytes.into(),
    })
}

/// Placement lists, type tables, hit points, flags and collision sizes (objects.md, behaviour.md
/// 3.6).
pub fn object_data(rom: &Rom) -> Result<ObjectData, String> {
    let reader = Reader { rom };
    let mut lists = Vec::with_capacity(LEVEL_COUNT);
    let mut group_dy = Vec::with_capacity(LEVEL_COUNT);
    for level in 0..LEVEL_COUNT {
        let (list, dy) =
            placements(&reader, level).map_err(|message| format!("level {level}: {message}"))?;
        lists.push(list);
        group_dy.push(dy);
    }
    let frames = reader.bytes(at::TYPE_FRAMES.0, at::TYPE_FRAMES.1, TYPE_COUNT)?;
    let states = reader.bytes(at::TYPE_STATES.0, at::TYPE_STATES.1, TYPE_COUNT)?;
    let table = |at: (u8, u16), len: usize| -> Result<HexRows<16>, String> {
        Ok(HexRows(reader.bytes(at.0, at.1, len)?.to_vec()))
    };
    Ok(ObjectData {
        placements: lists,
        group_dy,
        types: frames.iter().copied().zip(states.iter().copied()).collect(),
        hit_points: table(at::HIT_POINTS, HIT_POINT_ENTRIES)?,
        flags_low: table(at::FLAGS_LOW, FLAG_ENTRIES_LOW)?,
        flags_high: table(at::FLAGS_HIGH, FLAG_ENTRIES_HIGH)?,
        half_width: table(at::HALF_WIDTHS, SIZED_FRAMES)?,
        half_height: table(at::HALF_HEIGHTS, SIZED_FRAMES)?,
        warp_mesh: HexRows(reader.bytes(at::WARP_MESH.0, at::WARP_MESH.1, 65)?.to_vec()),
        demo_waypoints: reader
            .bytes(
                at::DEMO_WAYPOINTS.0,
                at::DEMO_WAYPOINTS.1,
                5 * DEMO_WAYPOINTS,
            )?
            .as_chunks::<5>()
            .0
            .iter()
            .map(|e| {
                (
                    u16::from_le_bytes([e[0], e[1]]),
                    u16::from_le_bytes([e[2], e[3]]),
                    e[4],
                )
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_class_records() {
        // No segments.
        assert_eq!(parse_shape_class(&[0xFF, 0x12]).unwrap(), vec![]);
        // A sloped floor and a wall, as in the spec's record layout (made-up values).
        let record = [
            0x02, 0x08, 0xFF, 0x0E, 0x01, 0x0C, 0x05, 0x08, 0xFF, 0x03, 0xFE, 0x02, 0xFF, 0x77,
        ];
        assert_eq!(
            parse_shape_class(&record).unwrap(),
            vec![
                Segment {
                    direction: 2,
                    line: Line::Floor {
                        y_low: 14,
                        x0: 1,
                        width: 12,
                        y_high: 5
                    }
                },
                Segment {
                    direction: 8,
                    line: Line::Wall {
                        x: 3,
                        y0: -2,
                        height: 2
                    }
                },
            ]
        );
        // A flat ceiling and a sloped ceiling.
        let record = [0x09, 0x0F, 0xFF, 0, 15, 16, 0x0F, 0xFF, 1, 2, 3, 4, 0xFF];
        let segments = parse_shape_class(&record).unwrap();
        assert_eq!(
            segments[1].line,
            Line::Ceiling {
                y_top: 1,
                x0: 2,
                width: 3,
                y_bottom: 4
            }
        );
    }

    #[test]
    fn broken_records_are_rejected() {
        // Wrong separator.
        assert!(parse_shape_class(&[0, 7, 0xFF, 0, 15, 0, 0, 0xFF, 1, 2, 3, 0xFF]).is_err());
        // Missing end marker.
        assert!(parse_shape_class(&[0, 0xFF, 0, 15, 0, 0]).is_err());
        // Direction out of range.
        assert!(parse_shape_class(&[16, 0xFF]).is_err());
        // Too short.
        assert!(parse_shape_class(&[1, 0xFF, 0, 0]).is_err());
    }
}
