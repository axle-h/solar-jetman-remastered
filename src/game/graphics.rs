//! The imported graphics (tile streams and colours from the player's ROM) and how a level's
//! background pattern table and palette are put together from them (`docs/spec/graphics.md`,
//! sections 5.7 and 7).

use crate::game::level::{FIRST_WARP_LEVEL, Level};
use std::collections::BTreeMap;
use std::fmt;

/// Bytes per 8 × 8 tile (2 bits per pixel, two planes).
pub const TILE_BYTES: usize = 16;
/// Size of CHR memory: two pattern tables of 256 tiles.
pub const CHR_SIZE: usize = 0x2000;

/// Tile streams of the asset pack used by gameplay backgrounds (graphics.md 4.1, 4.2, 4.5).
pub mod streams {
    pub const STATUS_PANEL: &str = "tiles/hud/status-panel.png";
    pub const STATUS_PANEL_1D60: &str = "tiles/hud/status-panel-1d60.png";
    pub const TERRAIN_TYPES_0_4: &str = "tiles/terrain/types-0-4.png";
    pub const TERRAIN_TYPES_1_6: &str = "tiles/terrain/types-1-6.png";
    pub const TERRAIN_TYPES_2_3_5: &str = "tiles/terrain/types-2-3-5.png";
    pub const TERRAIN_COMMON: &str = "tiles/terrain/common.png";
    pub const TYPE_3_EXTRA_12A0: &str = "tiles/terrain/type-3-extra-12a0.png";
    pub const TYPE_3_EXTRA_1820: &str = "tiles/terrain/type-3-extra-1820.png";
    pub const TYPE_5_EXTRA_15C0: &str = "tiles/terrain/type-5-extra-15c0.png";
    pub const TYPE_5_EXTRA_1B00: &str = "tiles/terrain/type-5-extra-1b00.png";
    pub const LEVELS_1_8_EXTRA: &str = "tiles/terrain/levels-1-8-extra.png";
    pub const WARP_BACKGROUNDS: &str = "tiles/warp/backgrounds.png";
    /// The item pictures of the shop (9 tiles each), which are also the panel's weapon icons.
    pub const ITEM_PICTURES: &str = "tiles/frames-screen/squares.png";
    /// The tractor beam's nine images of sprite tile $CF (player.md 8.4).
    pub const BEAM: &str = "tiles/sprites/beam.png";
    /// The bonus sign's nine points pictures, two tiles each (graphics.md 3.1).
    pub const SIGNS: &str = "tiles/hud/digits.png";
}

/// Tiles per warp-level background set.
const WARP_SET_TILES: usize = 17;

/// Palette presets used by gameplay (graphics.md 7.2): background palettes 0-1, sprite
/// palettes 0-1.
const PRESET_GAMEPLAY_BACKGROUND: usize = 11;
const PRESET_GAMEPLAY_SPRITES: usize = 10;

/// The colour every scene of the game uses as the universal background (black).
const BLACK: u8 = 0x0F;

/// Graphics imported from the player's ROM, as the game needs them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Graphics {
    /// Decoded tile streams by asset path, each exactly as long as the original writes it
    /// (it may end part-way into a tile).
    pub tile_streams: BTreeMap<String, Vec<u8>>,
    /// Palette presets by number.
    pub presets: Vec<PalettePreset>,
    /// Gameplay colours of each level: header bytes 12-23.
    pub level_colours: Vec<Vec<u8>>,
    /// The globe's 3 colours on the approach screen and the 12 colours of the surface picture,
    /// per level (graphics.md 7.3); `None` beyond the 14 planets or in a pack without them.
    pub approach_globe: Vec<Option<Vec<u8>>>,
    pub surface_colours: Vec<Option<Vec<u8>>>,
    /// The 8 byte values the status panel's partly filled gauge cells are made of
    /// (graphics.md 3); empty when the pack lacks them.
    pub gauge_rows: Vec<u8>,
    /// Nametable pictures by asset path (`pictures/title.ron`, ...): writes in order.
    pub pictures: BTreeMap<String, Vec<PictureWrite>>,
    /// The nametable parts to which the original adds a tile base chosen at run time (menu
    /// screen A, graphics.md 5.9), by asset path, as stored (without the base).
    pub based_pictures: BTreeMap<String, Vec<PictureWrite>>,
    /// The two sets of 15 colours of the screen of graphics.md 5.10 (shop and equip list), for
    /// palette entries 1-15 (byte 0 is not used); empty when the pack lacks them.
    pub frames_screen: Vec<Vec<u8>>,
    /// The title's logo fade-in (graphics.md 7.4): six columns of seven colours.
    pub title_fade: Vec<Vec<u8>>,
    /// The three colours entries 9-11 take after the last step.
    pub title_fade_end: Vec<u8>,
    /// The backdrop interlude's tables and colours (scenes.md 11.1, graphics.md 7.4); empty when
    /// the pack lacks them.
    pub backdrop: Backdrop,
    /// The analysis screen's colours for entries 9 and 10 by analysis list (graphics.md 7.4).
    pub analysis_pairs: Vec<Vec<u8>>,
    /// The sun flicker's white (scenes.md 2.1).
    pub title_white: u8,
    /// The flight's lit-window colours (12, scenes.md 5.1) and the ring's sine table (128,
    /// scenes.md 5.4a); empty when the pack lacks them.
    pub flight_window: Vec<u8>,
    pub flight_sine: Vec<u8>,
    /// The attract story's picture frames (graphics.md 4.5): 11 frames of 11 rows of 16 tile
    /// numbers, and 12 attribute bytes per frame; empty when the pack lacks them.
    pub attract_nametable: Vec<u8>,
    pub attract_attributes: Vec<u8>,
}

/// What the backdrop interlude is built from.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Backdrop {
    /// Colours for palette entries 16-31, and the three 4-colour sets for entries 12-15.
    pub sprite_colours: Vec<u8>,
    pub variants: Vec<Vec<u8>>,
    /// The 15 bars' widths and first columns.
    pub bar_widths: Vec<u8>,
    pub bar_columns: Vec<u8>,
    /// Four phases of 32 bytes; the tile is the byte plus 1.
    pub phase_tiles: Vec<u8>,
    /// x and type of the objects that spawn, 32 per backdrop and one more.
    pub spawn_x: Vec<u8>,
    pub spawn_types: Vec<u8>,
}

/// Bytes written to the nametable from a PPU address ($2000-$23FF; $23C0 and up are attributes).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PictureWrite {
    pub address: u16,
    pub bytes: Vec<u8>,
}

/// Colours a preset writes into the palette copy from `first_entry` on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PalettePreset {
    pub first_entry: u8,
    pub colours: Vec<u8>,
}

/// Something the level needs is not in the imported graphics.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MissingGraphics(pub String);

impl fmt::Display for MissingGraphics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "imported graphics lack {}", self.0)
    }
}

impl std::error::Error for MissingGraphics {}

/// One load into CHR memory: bytes of a stream written from a PPU address.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChrLoad {
    pub stream: &'static str,
    /// Byte range of the stream; `None` for all of it.
    pub range: Option<(usize, usize)>,
    pub address: u16,
}

const fn load(stream: &'static str, address: u16) -> ChrLoad {
    ChrLoad {
        stream,
        range: None,
        address,
    }
}

/// The background loads at the start of play of a level, in order (graphics.md 5.7 steps 2-5;
/// the other steps load sprites).
pub fn gameplay_background_loads(level: &Level) -> Vec<ChrLoad> {
    use streams::*;
    let mut loads = vec![load(STATUS_PANEL, 0x1E40), load(STATUS_PANEL_1D60, 0x1D60)];
    let number = usize::from(level.number);
    if number >= FIRST_WARP_LEVEL {
        // Set = level - 29, except level 41, which uses set 10 (graphics.md 4.1).
        let set = if number == 41 {
            10
        } else {
            number - FIRST_WARP_LEVEL
        };
        let bytes = WARP_SET_TILES * TILE_BYTES;
        loads.push(ChrLoad {
            stream: WARP_BACKGROUNDS,
            range: Some((set * bytes, (set + 1) * bytes)),
            address: 0x1810,
        });
        return loads;
    }
    let terrain = match level.graphics_type {
        0 | 4 => TERRAIN_TYPES_0_4,
        1 | 6 => TERRAIN_TYPES_1_6,
        _ => TERRAIN_TYPES_2_3_5,
    };
    loads.push(load(terrain, 0x1000));
    // Level 26 writes the panel stream a second time instead of the common background, so
    // its life icon is the panel's drawing (graphics.md 5.7 item 4, 5.7.6).
    if number == 26 {
        loads.push(load(STATUS_PANEL, 0x1E40));
    } else {
        loads.push(load(TERRAIN_COMMON, 0x1CC0));
    }
    if number == 1 || number == 8 {
        loads.push(load(LEVELS_1_8_EXTRA, 0x12D0));
    } else if level.graphics_type == 3 {
        loads.push(load(TYPE_3_EXTRA_12A0, 0x12A0));
        loads.push(load(TYPE_3_EXTRA_1820, 0x1820));
    } else if level.graphics_type == 5 {
        loads.push(load(TYPE_5_EXTRA_15C0, 0x15C0));
        loads.push(load(TYPE_5_EXTRA_1B00, 0x1B00));
    }
    loads
}

/// The first of the gameplay's three panel icon tiles: the jetpod's top and bottom, Jetman.
pub const PANEL_ICON_TILES: u8 = 0xF4;

/// One change made to CHR memory, kept so that a saved state can say how the memory was made
/// instead of holding the imported tile bytes.
#[derive(Debug, Clone)]
enum ChrOp {
    /// Bytes of a tile stream (`len` of them were written).
    Load { load: ChrLoad, len: usize },
    /// Bytes the game itself makes up.
    Bytes { address: u16, bytes: Vec<u8> },
    /// A gauge piece: 16 bytes of the value at `index` of the imported gauge rows (0 when
    /// there is none).
    Gauge { address: u16, index: usize },
    /// `len` bytes of the CHR memory the `loads` make (from `from` on), reversed bit by bit
    /// when `mirror`.
    Derived {
        loads: Vec<ChrLoad>,
        from: usize,
        len: usize,
        mirror: bool,
        address: u16,
    },
}

impl ChrOp {
    /// The CHR bytes the change covers.
    fn span(&self) -> std::ops::Range<usize> {
        let (address, len) = match self {
            ChrOp::Load { load, len } => (usize::from(load.address), *len),
            ChrOp::Bytes { address, bytes } => (usize::from(*address), bytes.len()),
            ChrOp::Gauge { address, .. } => (usize::from(*address), TILE_BYTES),
            ChrOp::Derived { address, len, .. } => (usize::from(*address), *len),
        };
        let start = address.min(CHR_SIZE);
        start..(start + len).min(CHR_SIZE)
    }
}

/// How a [`ChrLoad`] is saved.
#[derive(serde::Serialize, serde::Deserialize)]
struct SavedLoad {
    stream: String,
    range: Option<(usize, usize)>,
    address: u16,
}

/// How a [`ChrOp`] is saved.
#[derive(serde::Serialize, serde::Deserialize)]
enum SavedChrOp {
    Load {
        stream: String,
        range: Option<(usize, usize)>,
        address: u16,
    },
    Bytes(u16, Vec<u8>),
    Gauge(u16, usize),
    Derived {
        loads: Vec<SavedLoad>,
        from: usize,
        len: usize,
        mirror: bool,
        address: u16,
    },
}

/// CHR memory: 8 KB of tiles. Starts empty (all zero). A saved state holds what was done to it
/// (which streams went where), not the tiles: loading rebuilds them from the imported graphics.
#[derive(Clone)]
pub struct Chr {
    bytes: Box<[u8; CHR_SIZE]>,
    ops: Vec<ChrOp>,
}

impl PartialEq for Chr {
    fn eq(&self, other: &Chr) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Chr {}

impl fmt::Debug for Chr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Chr(..)")
    }
}

impl Default for Chr {
    fn default() -> Self {
        Chr {
            bytes: Box::new([0; CHR_SIZE]),
            ops: Vec::new(),
        }
    }
}

impl serde::Serialize for Chr {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Drop the changes that later ones wrote over completely.
        let mut covered = vec![false; CHR_SIZE];
        let mut kept = Vec::new();
        for op in self.ops.iter().rev() {
            let span = op.span();
            if span.clone().any(|at| !covered[at]) {
                covered[span].fill(true);
                kept.push(match op {
                    ChrOp::Load { load, .. } => SavedChrOp::Load {
                        stream: load.stream.to_owned(),
                        range: load.range,
                        address: load.address,
                    },
                    ChrOp::Bytes { address, bytes } => SavedChrOp::Bytes(*address, bytes.clone()),
                    ChrOp::Gauge { address, index } => SavedChrOp::Gauge(*address, *index),
                    ChrOp::Derived {
                        loads,
                        from,
                        len,
                        mirror,
                        address,
                    } => SavedChrOp::Derived {
                        loads: loads
                            .iter()
                            .map(|l| SavedLoad {
                                stream: l.stream.to_owned(),
                                range: l.range,
                                address: l.address,
                            })
                            .collect(),
                        from: *from,
                        len: *len,
                        mirror: *mirror,
                        address: *address,
                    },
                });
            }
        }
        kept.reverse();
        kept.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for Chr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Chr, D::Error> {
        use serde::de::Error;
        let saved = Vec::<SavedChrOp>::deserialize(d)?;
        let assets = crate::game::state::assets::<D::Error>()?;
        let mut chr = Chr::default();
        for op in saved {
            let done = match op {
                SavedChrOp::Load {
                    stream,
                    range,
                    address,
                } => chr.apply(
                    &assets.graphics,
                    &[ChrLoad {
                        stream: crate::game::state::intern(&stream),
                        range,
                        address,
                    }],
                ),
                SavedChrOp::Bytes(address, bytes) => {
                    chr.write(address, &bytes);
                    Ok(())
                }
                SavedChrOp::Gauge(address, index) => {
                    chr.write_gauge(address, &assets.graphics.gauge_rows, index);
                    Ok(())
                }
                SavedChrOp::Derived {
                    loads,
                    from,
                    len,
                    mirror,
                    address,
                } => {
                    let loads: Vec<ChrLoad> = loads
                        .into_iter()
                        .map(|l| ChrLoad {
                            stream: crate::game::state::intern(&l.stream),
                            range: l.range,
                            address: l.address,
                        })
                        .collect();
                    chr.copy_derived(&assets.graphics, &loads, from, len, mirror, address)
                }
            };
            done.map_err(D::Error::custom)?;
        }
        Ok(chr)
    }
}

impl Chr {
    /// Writes `bytes` from `address`. Bytes past the end of CHR memory go to the nametable on
    /// the original; here they are dropped. For bytes the game makes up; imported tiles go in
    /// with [`Chr::apply`], so that a saved state can leave them out.
    pub fn write(&mut self, address: u16, bytes: &[u8]) {
        self.put(address, bytes);
        self.ops.push(ChrOp::Bytes {
            address,
            bytes: bytes.to_vec(),
        });
    }

    fn put(&mut self, address: u16, bytes: &[u8]) -> usize {
        let start = usize::from(address).min(CHR_SIZE);
        let len = bytes.len().min(CHR_SIZE - start);
        self.bytes[start..start + len].copy_from_slice(&bytes[..len]);
        len
    }

    /// A gauge piece (16 bytes) of the value at `index` of `gauge_rows`, 0 if there is none.
    pub fn write_gauge(&mut self, address: u16, gauge_rows: &[u8], index: usize) {
        let value = gauge_rows.get(index).copied().unwrap_or(0);
        self.put(address, &[value; TILE_BYTES]);
        self.ops.push(ChrOp::Gauge { address, index });
    }

    /// Takes the three panel icon tiles of the gameplay of `level` (those of its background).
    pub fn copy_panel_icons(
        &mut self,
        graphics: &Graphics,
        level: &Level,
    ) -> Result<(), MissingGraphics> {
        self.copy_derived(
            graphics,
            &gameplay_background_loads(level),
            0x1000 + usize::from(PANEL_ICON_TILES) * TILE_BYTES,
            3 * TILE_BYTES,
            false,
            0x1000 + u16::from(PANEL_ICON_TILES) * TILE_BYTES as u16,
        )
    }

    /// Writes at `address` `len` bytes from `from` on of the CHR memory that `loads` make,
    /// reversed bit by bit when `mirror` (the saved state keeps the recipe, not the bytes).
    pub fn copy_derived(
        &mut self,
        graphics: &Graphics,
        loads: &[ChrLoad],
        from: usize,
        len: usize,
        mirror: bool,
        address: u16,
    ) -> Result<(), MissingGraphics> {
        let mut source = Chr::default();
        source.apply(graphics, loads)?;
        let bytes: Vec<u8> = source.bytes[from.min(CHR_SIZE)..(from + len).min(CHR_SIZE)]
            .iter()
            .map(|&b| if mirror { b.reverse_bits() } else { b })
            .collect();
        self.put(address, &bytes);
        self.ops.push(ChrOp::Derived {
            loads: loads.to_vec(),
            from,
            len: bytes.len(),
            mirror,
            address,
        });
        Ok(())
    }

    /// Replays loads in order.
    pub fn apply(&mut self, graphics: &Graphics, loads: &[ChrLoad]) -> Result<(), MissingGraphics> {
        for one in loads {
            let stream = graphics
                .tile_streams
                .get(one.stream)
                .ok_or_else(|| MissingGraphics(format!("tile stream {}", one.stream)))?;
            let bytes = match one.range {
                None => &stream[..],
                Some((start, end)) => stream.get(start..end).ok_or_else(|| {
                    MissingGraphics(format!("bytes {start:#X}-{end:#X} of {}", one.stream))
                })?,
            };
            let len = self.put(one.address, bytes);
            self.ops.push(ChrOp::Load {
                load: one.clone(),
                len,
            });
        }
        Ok(())
    }

    /// The pixel values (0-3) of every tile of pattern table `table` (0 or 1).
    /// The 8 KB of tile bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    pub fn pattern_table(&self, table: usize) -> PatternTable {
        let base = (table & 1) * 0x1000;
        let tiles = (0..256)
            .map(|tile| {
                let start = base + tile * TILE_BYTES;
                let bytes: &[u8; TILE_BYTES] = self.bytes[start..start + TILE_BYTES]
                    .try_into()
                    .expect("16 bytes");
                tile_pixels(bytes)
            })
            .collect();
        PatternTable { tiles }
    }
}

/// The 64 pixel values (0-3) of a tile in the NES 2-bit format, row by row.
pub fn tile_pixels(tile: &[u8; TILE_BYTES]) -> [u8; 64] {
    let mut pixels = [0u8; 64];
    for row in 0..8 {
        for column in 0..8 {
            let bit = 7 - column;
            pixels[row * 8 + column] = (tile[row] >> bit & 1) | (tile[row + 8] >> bit & 1) << 1;
        }
    }
    pixels
}

/// 256 tiles as pixel values.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternTable {
    #[serde(with = "crate::game::state::vec_array")]
    tiles: Vec<[u8; 64]>,
}

impl PatternTable {
    /// Pixel value (0-3) at (`x`, `y`) of `tile`, `x` and `y` 0-7.
    pub fn pixel(&self, tile: u8, x: u32, y: u32) -> u8 {
        self.tiles[usize::from(tile)][(y * 8 + x) as usize]
    }

    /// One row (0-7) of a tile.
    pub fn row(&self, tile: u8, y: u32) -> &[u8] {
        let start = (y * 8) as usize;
        &self.tiles[usize::from(tile)][start..start + 8]
    }

    /// All 64 pixel values of a tile, row by row.
    pub fn tile(&self, tile: u8) -> [u8; 64] {
        self.tiles[usize::from(tile)]
    }

    /// Replaces a tile's pixel values.
    pub fn set_tile(&mut self, tile: u8, pixels: [u8; 64]) {
        self.tiles[usize::from(tile)] = pixels;
    }
}

/// The 32 palette entries (NES colour numbers) as the game keeps them: entries 0-15 for the
/// background, 16-31 for sprites. Entry 16 is the universal background colour, shown wherever
/// a background pixel is 0 (graphics.md 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Palette(pub [u8; 32]);

impl Palette {
    pub fn universal_background(&self) -> u8 {
        self.0[16]
    }

    /// Colour of background pixel `value` (0-3) drawn with background palette `palette` (0-3).
    pub fn background(&self, palette: u8, value: u8) -> u8 {
        if value == 0 {
            self.universal_background()
        } else {
            self.0[usize::from(palette & 3) * 4 + usize::from(value & 3)]
        }
    }

    pub fn apply(&mut self, preset: &PalettePreset) {
        for (i, &colour) in preset.colours.iter().enumerate() {
            if let Some(entry) = self.0.get_mut(usize::from(preset.first_entry) + i) {
                *entry = colour;
            }
        }
    }
}

/// The gameplay palette of a level (graphics.md 7.3): presets 11 and 10, then the level's
/// colours into background palettes 2-3 and sprite palettes 2-3, and entry 16 black.
pub fn gameplay_palette(graphics: &Graphics, level: usize) -> Result<Palette, MissingGraphics> {
    let mut palette = Palette([BLACK; 32]);
    for preset in [PRESET_GAMEPLAY_BACKGROUND, PRESET_GAMEPLAY_SPRITES] {
        let preset = graphics
            .presets
            .get(preset)
            .ok_or_else(|| MissingGraphics(format!("palette preset {preset}")))?;
        palette.apply(preset);
    }
    let colours = graphics
        .level_colours
        .get(level)
        .filter(|colours| colours.len() == 12)
        .ok_or_else(|| MissingGraphics(format!("the 12 colours of level {level}")))?;
    for (i, &entry) in [9, 10, 11, 13, 14, 15, 25, 26, 27, 29, 30, 31]
        .iter()
        .enumerate()
    {
        palette.0[entry] = colours[i];
    }
    // Entry 16 is set to black; on warp levels black is uploaded in its place anyway.
    palette.0[16] = BLACK;
    Ok(palette)
}

/// What a level's background is drawn with.
///
/// A saved state holds only the level's number; loading builds the rest again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "LevelNumber", into = "LevelNumber")]
pub struct LevelGraphics {
    /// The number of the level these are for.
    pub number: u8,
    pub background: PatternTable,
    pub palette: Palette,
    /// The gauge pieces' byte values ([`Graphics::gauge_rows`]).
    pub gauge_rows: Vec<u8>,
    /// The tile bytes of the item pictures (0x90 each, graphics.md 1.2), empty when the import
    /// lacks them.
    pub item_pictures: Vec<u8>,
    /// The tractor beam's nine images of sprite tile $CF, 16 bytes each (player.md 8.4), empty
    /// when the import lacks them.
    pub beam_tiles: Vec<u8>,
    /// The bonus sign's nine pictures for sprite tiles $BC and $D0, 32 bytes each
    /// (graphics.md 3.1), empty when the import lacks them.
    pub sign_tiles: Vec<u8>,
}

impl LevelGraphics {
    /// Builds the background pattern table (table 1, PPU $1000) and palette of a level,
    /// starting from empty CHR memory.
    pub fn new(graphics: &Graphics, level: &Level) -> Result<LevelGraphics, MissingGraphics> {
        let mut chr = Chr::default();
        chr.apply(graphics, &gameplay_background_loads(level))?;
        Ok(LevelGraphics {
            number: level.number,
            background: chr.pattern_table(1),
            palette: gameplay_palette(graphics, usize::from(level.number))?,
            gauge_rows: graphics.gauge_rows.clone(),
            item_pictures: graphics
                .tile_streams
                .get(streams::ITEM_PICTURES)
                .cloned()
                .unwrap_or_default(),
            beam_tiles: graphics
                .tile_streams
                .get(streams::BEAM)
                .cloned()
                .unwrap_or_default(),
            sign_tiles: graphics
                .tile_streams
                .get(streams::SIGNS)
                .cloned()
                .unwrap_or_default(),
        })
    }
}

/// How a [`LevelGraphics`] is saved.
#[derive(serde::Serialize, serde::Deserialize)]
struct LevelNumber(u8);

impl From<LevelGraphics> for LevelNumber {
    fn from(graphics: LevelGraphics) -> LevelNumber {
        LevelNumber(graphics.number)
    }
}

impl TryFrom<LevelNumber> for LevelGraphics {
    type Error = String;

    fn try_from(saved: LevelNumber) -> Result<LevelGraphics, String> {
        let assets =
            crate::game::state::assets::<serde::de::value::Error>().map_err(|e| e.to_string())?;
        let level = assets
            .world
            .levels
            .iter()
            .find(|level| level.number == saved.0)
            .ok_or_else(|| format!("no level {}", saved.0))?;
        LevelGraphics::new(&assets.graphics, level).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::level::tests::synthetic_world;

    /// Synthetic graphics: every stream is tiles whose bytes are all the same value (the stream's
    /// own marker), so a test can tell which load a tile came from.
    pub(crate) fn synthetic_graphics() -> Graphics {
        use streams::*;
        let stream = |marker: u8, tiles: usize| vec![marker; tiles * TILE_BYTES];
        let mut tile_streams = BTreeMap::new();
        for (path, marker, tiles) in [
            (STATUS_PANEL, 0x01, 27),
            (STATUS_PANEL_1D60, 0x02, 5),
            (TERRAIN_TYPES_0_4, 0x10, 181),
            (TERRAIN_TYPES_1_6, 0x11, 256),
            (TERRAIN_TYPES_2_3_5, 0x12, 191),
            (TERRAIN_COMMON, 0x20, 51),
            (TYPE_3_EXTRA_12A0, 0x30, 32),
            (TYPE_3_EXTRA_1820, 0x31, 18),
            (TYPE_5_EXTRA_15C0, 0x50, 19),
            (TYPE_5_EXTRA_1B00, 0x51, 15),
            (LEVELS_1_8_EXTRA, 0x18, 56),
        ] {
            tile_streams.insert(path.to_owned(), stream(marker, tiles));
        }
        // Warp sets: set k is all bytes 0x80 + k.
        let warp = (0..12u8)
            .flat_map(|set| vec![0x80 + set; WARP_SET_TILES * TILE_BYTES])
            .collect();
        tile_streams.insert(WARP_BACKGROUNDS.to_owned(), warp);
        let presets = (0..28u8)
            .map(|p| PalettePreset {
                first_entry: if p == 10 { 16 } else { 1 },
                colours: (0..8).map(|i| p + i).collect(),
            })
            .collect();
        let level_colours = (0..42u8)
            .map(|level| (0..12).map(|i| (level + i) & 0x3F).collect())
            .collect();
        Graphics {
            tile_streams,
            presets,
            level_colours,
            ..Graphics::default()
        }
    }

    fn level(number: u8, graphics_type: u8) -> Level {
        let mut level = synthetic_world().levels[0].clone();
        level.number = number;
        level.graphics_type = graphics_type;
        level
    }

    /// The marker byte of the first byte of tile `tile` of table 1.
    fn marker(chr: &Chr, tile: usize) -> u8 {
        chr.bytes[0x1000 + tile * TILE_BYTES]
    }

    #[test]
    fn terrain_loads_follow_the_graphics_type() {
        let graphics = synthetic_graphics();
        let chr_for = |level: &Level| {
            let mut chr = Chr::default();
            chr.apply(&graphics, &gameplay_background_loads(level))
                .unwrap();
            chr
        };
        let chr = chr_for(&level(4, 0));
        assert_eq!(marker(&chr, 0), 0x10);
        assert_eq!(marker(&chr, 0xB4), 0x10);
        assert_eq!(marker(&chr, 0xB5), 0); // nothing loaded there
        assert_eq!(marker(&chr, 0xCC), 0x20); // common background at $1CC0
        assert_eq!(marker(&chr, 0xFE), 0x20);
        assert_eq!(marker(&chr, 0xD6), 0x20); // common overwrites the status panel at $1D60

        let chr = chr_for(&level(0, 3));
        assert_eq!(marker(&chr, 0x29), 0x12);
        assert_eq!(marker(&chr, 0x2A), 0x30); // $12A0
        assert_eq!(marker(&chr, 0x82), 0x31); // $1820

        let chr = chr_for(&level(12, 5));
        assert_eq!(marker(&chr, 0x5C), 0x50);
        assert_eq!(marker(&chr, 0xB0), 0x51);

        // Levels 1 and 8 load their own extra instead of the type's.
        let chr = chr_for(&level(8, 1));
        assert_eq!(marker(&chr, 0x2C), 0x11);
        assert_eq!(marker(&chr, 0x2D), 0x18);
    }

    #[test]
    fn warp_levels_load_only_their_set() {
        let graphics = synthetic_graphics();
        for (number, set) in [(29u8, 0u8), (35, 6), (40, 11), (41, 10)] {
            let mut chr = Chr::default();
            chr.apply(&graphics, &gameplay_background_loads(&level(number, 0)))
                .unwrap();
            assert_eq!(marker(&chr, 0x00), 0, "no terrain on level {number}");
            assert_eq!(marker(&chr, 0x81), 0x80 + set, "level {number}");
            assert_eq!(marker(&chr, 0x91), 0x80 + set, "level {number}");
            assert_eq!(marker(&chr, 0x92), 0, "level {number}");
            assert_eq!(marker(&chr, 0xE4), 0x01, "status panel at $1E40");
        }
    }

    #[test]
    fn partial_streams_and_overflow() {
        let mut chr = Chr::default();
        chr.write(0x1FFE, &[1, 2, 3, 4]);
        assert_eq!(&chr.bytes[CHR_SIZE - 2..], &[1, 2]);
        let mut graphics = synthetic_graphics();
        graphics.tile_streams.remove(streams::TERRAIN_COMMON);
        let error = Chr::default()
            .apply(&graphics, &gameplay_background_loads(&level(0, 0)))
            .unwrap_err();
        assert!(error.0.contains("common"), "{error}");
    }

    #[test]
    fn tiles_decode_to_pixels() {
        let mut chr = Chr::default();
        let mut tile = [0u8; 16];
        tile[0] = 0b1000_0001;
        tile[8] = 0b1100_0000;
        chr.write(0x1000 + 5 * 16, &tile);
        let table = chr.pattern_table(1);
        assert_eq!(table.row(5, 0), &[3, 2, 0, 0, 0, 0, 0, 1]);
        assert_eq!(table.pixel(5, 7, 0), 1);
        assert_eq!(table.pixel(5, 0, 1), 0);
    }

    #[test]
    fn gameplay_palette_layout() {
        let graphics = synthetic_graphics();
        let palette = gameplay_palette(&graphics, 3).unwrap();
        // Preset 11 from entry 1, preset 10 from entry 16, then entry 16 black.
        assert_eq!(&palette.0[1..8], &[11, 12, 13, 14, 15, 16, 17]);
        assert_eq!(&palette.0[17..24], &[11, 12, 13, 14, 15, 16, 17]);
        assert_eq!(palette.universal_background(), BLACK);
        // Level 3's colours: 3, 4, ... 14.
        assert_eq!(&palette.0[9..12], &[3, 4, 5]);
        assert_eq!(&palette.0[13..16], &[6, 7, 8]);
        assert_eq!(&palette.0[25..28], &[9, 10, 11]);
        assert_eq!(&palette.0[29..32], &[12, 13, 14]);
        assert_eq!(palette.background(2, 1), 3);
        assert_eq!(palette.background(3, 3), 8);
        assert_eq!(palette.background(3, 0), BLACK);
        assert!(gameplay_palette(&graphics, 42).is_err());
    }
}
