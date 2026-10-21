//! Level data (`docs/spec/levels.md`): per-level settings and object placement, planet maps
//! with their palette maps, the block sets and chunks maps are built of, and the terrain
//! collision shapes. The import extracts them from the player's ROM into the pack's `game/`
//! folder ([`crate::rom::extract`]); no colours or tile pixels are in them.

use crate::game::gameplay::objdata::{OBJECTS_FILE, ObjectData};
use crate::game::hex::{DigitGrid, HexGrid, HexRows};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Number of levels (RAM $3B, 0-41).
pub const LEVEL_COUNT: usize = 42;
/// The planets are levels 0-12.
pub const PLANETS: usize = 13;
/// Warp levels are 29-41.
pub const FIRST_WARP_LEVEL: usize = 29;

/// Pixels per tile, block and chunk side.
pub const TILE_SIZE: u32 = 8;
pub const BLOCK_SIZE: u32 = 16;
pub const CHUNK_SIZE: u32 = 128;
/// Blocks per chunk side.
pub const CHUNK_BLOCKS: usize = 8;

/// Number of block sets, blocks per set and chunk definitions.
pub const BLOCK_SETS: usize = 3;
pub const BLOCKS_PER_SET: usize = 170;
pub const CHUNKS: usize = 246;
/// Number of collision shape classes.
pub const SHAPE_CLASSES: usize = 53;

/// File names inside the pack's `game/` folder.
pub const LEVELS_FILE: &str = "levels.ron";
pub const BLOCKS_FILE: &str = "blocks.ron";
pub const COLLISION_FILE: &str = "collision.ron";
/// Maps are `maps/<name>.ron`, named by the level header's `map` field.
pub const MAPS_DIR: &str = "maps";

/// A position in world pixels, (0, 0) at the map's top-left corner. Written `(x, y)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "(u16, u16)", into = "(u16, u16)")]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

impl From<(u16, u16)> for Point {
    fn from((x, y): (u16, u16)) -> Point {
        Point { x, y }
    }
}

impl From<Point> for (u16, u16) {
    fn from(point: Point) -> (u16, u16) {
        (point.x, point.y)
    }
}

/// `levels.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelsFile {
    /// Graphics type (0-6) → planet-surface picture number; the block set is that number AND 3
    /// (levels.md 4.1).
    pub surface_picture_by_graphics_type: HexRows<8>,
    pub levels: Vec<Level>,
}

/// One level's header, without its colours (levels.md 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    /// The level index, 0-41.
    pub number: u8,
    /// Camera position at level setup (bytes 0-3).
    pub camera_start: Point,
    /// Home position (bytes 4-7): where the player starts, and on planets where the
    /// mothership stands.
    pub home: Point,
    /// Map width in chunks (byte 8 + 1).
    pub width: u8,
    /// Map height in chunks as the header gives it (byte 9 + 1); the map has one more row.
    pub height: u8,
    /// Object activity margin in pixels (byte 10).
    pub object_margin: u8,
    /// Graphics type (byte 11, bits 0-2).
    pub graphics_type: u8,
    /// Gravity (byte 11 with bits 0-2 cleared, signed).
    pub gravity: i8,
    /// Block set 0-2, from the graphics type through the surface picture number.
    pub block_set: u8,
    /// Name of the map file in `maps/`.
    pub map: String,
    /// Object list (levels.md 3.3), in list order.
    pub objects: Vec<Placement>,
}

/// One entry of a level's object list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub x: u16,
    pub y: u16,
    /// Object type (the objects spec gives their meaning).
    pub kind: u8,
}

impl Level {
    /// Rows of chunks in the map: one more than the header's height (levels.md 4.2).
    pub fn map_rows(&self) -> usize {
        usize::from(self.height) + 1
    }

    /// The largest camera position (levels.md 5.2): x up to the map width less one screen,
    /// y up to `256 × (byte 9 >> 1) + 56`.
    pub fn camera_limit(&self) -> Point {
        let byte8 = u16::from(self.width.saturating_sub(1));
        let byte9 = u16::from(self.height.saturating_sub(1));
        Point {
            x: 256 * (byte8 >> 1),
            y: 256 * (byte9 >> 1) + 56,
        }
    }

    /// `camera` held within the camera limits.
    pub fn clamp_camera(&self, camera: Point) -> Point {
        let limit = self.camera_limit();
        Point {
            x: camera.x.min(limit.x),
            y: camera.y.min(limit.y),
        }
    }

    pub fn is_warp_level(&self) -> bool {
        usize::from(self.number) >= FIRST_WARP_LEVEL
    }
}

/// `maps/<name>.ron`: a grid of chunk numbers and the background palette of each chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMap {
    /// Chunk numbers, one row of the map per string.
    pub chunks: HexGrid,
    /// Background palette (0-3) of each chunk, same shape as `chunks` (levels.md 4.5).
    pub palettes: DigitGrid,
}

/// `blocks.ron`: the three block sets and the chunk definitions shared by all levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocks {
    /// Per set, one row per block: top-left, top-right, bottom-left, bottom-right tile.
    pub block_sets: Vec<HexRows<4>>,
    /// Per chunk, 8 rows of 8 block numbers, top row first.
    pub chunks: Vec<HexRows<8>>,
}

/// `collision.ron`: terrain shapes (levels.md 6.1-6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collision {
    /// Per block set, the shape class of each block.
    pub block_classes: Vec<HexRows<17>>,
    /// The shape classes, by class number.
    pub classes: Vec<ShapeClass>,
}

/// A shape class: the surfaces of a block, in block-local pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeClass {
    pub segments: Vec<Segment>,
}

/// One surface of a shape class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// 0-15: which kind of floor, wall or ceiling, and which side is solid (levels.md 6.2).
    pub direction: u8,
    pub line: Line,
}

/// Where a segment lies, in the fields the spec gives for its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Line {
    /// Directions 0 (floor) and 9 (ceiling): y from x0 to x0 + width.
    Flat { x0: i8, width: i8, y: i8 },
    /// Directions 7 and 8: x from y0 to y0 + height.
    Wall { x: i8, y0: i8, height: i8 },
    /// Directions 1-6: a sloped floor; `y_low` is the larger y, `y_high` the smaller.
    Floor {
        y_low: i8,
        x0: i8,
        width: i8,
        y_high: i8,
    },
    /// Directions 10-15: a sloped ceiling.
    Ceiling {
        y_top: i8,
        x0: i8,
        width: i8,
        y_bottom: i8,
    },
}

impl Line {
    /// Whether a segment of `direction` is stored in this layout.
    pub fn fits(&self, direction: u8) -> bool {
        match self {
            Line::Flat { .. } => matches!(direction, 0 | 9),
            Line::Wall { .. } => matches!(direction, 7 | 8),
            Line::Floor { .. } => (1..=6).contains(&direction),
            Line::Ceiling { .. } => (10..=15).contains(&direction),
        }
    }
}

/// A data file that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataError {
    pub file: String,
    pub message: String,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

impl std::error::Error for DataError {}

fn error(file: &str, message: impl Into<String>) -> DataError {
    DataError {
        file: file.to_owned(),
        message: message.into(),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(file: &str, text: &str) -> Result<T, DataError> {
    ron::from_str(text).map_err(|e| error(file, e.to_string()))
}

/// Writes a level data file as RON: nested structures on their own lines,
/// small ones (positions, objects, segments) on one line.
pub fn to_ron<T: Serialize>(value: &T) -> Result<String, ron::Error> {
    let config = ron::ser::PrettyConfig::new()
        .indentor("  ".to_owned())
        .struct_names(true)
        .depth_limit(4);
    let mut text = ron::ser::to_string_pretty(value, config)?;
    text.push('\n');
    Ok(text)
}

/// All level data, checked for consistency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub surface_picture_by_graphics_type: HexRows<8>,
    pub levels: Vec<Level>,
    pub maps: BTreeMap<String, ChunkMap>,
    pub blocks: Blocks,
    pub collision: Collision,
    pub objects: ObjectData,
}

impl World {
    /// Reads the data files: `levels.ron`, `blocks.ron`, `collision.ron` and the maps, each
    /// given as (name without `maps/` and `.ron`, text).
    pub fn parse<'a>(
        levels: &str,
        blocks: &str,
        collision: &str,
        objects: &str,
        maps: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<World, DataError> {
        let levels: LevelsFile = parse(LEVELS_FILE, levels)?;
        let blocks: Blocks = parse(BLOCKS_FILE, blocks)?;
        let collision: Collision = parse(COLLISION_FILE, collision)?;
        let objects = ObjectData::parse(objects).map_err(|message| error(OBJECTS_FILE, message))?;
        let mut parsed = BTreeMap::new();
        for (name, text) in maps {
            let file = format!("{MAPS_DIR}/{name}.ron");
            parsed.insert(name.to_owned(), parse::<ChunkMap>(&file, text)?);
        }
        let world = World {
            surface_picture_by_graphics_type: levels.surface_picture_by_graphics_type,
            levels: levels.levels,
            maps: parsed,
            blocks,
            collision,
            objects,
        };
        world.check()?;
        Ok(world)
    }

    /// The data files of this world, by path inside the pack's `game/` folder.
    pub fn to_files(&self) -> Result<BTreeMap<String, String>, ron::Error> {
        let mut files = BTreeMap::new();
        let levels = LevelsFile {
            surface_picture_by_graphics_type: self.surface_picture_by_graphics_type.clone(),
            levels: self.levels.clone(),
        };
        files.insert(LEVELS_FILE.to_owned(), to_ron(&levels)?);
        files.insert(BLOCKS_FILE.to_owned(), to_ron(&self.blocks)?);
        files.insert(COLLISION_FILE.to_owned(), to_ron(&self.collision)?);
        files.insert(OBJECTS_FILE.to_owned(), to_ron(&self.objects)?);
        for (name, map) in &self.maps {
            files.insert(format!("{MAPS_DIR}/{name}.ron"), to_ron(map)?);
        }
        Ok(files)
    }

    fn check(&self) -> Result<(), DataError> {
        if self.levels.len() != LEVEL_COUNT {
            return Err(error(
                LEVELS_FILE,
                format!("{} levels, expected {LEVEL_COUNT}", self.levels.len()),
            ));
        }
        for (index, level) in self.levels.iter().enumerate() {
            let fail = |message: String| error(LEVELS_FILE, format!("level {index}: {message}"));
            if usize::from(level.number) != index {
                return Err(fail(format!("numbered {}", level.number)));
            }
            if usize::from(level.block_set) >= BLOCK_SETS {
                return Err(fail(format!("block set {}", level.block_set)));
            }
            if level.width == 0 || level.height == 0 {
                return Err(fail("empty map".to_owned()));
            }
            let Some(map) = self.maps.get(&level.map) else {
                return Err(fail(format!("no map `{}`", level.map)));
            };
            let file = format!("{MAPS_DIR}/{}.ron", level.map);
            let shape = (usize::from(level.width), level.map_rows());
            if (map.chunks.width(), map.chunks.height()) != shape {
                return Err(error(
                    &file,
                    format!(
                        "{}×{} chunks, level {index} needs {}×{}",
                        map.chunks.width(),
                        map.chunks.height(),
                        shape.0,
                        shape.1
                    ),
                ));
            }
            if (map.palettes.width(), map.palettes.height()) != shape {
                return Err(error(&file, "palette grid differs from the chunk grid"));
            }
            if map.palettes.cells().iter().any(|&p| p > 3) {
                return Err(error(&file, "palettes are 0-3"));
            }
        }
        if self.blocks.block_sets.len() != BLOCK_SETS
            || self
                .blocks
                .block_sets
                .iter()
                .any(|set| set.len() != 4 * BLOCKS_PER_SET)
        {
            return Err(error(
                BLOCKS_FILE,
                format!("expected {BLOCK_SETS} sets of {BLOCKS_PER_SET} blocks"),
            ));
        }
        if self.blocks.chunks.len() != CHUNKS
            || self.blocks.chunks.iter().any(|chunk| chunk.len() != 64)
        {
            return Err(error(
                BLOCKS_FILE,
                format!("expected {CHUNKS} chunks of 8 × 8 blocks"),
            ));
        }
        if self.collision.block_classes.len() != BLOCK_SETS
            || self
                .collision
                .block_classes
                .iter()
                .any(|classes| classes.len() != BLOCKS_PER_SET)
        {
            return Err(error(
                COLLISION_FILE,
                format!("expected {BLOCK_SETS} block → class maps of {BLOCKS_PER_SET} blocks"),
            ));
        }
        if self.collision.classes.len() != SHAPE_CLASSES {
            return Err(error(
                COLLISION_FILE,
                format!("expected {SHAPE_CLASSES} shape classes"),
            ));
        }
        for (number, class) in self.collision.classes.iter().enumerate() {
            for segment in &class.segments {
                if !segment.line.fits(segment.direction) {
                    return Err(error(
                        COLLISION_FILE,
                        format!(
                            "class {number}: direction {} cannot have {:?}",
                            segment.direction, segment.line
                        ),
                    ));
                }
            }
        }
        if let Some(&class) = self
            .collision
            .block_classes
            .iter()
            .flat_map(|classes| classes.iter())
            .find(|&&class| usize::from(class) >= SHAPE_CLASSES)
        {
            return Err(error(
                COLLISION_FILE,
                format!("a block has shape class {class}"),
            ));
        }
        Ok(())
    }

    /// The map of a level.
    pub fn map(&self, level: &Level) -> &ChunkMap {
        &self.maps[&level.map]
    }

    /// The background tile and palette at tile coordinates (`tx`, `ty`) of a level
    /// (levels.md 4.3). Outside the map, and for chunk or block numbers past the end of the
    /// data, the answer is tile 0 (empty sky) with palette 0.
    pub fn tile_at(&self, level: &Level, tx: u32, ty: u32) -> MapTile {
        let map = self.map(level);
        let cx = (tx / 16) as usize;
        let cy = (ty / 16) as usize;
        let (Some(chunk), Some(palette)) = (map.chunks.get(cx, cy), map.palettes.get(cx, cy))
        else {
            return MapTile::default();
        };
        let block_index = ((ty / 2) % 8 * 8 + (tx / 2) % 8) as usize;
        let block = self
            .blocks
            .chunks
            .get(usize::from(chunk))
            .and_then(|chunk| chunk.get(block_index).copied());
        let corner = ((ty % 2) * 2 + tx % 2) as usize;
        let tile = block.and_then(|block| {
            self.blocks.block_sets[usize::from(level.block_set)]
                .get(usize::from(block) * 4 + corner)
                .copied()
        });
        MapTile {
            tile: tile.unwrap_or(0),
            palette,
        }
    }
}

/// A background tile of the map and the palette it is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapTile {
    /// Tile number in the background pattern table.
    pub tile: u8,
    /// Background palette 0-3.
    pub palette: u8,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small world: every level uses one 2 × 3 map (header height 2) of chunks 0 and 1;
    /// chunk 1 is block 1 everywhere except its top-left block (block 2); block n of set s is
    /// tiles 4n + 16s .. 4n + 16s + 3.
    pub(crate) fn synthetic_world() -> World {
        let levels = (0..LEVEL_COUNT as u8)
            .map(|number| Level {
                number,
                camera_start: Point { x: 0, y: 8 },
                home: Point { x: 100, y: 120 },
                width: 2,
                height: 2,
                object_margin: 64,
                graphics_type: 1,
                gravity: -8,
                block_set: number % 3,
                map: "small".to_owned(),
                objects: vec![Placement {
                    x: 10,
                    y: 20,
                    kind: 21,
                }],
            })
            .collect();
        let chunks = HexGrid::new(2, vec![0, 1, 1, 0, 1, 1]);
        let palettes = DigitGrid::new(2, vec![0, 2, 3, 0, 1, 3]);
        let block_sets = (0..BLOCK_SETS)
            .map(|set| {
                HexRows(
                    (0..4 * BLOCKS_PER_SET)
                        .map(|i| (i + 16 * set) as u8)
                        .collect(),
                )
            })
            .collect();
        let mut chunk_defs = vec![HexRows(vec![0u8; 64]); CHUNKS];
        let mut one = vec![1u8; 64];
        one[0] = 2;
        chunk_defs[1] = HexRows(one);
        let classes = (0..SHAPE_CLASSES)
            .map(|class| ShapeClass {
                segments: if class == 1 {
                    vec![Segment {
                        direction: 0,
                        line: Line::Flat {
                            x0: 0,
                            width: 15,
                            y: 0,
                        },
                    }]
                } else {
                    Vec::new()
                },
            })
            .collect();
        World {
            surface_picture_by_graphics_type: HexRows(vec![1, 2, 0, 4, 5, 0, 6]),
            levels,
            maps: BTreeMap::from([("small".to_owned(), ChunkMap { chunks, palettes })]),
            blocks: Blocks {
                block_sets,
                chunks: chunk_defs,
            },
            collision: Collision {
                block_classes: vec![HexRows(vec![1; BLOCKS_PER_SET]); BLOCK_SETS],
                classes,
            },
            objects: crate::game::gameplay::objdata::tests::synthetic(),
        }
    }

    #[test]
    fn files_round_trip() {
        let world = synthetic_world();
        let files = world.to_files().unwrap();
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            [
                "blocks.ron",
                "collision.ron",
                "levels.ron",
                "maps/small.ron",
                "objects.ron"
            ]
        );
        let back = World::parse(
            &files["levels.ron"],
            &files["blocks.ron"],
            &files["collision.ron"],
            &files["objects.ron"],
            [("small", files["maps/small.ron"].as_str())],
        )
        .unwrap();
        assert_eq!(back, world);
        assert!(
            files["levels.ron"].contains("camera_start: (0, 8),"),
            "{}",
            files["levels.ron"]
        );
        assert!(files["maps/small.ron"].contains("\"00 01\""));
        assert!(files["maps/small.ron"].contains("\"02\""));
        assert!(
            files["collision.ron"].contains("line: Flat(x0: 0, width: 15, y: 0)"),
            "{}",
            files["collision.ron"]
        );
    }

    #[test]
    fn inconsistent_data_is_rejected() {
        let world = synthetic_world();
        let files = world.to_files().unwrap();
        let parse = |levels: &str, map: &str| {
            World::parse(
                levels,
                &files["blocks.ron"],
                &files["collision.ron"],
                &files["objects.ron"],
                [("small", map)],
            )
        };
        let wrong_height = files["levels.ron"].replace("height: 2", "height: 3");
        let error = parse(&wrong_height, &files["maps/small.ron"]).unwrap_err();
        assert_eq!(error.file, "maps/small.ron", "{error}");
        let missing = files["levels.ron"].replacen("map: \"small\"", "map: \"big\"", 1);
        assert!(parse(&missing, &files["maps/small.ron"]).is_err());
        let bad_palette = files["maps/small.ron"].replace("\"13\"", "\"14\"");
        assert!(parse(&files["levels.ron"], &bad_palette).is_err());
    }

    #[test]
    fn tiles_decode_through_chunks_and_blocks() {
        let world = synthetic_world();
        let level = &world.levels[0]; // block set 0
        // Chunk (0, 0) is chunk 0: block 0, tile 0, palette 0.
        assert_eq!(world.tile_at(level, 2, 4), MapTile::default());
        assert_eq!(world.tile_at(level, 3, 5).tile, 3);
        // Chunk (1, 0) is chunk 1; its top-left block is block 2, tiles 8-11, palette 2.
        let at = |tx, ty| world.tile_at(level, tx, ty);
        assert_eq!(
            at(16, 0),
            MapTile {
                tile: 8,
                palette: 2
            }
        );
        assert_eq!(at(17, 0).tile, 9);
        assert_eq!(at(16, 1).tile, 10);
        assert_eq!(at(17, 1).tile, 11);
        // Other blocks of chunk 1 are block 1: tiles 4-7.
        assert_eq!(at(18, 0).tile, 4);
        assert_eq!(at(31, 15).tile, 7);
        // Chunk row 2 (the row beyond the header's height) is part of the map.
        assert_eq!(
            at(0, 32),
            MapTile {
                tile: 8,
                palette: 1
            }
        );
        // Outside the map: sky.
        assert_eq!(at(32, 0), MapTile::default());
        assert_eq!(at(0, 48), MapTile::default());
        // Block set 1 shifts tile numbers by 16.
        assert_eq!(world.tile_at(&world.levels[1], 16, 0).tile, 24);
    }

    #[test]
    fn camera_limits_follow_the_header() {
        let mut level = synthetic_world().levels[0].clone();
        // Level 0 of the original: 8 × 8 chunks → bytes 7, 7.
        level.width = 8;
        level.height = 8;
        assert_eq!(level.camera_limit(), Point { x: 768, y: 824 });
        // Byte 9 even (height 9): the extra chunk row can be seen.
        level.height = 9;
        assert_eq!(level.camera_limit().y, 1080);
        assert_eq!(
            level.clamp_camera(Point { x: 2000, y: 10 }),
            Point { x: 768, y: 10 }
        );
        assert_eq!(level.map_rows(), 10);
    }
}
