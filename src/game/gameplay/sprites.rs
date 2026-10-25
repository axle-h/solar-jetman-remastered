//! The pod's sprite: layouts from the pack's `game/sprites.ron` (`docs/spec/objects.md` 6.2-6.4) drawn
//! with the pod's tiles from the player's ROM.

use crate::game::graphics::{Chr, ChrLoad, Graphics, MissingGraphics, Palette, PatternTable};
use crate::game::hex::HexRows;
use crate::game::playfield::{Frame, PLAYFIELD_LINES, blank_left_columns};
use serde::{Deserialize, Serialize};

/// File name inside the pack's `game/` folder.
pub const SPRITES_FILE: &str = "sprites.ron";

/// Number of pod layouts (table P).
pub const POD_LAYOUTS: usize = 32;

/// One 8 × 8 sprite of a layout: offset from the object point, tile and attribute byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(i8, i8, u8, u8)", into = "(i8, i8, u8, u8)")]
pub struct Piece {
    pub dx: i8,
    pub dy: i8,
    pub tile: u8,
    pub attributes: u8,
}

impl From<(i8, i8, u8, u8)> for Piece {
    fn from((dx, dy, tile, attributes): (i8, i8, u8, u8)) -> Piece {
        Piece {
            dx,
            dy,
            tile,
            attributes,
        }
    }
}

impl From<Piece> for (i8, i8, u8, u8) {
    fn from(piece: Piece) -> (i8, i8, u8, u8) {
        (piece.dx, piece.dy, piece.tile, piece.attributes)
    }
}

/// Layouts of table A (slots 15-30) and B (slots 1-14), objects.md 6.1.
pub const LAYOUTS_A: usize = 193;
pub const LAYOUTS_B: usize = 218;

/// One record of the object sprite sets (objects.md 5.2, graphics.md 6.4): the layout frames
/// `first..end` draw with the tiles `count` tiles from `address` (bank 1) on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(u8, u8, u16, u8)", into = "(u8, u8, u16, u8)")]
pub struct SetRecord {
    pub first: u8,
    pub end: u8,
    pub address: u16,
    pub count: u8,
}

impl From<(u8, u8, u16, u8)> for SetRecord {
    fn from((first, end, address, count): (u8, u8, u16, u8)) -> SetRecord {
        SetRecord {
            first,
            end,
            address,
            count,
        }
    }
}

impl From<SetRecord> for (u8, u8, u16, u8) {
    fn from(record: SetRecord) -> (u8, u8, u16, u8) {
        (record.first, record.end, record.address, record.count)
    }
}

/// Which sprite sets each level loads (graphics.md 6.4): per level the first frames of the
/// records of list A and of list B, and the records themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectSets {
    pub lists_a: Vec<HexRows<16>>,
    pub lists_b: Vec<HexRows<16>>,
    pub records_a: Vec<SetRecord>,
    pub records_b: Vec<SetRecord>,
}

/// `sprites.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteFile {
    /// The pod's layouts P$00-P$1F, each a list of pieces in priority order (first in front).
    pub pod: Vec<Vec<Piece>>,
    /// Layouts A$00-A$C0 (an unused one is empty) and B$00-B$D9.
    pub table_a: Vec<Vec<Piece>>,
    pub table_b: Vec<Vec<Piece>>,
    pub sets: ObjectSets,
}

impl SpriteFile {
    pub fn parse(text: &str) -> Result<SpriteFile, String> {
        let file: SpriteFile = ron::from_str(text).map_err(|error| error.to_string())?;
        if file.pod.len() != POD_LAYOUTS {
            return Err(format!(
                "{} pod layouts, expected {POD_LAYOUTS}",
                file.pod.len()
            ));
        }
        if file.table_a.len() != LAYOUTS_A || file.table_b.len() != LAYOUTS_B {
            return Err(format!(
                "{} and {} layouts in tables A and B, expected {LAYOUTS_A} and {LAYOUTS_B}",
                file.table_a.len(),
                file.table_b.len()
            ));
        }
        Ok(file)
    }

    /// The pieces to draw for the pod's frame RAM $0374 (objects.md 6.4): frames 0-16 use
    /// their layout, 17-31 the layout of 32 - frame mirrored horizontally, $20-$2E (Jetman's
    /// poses) the layout of frame - 15, mirrored when `left` (RAM $0336 bit 0 set and bit 1
    /// clear). Other frames, $FF among them, show nothing.
    pub fn pod_pieces(&self, frame: u8, left: bool) -> Vec<Piece> {
        let frame = usize::from(frame);
        let (layout, mirror) = match frame {
            0..=0x10 => (frame, false),
            0x11..=0x1F => (32 - frame, true),
            0x20..=0x2E => (frame - 0x0F, left),
            _ => return Vec::new(),
        };
        self.pod
            .get(layout)
            .into_iter()
            .flatten()
            .map(|&piece| {
                if mirror {
                    Piece {
                        dx: piece.dx.wrapping_neg(),
                        attributes: piece.attributes ^ 0x40,
                        ..piece
                    }
                } else {
                    piece
                }
            })
            .collect()
    }
}

/// Streams of the asset pack that hold the common sprite tiles and where they load
/// (graphics.md 6.1).
pub const COMMON_SPRITES: [(&str, u16); 9] = [
    ("tiles/sprites/common-0ba0.png", 0x0BA0),
    ("tiles/sprites/common-0bd0.png", 0x0BD0),
    ("tiles/sprites/common-0c80.png", 0x0C80),
    ("tiles/sprites/common-0ca0.png", 0x0CA0),
    ("tiles/sprites/common-0d10.png", 0x0D10),
    ("tiles/sprites/common-0de0.png", 0x0DE0),
    ("tiles/sprites/common-0df0.png", 0x0DF0),
    ("tiles/sprites/common-0f50.png", 0x0F50),
    ("tiles/sprites/common-0f70.png", 0x0F70),
];

/// Where the object sprite sets' tile data lie in the ROM's bank 1 (the address is also the ROM
/// offset), as (first address, length, stream of the asset pack) (graphics.md 6.4, rom-map.md).
const OBJECT_TILE_STREAMS: [(u16, usize, &str); 3] = [
    (0xE045, 0x1480, "tiles/sprites/objects.png"),
    (0xF91E, 0x60, "tiles/sprites/objects-extra-a.png"),
    (0xF97E, 0xF0, "tiles/sprites/objects-extra-b.png"),
];

/// Object sprite sets are copied to the tiles from $41 on (objects.md 5.1).
const FIRST_OBJECT_TILE: usize = 0x41;

/// The final level's sprite tiles at $0000 (graphics.md 5.7.5).
const LEVEL_26_STREAM: &str = "tiles/sprites/level-26.png";

/// Jetman's own figure, 18 tiles (graphics.md 5.7.4; the title's sprites).
const JETMAN_STREAM: &str = "tiles/sprites/title.png";

/// The pod's tiles by pod type (RAM $DC AND 3), graphics.md 6.1.
pub fn pod_stream(pod_type: u8) -> &'static str {
    match pod_type & 3 {
        0 => "tiles/sprites/pod-type-0.png",
        2 => "tiles/sprites/pod-type-2.png",
        _ => "tiles/sprites/pod-type-1-and-3.png",
    }
}

/// The sprite pattern table (PPU $0000) of a level: the pod's tiles, the common sprites and the
/// object sprite sets the level loads, with the frame ranges that say where each set's tiles are
/// (objects.md 5.2).
///
/// A saved state holds only the level and the pod type; loading builds the rest again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "SpritesSource", into = "SpritesSource")]
pub struct Sprites {
    level: usize,
    pod_type: u8,
    table: PatternTable,
    /// The same with Jetman's own figure in tiles $01-$12, which the original writes over the
    /// pod's first 18 tiles from the pod's destruction until a pod exists again (graphics.md
    /// 5.7.4).
    jetman_table: PatternTable,
    /// (first frame, end frame, tile base) for layout tables A and B, in list order.
    ranges_a: Vec<(u8, u8, u8)>,
    ranges_b: Vec<(u8, u8, u8)>,
    /// Quirk Q35 (`docs/deviations.md`): the original's drawer puts every layout drawn without a
    /// vertical flip one pixel further right. Only the validation mode keeps it.
    pub original_rules: bool,
}

/// How [`Sprites`] are saved.
#[derive(serde::Serialize, serde::Deserialize)]
struct SpritesSource {
    level: usize,
    pod_type: u8,
    original_rules: bool,
}

impl From<Sprites> for SpritesSource {
    fn from(sprites: Sprites) -> SpritesSource {
        SpritesSource {
            level: sprites.level,
            pod_type: sprites.pod_type,
            original_rules: sprites.original_rules,
        }
    }
}

impl TryFrom<SpritesSource> for Sprites {
    type Error = String;

    fn try_from(saved: SpritesSource) -> Result<Sprites, String> {
        let assets =
            crate::game::state::assets::<serde::de::value::Error>().map_err(|e| e.to_string())?;
        let mut sprites = Sprites::new(
            &assets.graphics,
            &assets.layouts,
            saved.level,
            saved.pod_type,
        )
        .map_err(|e| e.to_string())?;
        sprites.original_rules = saved.original_rules;
        Ok(sprites)
    }
}

/// The frames of layout tables B (slots 1-14) and A (slots 15-30) stored in the single-sprite
/// form (count 0, `objects.md` 6.2): the original places them exactly, without Q35's +1.
const SINGLE_B: [(u8, u8); 11] = [
    (0x10, 0x12),
    (0x18, 0x37),
    (0x46, 0x46),
    (0x49, 0x49),
    (0x4E, 0x4E),
    (0x51, 0x51),
    (0x62, 0x63),
    (0x6D, 0x6E),
    (0x9F, 0x9F),
    (0, 0),
    (0, 0),
];
const SINGLE_A: [(u8, u8); 11] = [
    (0x33, 0x33),
    (0x37, 0x3E),
    (0x50, 0x50),
    (0x84, 0x84),
    (0x88, 0x8C),
    (0xA2, 0xA2),
    (0xBE, 0xBE),
    (0xC0, 0xC0),
    (0, 0),
    (0, 0),
    (0, 0),
];

/// Whether `frame` of table A (`table_a`) or B is a single-sprite layout (`objects.md` 6.2).
pub fn single_sprite(table_a: bool, frame: u8) -> bool {
    let ranges = if table_a { &SINGLE_A } else { &SINGLE_B };
    ranges
        .iter()
        .any(|&(first, last)| last != 0 && (first..=last).contains(&frame))
}

/// The x of a layout piece's left edge for an object at `x` (`objects.md` 6.2): `dx` already
/// mirrored. The original adds 1 unless the object is drawn flipped vertically (Q35; layouts
/// with entries only, see [`single_sprite`]); the remake never does.
pub fn piece_left(x: i32, dx: i32, flipped: bool, original_rules: bool) -> i32 {
    x + dx + i32::from(original_rules && !flipped)
}

impl Sprites {
    /// The sprite tiles of play on `level` with a pod of `pod_type`. Level 26 loads its own
    /// sprite tiles instead of object sets (graphics.md 5.7.5).
    pub fn new(
        graphics: &Graphics,
        layouts: &SpriteFile,
        level: usize,
        pod_type: u8,
    ) -> Result<Sprites, MissingGraphics> {
        let mut chr = Chr::default();
        let mut loads = vec![ChrLoad {
            stream: pod_stream(pod_type),
            range: None,
            address: 0x0010,
        }];
        for (stream, address) in COMMON_SPRITES {
            loads.push(ChrLoad {
                stream,
                range: None,
                address,
            });
        }
        chr.apply(graphics, &loads)?;
        if level == 26 {
            // The final level's own tiles over the pod set, then a copy of the terrain tiles
            // $27-$5C for the rocks (graphics.md 5.7.5).
            let tile = |n: usize| n * crate::game::graphics::TILE_BYTES;
            chr.apply(
                graphics,
                &[
                    ChrLoad {
                        stream: LEVEL_26_STREAM,
                        range: None,
                        address: 0,
                    },
                    ChrLoad {
                        stream: crate::game::graphics::streams::TERRAIN_TYPES_1_6,
                        range: Some((tile(0x27), tile(0x5D))),
                        address: tile(0x27) as u16,
                    },
                ],
            )?;
        }

        let mut ranges = [Vec::new(), Vec::new()];
        let mut next_tile = FIRST_OBJECT_TILE;
        let sets = &layouts.sets;
        let wanted = [
            (&sets.lists_a, &sets.records_a),
            (&sets.lists_b, &sets.records_b),
        ];
        if level != 26 {
            for (side, (lists, records)) in wanted.into_iter().enumerate() {
                let Some(list) = lists.get(level) else {
                    continue;
                };
                for &first in list.iter() {
                    let record = records
                        .iter()
                        .find(|record| record.first == first)
                        .ok_or_else(|| MissingGraphics(format!("sprite set record {first:#X}")))?;
                    let tiles = usize::from(record.count);
                    let (start, _, stream) = OBJECT_TILE_STREAMS
                        .iter()
                        .copied()
                        .find(|&(start, len, _)| {
                            (start..start + len as u16).contains(&record.address)
                        })
                        .ok_or_else(|| {
                            MissingGraphics(format!("sprite tiles at {:#X}", record.address))
                        })?;
                    let from = usize::from(record.address - start);
                    chr.apply(
                        graphics,
                        &[ChrLoad {
                            stream,
                            range: Some((from, from + tiles * crate::game::graphics::TILE_BYTES)),
                            address: (next_tile * crate::game::graphics::TILE_BYTES) as u16,
                        }],
                    )?;
                    ranges[side].push((record.first, record.end, next_tile as u8));
                    next_tile += tiles;
                }
            }
        }
        if level == 26 {
            // One range of its own: frame $33 at tile base $17 (graphics.md 5.7.5).
            ranges[0].push((0x33, 0x34, 0x17));
        }
        let [ranges_a, ranges_b] = ranges;
        let table = chr.pattern_table(0);
        chr.apply(
            graphics,
            &[ChrLoad {
                stream: JETMAN_STREAM,
                range: None,
                address: 0x0010,
            }],
        )?;
        Ok(Sprites {
            level,
            pod_type,
            table,
            jetman_table: chr.pattern_table(0),
            ranges_a,
            ranges_b,
            original_rules: false,
        })
    }

    /// The tile base of a frame in layout table A or B (objects.md 5.2): that of the first
    /// range holding it, 0 if none does.
    fn tile_base(&self, table_a: bool, frame: u8) -> u8 {
        let ranges = if table_a {
            &self.ranges_a
        } else {
            &self.ranges_b
        };
        ranges
            .iter()
            .find(|&&(first, end, _)| (first..end).contains(&frame))
            .map_or(0, |&(_, _, base)| base)
    }

    /// The sprite pattern table (PPU $0000) of the level.
    /// A copy with `tile` replaced by the 16 bytes `planes` (8 rows of plane 0, then plane 1),
    /// as the original rewrites a sprite tile in video memory while it plays.
    pub fn with_tile(&self, tile: u8, planes: &[u8]) -> Sprites {
        let mut pixels = [0u8; 64];
        for (i, pixel) in pixels.iter_mut().enumerate() {
            let (row, bit) = (i / 8, 7 - i % 8);
            let low = planes.get(row).map_or(0, |b| (b >> bit) & 1);
            let high = planes.get(row + 8).map_or(0, |b| (b >> bit) & 1);
            *pixel = low | high << 1;
        }
        let mut copy = self.clone();
        copy.table.set_tile(tile, pixels);
        copy
    }

    /// A copy whose pod tiles $01-$12 show Jetman's figure (graphics.md 5.7.4).
    pub fn with_jetman(&self) -> Sprites {
        Sprites {
            table: self.jetman_table.clone(),
            ..self.clone()
        }
    }

    pub fn pattern_table(&self) -> &PatternTable {
        &self.table
    }

    /// Draws the pod with its object point at screen position `at` (objects.md 6.2: the NES
    /// shows a sprite one line below its Y), for its frame and horizontal direction `(facing,
    /// left)` (see [`SpriteFile::pod_pieces`]). `(hit_pause, flash)`: the hit pause counter and
    /// the palette bits of RAM $98. While the hit pause runs the pod shows a spark, then blinks,
    /// hidden on one step in four (6.4).
    pub fn draw_pod(
        &self,
        frame: &mut Frame,
        layouts: &SpriteFile,
        palette: &Palette,
        (facing, left): (u8, bool),
        (hit_pause, flash): (u8, u8),
        at: (i32, i32),
    ) {
        if hit_pause != 0 && hit_pause < 0x2C && hit_pause & 3 == 0 {
            return;
        }
        // The hit pause RAM $99 (objects.md 6.4): from $2C up a spark replaces the pod, P$1C on
        // even values and P$1D on odd ones (its palette 1 is in RAM $98's bits, `flash`).
        let pieces = if hit_pause >= 0x2C {
            let spark = 0x1C + usize::from(hit_pause & 1);
            layouts.pod.get(spark).cloned().unwrap_or_default()
        } else {
            layouts.pod_pieces(facing, left)
        };
        for piece in pieces.iter().rev() {
            // RAM $98 is ORed into the pod's sprite attributes (game-flow.md 9.2).
            let piece = Piece {
                attributes: piece.attributes | flash,
                ..*piece
            };
            self.draw_piece(frame, palette, &piece, at, 0);
        }
    }

    /// Draws an object of slot `slot` (1-30) with its object point at screen position `at`
    /// (objects.md 5-6): table B for slots 1-14, A for 15-30, mirrored by its direction flags
    /// (6.3).
    pub fn draw_object(
        &self,
        frame: &mut Frame,
        layouts: &SpriteFile,
        palette: &Palette,
        slot: usize,
        object: &super::objects::Object,
        at: (i32, i32),
    ) {
        let table_a = slot >= 15;
        let table = if table_a {
            &layouts.table_a
        } else {
            &layouts.table_b
        };
        let Some(pieces) = table.get(usize::from(object.frame)) else {
            return;
        };
        let base = self.tile_base(table_a, object.frame);
        // The attribute mask and the mirroring of the picture (objects.md 6.3).
        let (fx, fy) = (object.flags_x, object.flags_y);
        let mut mask = 0u8;
        if fx & 1 != 0 {
            mask |= (fx >> 1) & 0x7E;
            if fx & 2 == 0 {
                mask |= 0x40;
            }
        }
        mask |= fy & 0x80;
        let mirrored = mask & 0x40 != 0;
        let flipped = fy & 0x80 != 0;
        let placed: Vec<Piece> = pieces
            .iter()
            .map(|piece| Piece {
                dx: if mirrored {
                    piece.dx.wrapping_neg()
                } else {
                    piece.dx
                },
                dy: if flipped {
                    piece.dy.wrapping_neg()
                } else {
                    piece.dy
                },
                attributes: piece.attributes ^ mask,
                ..*piece
            })
            .collect();
        // The first entry is in front, so draw from the last; a flipped picture also has its
        // entries in reverse order.
        let order: Box<dyn Iterator<Item = &Piece>> = if flipped {
            Box::new(placed.iter())
        } else {
            Box::new(placed.iter().rev())
        };
        let stray = self.original_rules && !flipped && !single_sprite(table_a, object.frame);
        let at = (at.0 + i32::from(stray), at.1);
        for piece in order {
            self.draw_piece(frame, palette, piece, at, base);
        }
    }

    fn draw_piece(
        &self,
        frame: &mut Frame,
        palette: &Palette,
        piece: &Piece,
        at: (i32, i32),
        base: u8,
    ) {
        let left = at.0 + i32::from(piece.dx);
        let top = at.1 + i32::from(piece.dy) + 1;
        draw_tile(
            frame,
            palette,
            &self.table,
            piece.tile.wrapping_add(base),
            piece.attributes,
            (left, top),
        );
    }
}

/// Draws one 8 × 8 sprite tile of `table` with its top-left pixel at `(left, top)` on the
/// playfield: `attributes` are the sprite attribute byte (palette 0-3, bit 5 behind the
/// background, bits 6 and 7 the flips). Nothing shows in the blanked left columns
/// ([`blank_left_columns`]) or below the playfield.
pub fn draw_tile(
    frame: &mut Frame,
    palette: &Palette,
    table: &PatternTable,
    tile: u8,
    attributes: u8,
    (left, top): (i32, i32),
) {
    let behind = attributes & 0x20 != 0;
    let width = frame.width;
    let columns = blank_left_columns(width) as i32..width as i32;
    for row in 0..8i32 {
        let y = top + row;
        if !(0..PLAYFIELD_LINES as i32).contains(&y) {
            continue;
        }
        let source_row = if attributes & 0x80 != 0 { 7 - row } else { row } as u32;
        let pixels = table.row(tile, source_row);
        for column in 0..8i32 {
            let x = left + column;
            if !columns.contains(&x) {
                continue;
            }
            let source_column = if attributes & 0x40 != 0 {
                7 - column
            } else {
                column
            };
            let value = pixels[source_column as usize];
            if value == 0 {
                continue;
            }
            let at_pixel = y as usize * width + x as usize;
            // Behind the background: only over the universal background colour.
            if behind && frame.pixels[at_pixel] != palette.universal_background() {
                continue;
            }
            let entry = 16 + usize::from(attributes & 3) * 4 + usize::from(value);
            frame.pixels[at_pixel] = palette.0[entry];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::piece_left;

    #[test]
    fn the_drawer_offset_is_a_validation_mode_rule() {
        // Q35: the original puts an unflipped layout 1 px right; the remake never does.
        assert_eq!(piece_left(100, -4, false, true), 97);
        assert_eq!(piece_left(100, -4, true, true), 96);
        assert_eq!(piece_left(100, -4, false, false), 96);
        assert_eq!(piece_left(100, -4, true, false), 96);
        assert!(super::single_sprite(false, 0x19) && super::single_sprite(true, 0x33));
        assert!(!super::single_sprite(false, 0x13) && !super::single_sprite(true, 0x36));
        assert!(!super::single_sprite(true, 0));
    }
}
