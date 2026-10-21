//! Where the original's graphics are in the PRG ROM (`docs/spec/graphics.md`, section 4): a
//! table of locations, formats and lengths, with no graphics in it (`docs/CLEAN_ROOM.md`,
//! rule 5). Names are ours and say what each blob is.

use crate::rom::cartridge::Rom;
use crate::rom::decode::{GLYPH_BYTES, font_tiles, run_length};

/// How a blob is stored (graphics.md section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Bytes copied unchanged (2.1 raw tiles, 2.2 raw nametable or attribute bytes).
    Raw,
    /// A run-length stream (2.3) with this bias added to every output byte.
    RunLength { bias: u8 },
    /// 1-bit font glyphs (2.4), 7 bytes each, mask 0.
    Font,
}

impl Format {
    /// Short name used in the manifest.
    pub fn name(self) -> &'static str {
        match self {
            Format::Raw => "raw",
            Format::RunLength { .. } => "run-length",
            Format::Font => "font",
        }
    }
}

/// How sure the spec is of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certainty {
    /// Verified against the running original.
    Verified,
    /// The decode was verified; what the scene is, is not known.
    DecodeVerified,
    /// Read from the program, not yet verified.
    Read,
    /// A hypothesis.
    Hypothesis,
}

impl Certainty {
    /// Short name used in the manifest.
    pub fn name(self) -> &'static str {
        match self {
            Certainty::Verified => "verified",
            Certainty::DecodeVerified => "decode verified",
            Certainty::Read => "read from the program",
            Certainty::Hypothesis => "hypothesis",
        }
    }
}

/// One blob of data in the PRG ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blob {
    /// PRG ROM offset (header excluded).
    pub offset: u32,
    pub format: Format,
    /// Raw: bytes. Run-length: the compressed length `L`. Font: number of glyphs.
    pub length: u32,
    /// Decoded size in bytes, when the spec states it.
    pub decoded: Option<u32>,
}

const fn raw(offset: u32, length: u32) -> Blob {
    Blob {
        offset,
        format: Format::Raw,
        length,
        decoded: Some(length),
    }
}

const fn rle(offset: u32, length: u32, decoded: u32) -> Blob {
    rle_bias(offset, length, decoded, 0)
}

const fn rle_bias(offset: u32, length: u32, decoded: u32, bias: u8) -> Blob {
    Blob {
        offset,
        format: Format::RunLength { bias },
        length,
        decoded: Some(decoded),
    }
}

/// A run-length stream whose decoded size the spec does not give.
const fn rle_unsized(offset: u32, length: u32) -> Blob {
    Blob {
        offset,
        format: Format::RunLength { bias: 0 },
        length,
        decoded: None,
    }
}

const fn font(offset: u32, glyphs: u32) -> Blob {
    Blob {
        offset,
        format: Format::Font,
        length: glyphs,
        decoded: Some(glyphs * 16),
    }
}

impl Blob {
    /// Number of ROM bytes the blob occupies.
    pub fn stored_bytes(&self) -> usize {
        match self.format {
            Format::Font => self.length as usize * GLYPH_BYTES,
            Format::Raw | Format::RunLength { .. } => self.length as usize,
        }
    }

    /// Exclusive end offset in the PRG ROM.
    pub fn end(&self) -> usize {
        self.offset as usize + self.stored_bytes()
    }

    /// The stored bytes, or `None` if the blob runs past the end of the ROM.
    pub fn stored<'a>(&self, rom: &'a Rom) -> Option<&'a [u8]> {
        rom.bytes(self.offset as usize, self.stored_bytes())
    }

    /// The decoded bytes (tile bytes, or nametable / attribute bytes), or `None` if the blob
    /// runs past the end of the ROM.
    pub fn decode(&self, rom: &Rom) -> Option<Vec<u8>> {
        let stored = self.stored(rom)?;
        Some(match self.format {
            Format::Raw => stored.to_vec(),
            Format::RunLength { bias } => run_length(stored, bias),
            Format::Font => font_tiles(stored, 0),
        })
    }
}

/// A set of tiles, imported as one sheet.
#[derive(Debug, Clone, Copy)]
pub struct TileSet {
    /// Path in the asset pack.
    pub path: &'static str,
    pub blob: Blob,
    /// PPU addresses the original loads it to; empty when variable or not known.
    pub destinations: &'static [u16],
    /// For a sheet of equal blocks of which the original loads one: tiles per block.
    pub block_tiles: Option<u16>,
    pub purpose: &'static str,
    pub certainty: Certainty,
}

/// One piece of a picture: decoded bytes written to the nametable from `address`.
#[derive(Debug, Clone, Copy)]
pub struct PicturePart {
    pub blob: Blob,
    /// PPU address of the first byte ($2000-$23FF).
    pub address: u16,
    /// The original adds a tile base chosen at run time to every byte (graphics.md 5.9); the
    /// pack holds the bytes decoded with bias 0.
    pub add_tile_base: bool,
}

/// A nametable picture: its parts are written in order.
#[derive(Debug, Clone, Copy)]
pub struct Picture {
    pub path: &'static str,
    pub parts: &'static [PicturePart],
    pub purpose: &'static str,
    pub certainty: Certainty,
}

use Certainty::{DecodeVerified, Hypothesis, Read, Verified};

const fn tiles(
    path: &'static str,
    blob: Blob,
    destinations: &'static [u16],
    purpose: &'static str,
    certainty: Certainty,
) -> TileSet {
    TileSet {
        path,
        blob,
        destinations,
        block_tiles: None,
        purpose,
        certainty,
    }
}

const fn blocks(mut set: TileSet, block_tiles: u16) -> TileSet {
    set.block_tiles = Some(block_tiles);
    set
}

const fn part(blob: Blob, address: u16) -> PicturePart {
    PicturePart {
        blob,
        address,
        add_tile_base: false,
    }
}

/// Every tile set of graphics.md section 4, in ROM order.
#[rustfmt::skip]
pub const TILE_SETS: &[TileSet] = &[
    // Bank 1: sprite tiles (4.1, 6.1, 6.4).
    tiles("tiles/hud/digits.png", raw(0x08100, 0x120), &[], "HUD digit tiles, streamed 2 at a time during play", Read),
    tiles("tiles/sprites/common-0f70.png", raw(0x0CB45, 0x80), &[0x0F70], "common sprites (title and gameplay)", Verified),
    tiles("tiles/sprites/pod-type-0.png", raw(0x0CBC5, 0x400), &[0x0010], "jetpod sprites in play, pod type 0", Verified),
    tiles("tiles/sprites/pod-type-1-and-3.png", raw(0x0CFC5, 0x400), &[0x0010], "jetpod sprites in play, pod types 1 and 3", Verified),
    tiles("tiles/sprites/pod-unused-a.png", raw(0x0D3C5, 0x3C0), &[], "look like further pod frames; no loader found", Hypothesis),
    tiles("tiles/sprites/pod-type-2.png", raw(0x0D785, 0x400), &[0x0010], "jetpod sprites in play, pod type 2", Verified),
    tiles("tiles/sprites/beam.png", raw(0x0805D, 0x90), &[0x0CF0], "the tractor beam's nine tile images, one uploaded to sprite tile $CF each frame of a tow (player.md 8.4)", Verified),
    tiles("tiles/sprites/pod-unused-b.png", raw(0x0DB85, 0x3C0), &[], "look like further pod frames; no loader found", Hypothesis),
    tiles("tiles/sprites/common-0df0.png", raw(0x0DF45, 0x100), &[0x0DF0], "common sprites", Verified),
    tiles("tiles/sprites/objects.png", raw(0x0E045, 0x1480), &[], "object (enemy) sprite sets, copied per level from tile $41 of table 0 (graphics.md 6.4); tile n is ROM offset 0x0E045 + 16 n", Verified),
    tiles("tiles/sprites/common-0c80.png", raw(0x0E0B5, 0x20), &[0x0C80], "common sprites (inside the object area)", Verified),
    tiles("tiles/sprites/common-0ca0.png", raw(0x0E255, 0x50), &[0x0CA0], "common sprites (inside the object area)", Verified),
    tiles("tiles/sprites/common-0d10.png", raw(0x0E2A5, 0xD0), &[0x0D10], "common sprites (inside the object area)", Verified),
    tiles("tiles/misc/mirrored-pair.png", raw(0x0E345, 0x20), &[0x1A00], "two tiles written as is and mirrored left to right on one screen (graphics.md 3)", Read),
    tiles("tiles/sprites/title.png", raw(0x0F185, 0x120), &[0x0010], "title sprites", Verified),
    tiles("tiles/sprites/level-26.png", rle(0x0F4C5, 0x1B2, 0x1C1), &[0x0000], "level 26 only", Read),
    tiles("tiles/sprites/streamed-animation.png", raw(0x0F6AE, 0x1A0), &[], "13 frames of 2 tiles, streamed during the first 13 frames of a sequence", Read),
    tiles("tiles/sprites/objects-extra-a.png", raw(0x0F91E, 0x60), &[], "object sprite set (list B)", Verified),
    tiles("tiles/sprites/objects-extra-b.png", raw(0x0F97E, 0xF0), &[], "object sprite set (list B)", Verified),
    tiles("tiles/hud/status-panel.png", rle(0x0FA6E, 0xD4, 0x1B0), &[0x1E40], "status panel tiles (approach screen, gameplay HUD, surface menu)", Verified),
    tiles("tiles/hud/status-panel-alt.png", rle(0x0FA74, 0x86, 0xF0), &[], "a second stream starting inside the status panel stream; caller not reached", Read),
    tiles("tiles/hud/status-panel-1d60.png", raw(0x0FB42, 0x50), &[0x1D60], "status panel tiles", Verified),
    blocks(tiles("tiles/sprites/common-0ef0-blocks.png", raw(0x0FB92, 6 * 0x60), &[0x0EF0], "six 6-tile blocks, one loaded by RAM $D1 (1-4 → block 0-3, 5 and up → value − 3)", Verified), 6),
    tiles("tiles/sprites/common-0f50.png", raw(0x0FDD2, 0x20), &[0x0F50], "common sprites", Verified),
    tiles("tiles/sprites/common-0ba0.png", raw(0x0FDF2, 0x20), &[0x0BA0], "common sprites", Verified),
    tiles("tiles/sprites/common-0de0.png", raw(0x0FE12, 0x10), &[0x0DE0], "common sprites", Verified),
    tiles("tiles/sprites/common-0bd0.png", raw(0x0FE22, 0xB0), &[0x0BD0], "common sprites", Verified),
    tiles("tiles/title/sprites-0b80.png", raw(0x0FE52, 0x20), &[0x0B80], "title sprites", Verified),
    tiles("tiles/title/sprites-0800.png", rle(0x0FF2F, 0x19, 0x60), &[0x0800, 0x0E00], "sprite tiles of the title ($0800) and attract sequence ($0E00)", Verified),
    // Bank 2: title, fonts, terrain (4.2).
    tiles("tiles/title/logo.png", rle(0x122DE, 0x8BB, 0x9C0), &[0x1640], "title logo", Verified),
    tiles("tiles/unidentified-a/tiles.png", rle(0x12DA5, 0x242, 0x261), &[0x1800], "screen of bank 2 entry $8036 (context unknown)", DecodeVerified),
    tiles("tiles/unidentified-b/tiles.png", rle(0x12FFD, 0x372, 0x3F1), &[0x1000], "screen of bank 2 entry $8033: the interplanetary flight's background tiles (scenes.md 5; graphics.md calls the context unknown)", DecodeVerified),
    tiles("tiles/terrain/types-1-6.png", rle(0x1336F, 0xDDD, 0x1000), &[0x1000], "terrain tiles, graphics types 1 and 6", Verified),
    tiles("tiles/terrain/types-0-4.png", rle(0x1414C, 0xB57, 0xB51), &[0x1000], "terrain tiles, graphics types 0 and 4", Verified),
    tiles("tiles/terrain/type-3-extra-12a0.png", rle(0x14CA3, 0x1EB, 0x201), &[0x12A0], "terrain extra, graphics type 3", Verified),
    tiles("tiles/terrain/type-3-extra-1820.png", rle(0x14E8E, 0x11F, 0x120), &[0x1820], "terrain extra, graphics type 3", Verified),
    tiles("tiles/terrain/type-5-extra-15c0.png", raw(0x14FAD, 0x130), &[0x15C0], "terrain extra, graphics type 5", Verified),
    tiles("tiles/terrain/type-5-extra-1b00.png", raw(0x150DD, 0xF0), &[0x1B00], "terrain extra, graphics type 5", Verified),
    tiles("tiles/terrain/levels-1-8-extra.png", rle(0x151CD, 0x379, 0x381), &[0x12D0], "terrain extra, levels 1 and 8", Verified),
    tiles("tiles/title/sprites-0860.png", rle(0x15546, 0x256, 0x341), &[0x0860], "title sprite tiles", Verified),
    tiles("tiles/frames-screen/tiles-alt.png", rle(0x1579C, 0x266, 0x291), &[0x1000], "screen of graphics.md 5.10, variant (RAM $63 bit 7 set)", Verified),
    tiles("tiles/frames-screen/tiles.png", rle(0x15A02, 0x1B4, 0x200), &[0x0000, 0x1000], "screen of graphics.md 5.10", Verified),
    tiles("tiles/font/letters.png", font(0x15BB6, 31), &[], "font: letters (A-Z and others)", Verified),
    tiles("tiles/font/digits.png", font(0x15C96, 12), &[], "font: digits and punctuation", Verified),
    tiles("tiles/font/frame.png", rle(0x15CEA, 0x56, 0x60), &[], "font extras: box frame pieces", Verified),
    tiles("tiles/frames-screen/squares.png", raw(0x15D40, 0x6C0), &[0x1600, 0x0800], "screen of graphics.md 5.10", Verified),
    tiles("tiles/frames-screen/squares-extra-a.png", raw(0x16400, 0x120), &[], "screen of graphics.md 5.10, after the squares when (level index AND 3) is not 3", Verified),
    tiles("tiles/frames-screen/squares-extra-b.png", raw(0x16520, 0x120), &[], "screen of graphics.md 5.10, after the squares when (level index AND 3) = 3", Read),
    tiles("tiles/terrain/types-2-3-5.png", rle(0x16640, 0xB19, 0xBF1), &[0x1000], "terrain tiles, graphics types 2, 3 and 5", Verified),
    tiles("tiles/terrain/common.png", rle(0x17159, 0x24C, 0x330), &[0x1CC0], "gameplay background tiles common to all terrain", Verified),
    // Bank 4: mothership launch (4.3).
    tiles("tiles/launch/pod-type-0.png", rle(0x2159C, 0x145, 0x170), &[0x0000], "launch sprites, pod type 0", Verified),
    tiles("tiles/launch/pod-type-1.png", rle(0x216E1, 0x15A, 0x170), &[0x0000], "launch sprites, pod type 1", Verified),
    tiles("tiles/launch/pod-type-2.png", rle(0x2183B, 0x153, 0x170), &[0x0000], "launch sprites, pod type 2", Verified),
    tiles("tiles/launch/sprites.png", rle(0x2198E, 0x56E, 0x791), &[0x0170], "launch sprites", Verified),
    // Bank 5: planet-surface pictures and equipment pictures (4.4).
    tiles("tiles/surface/common.png", rle_unsized(0x2840A, 0x19), &[0x1A30], "tiles $A3-$A7 of table 1 for every surface picture", Verified),
    tiles("tiles/surface/extra-shared.png", rle_unsized(0x29A7E, 0x239), &[0x1A80], "surface pictures 0, 2, 4 and 5: extra tiles", Verified),
    tiles("tiles/surface/extra-picture-1.png", rle_unsized(0x29CB7, 0x72), &[0x1A80, 0x0A80], "surface picture 1: extra tiles (also loaded at $0A80 by bank 5 entry $800C)", Verified),
    tiles("tiles/surface/extra-picture-6.png", rle_unsized(0x29A43, 0x3B), &[0x1A80], "surface picture 6: extra tiles", Verified),
    tiles("tiles/surface/picture-0-table-1.png", rle_unsized(0x29D29, 0x8C3), &[0x1000], "surface picture 0, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-0-table-0.png", rle_unsized(0x2A5EC, 0xC00), &[0x0000], "surface picture 0, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/surface/picture-2-table-1.png", rle_unsized(0x2B1EC, 0x7CA), &[0x1000], "surface picture 2, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-2-table-0.png", rle_unsized(0x2B9B6, 0x413), &[0x0000], "surface picture 2, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/surface/picture-1-table-1.png", rle_unsized(0x2BDC9, 0x533), &[0x1000], "surface picture 1, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-1-table-0.png", rle_unsized(0x2C2FC, 0xD80), &[0x0000], "surface picture 1, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/surface/picture-4-table-1.png", rle_unsized(0x2D07C, 0x814), &[0x1000], "surface picture 4, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-4-table-0.png", rle_unsized(0x2D890, 0x961), &[0x0000], "surface picture 4, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/surface/picture-5-table-1.png", rle_unsized(0x2E1F1, 0x6A0), &[0x1000], "surface picture 5, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-5-table-0.png", rle_unsized(0x2E891, 0xA97), &[0x0000], "surface picture 5, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/equipment/picture-2.png", rle(0x2F328, 0x1EC, 0x220), &[0x1810], "equipment picture 2 (items 6 and 7)", Verified),
    tiles("tiles/equipment/picture-0.png", rle(0x2F514, 0x194, 0x1C0), &[0x1810], "equipment picture 0 (items 0 and 3)", Verified),
    tiles("tiles/equipment/picture-1.png", rle(0x2F6A8, 0x1E7, 0x201), &[0x1810], "equipment picture 1 (item 4)", Verified),
    tiles("tiles/equipment/picture-3.png", rle(0x2F88F, 0x192, 0x1A1), &[0x1810], "equipment picture 3 (not used by the item map)", Read),
    // Bank 6: attract, approach, menus, shop, warp levels (4.5).
    tiles("tiles/attract/jetman-table-1.png", rle(0x30973, 0xEC5, 0x1000), &[0x1000], "attract sequence: floating Jetman, background", Verified),
    tiles("tiles/attract/jetman-table-0.png", rle(0x31838, 0xB12, 0xC00), &[0x0000], "attract sequence: floating Jetman, more tiles", Verified),
    blocks(tiles("tiles/equipment/shop-blocks.png", raw(0x32E56, 18 * 0x120), &[0x1810, 0x1930], "equipment/shop screen: block 0 at $1810, block (item + 1) at $1930; blocks 8-17 unconfirmed", Verified), 18),
    tiles("tiles/comet/table-1.png", rle(0x344A4, 0xC89, 0xF50), &[0x1000], "comet picture screen (bank 6 entry $8012; context unknown)", DecodeVerified),
    tiles("tiles/comet/table-0.png", rle(0x3512D, 0x1D0, 0x1E1), &[0x0000], "comet picture screen (bank 6 entry $8012; context unknown)", DecodeVerified),
    tiles("tiles/equipment/shop.png", rle(0x35394, 0x42F, 0x501), &[0x0800], "equipment/shop screen tiles", Verified),
    tiles("tiles/attract/space-loonies-logo.png", rle(0x357C3, 0x312, 0x3E1), &[0x0C00], "\"Federation of Space Loonies\" logo sprites", Verified),
    tiles("tiles/approach/background.png", rle(0x35B99, 0x3A3, 0x3E1), &[0x1800], "approach screen background", Verified),
    tiles("tiles/approach/sprites.png", rle(0x35F3C, 0x11E, 0x131), &[0x0010], "approach screen sprites", Verified),
    tiles("tiles/menus/frame-and-font.png", rle(0x3684F, 0x27C, 0x401), &[0x1000], "menu screens A-D: frame and font tiles (screen A loads them at $1000 + 16 × tile base)", Verified),
    blocks(tiles("tiles/warp/backgrounds.png", raw(0x36B43, 12 * 0x110), &[0x1810], "warp levels 29-41: 12 background sets, set = level − 29 (level 41 uses set 10)", Verified), 17),
    // Bank 7: surface picture 6, backdrop, ending, launch background (4.6).
    tiles("tiles/surface/picture-6-table-1.png", rle(0x3905A, 0x795, 0x9A0), &[0x1000], "surface picture 6, table 1 (screen rows 0-15)", Verified),
    tiles("tiles/surface/picture-6-table-0.png", rle(0x397EF, 0xC41, 0x1000), &[0x0000], "surface picture 6, table 0 (screen rows 16-29)", Verified),
    tiles("tiles/backdrop/table-1.png", rle(0x3A430, 0x3DD, 0x591), &[0x1000], "backdrop screen (bank 7 entry $8006; levels 0, 4 and 8; context unknown)", DecodeVerified),
    tiles("tiles/backdrop/table-0.png", raw(0x3A88D, 0x1C0), &[0x0800], "backdrop screen (bank 7 entry $8006; levels 0, 4 and 8; context unknown)", DecodeVerified),
    tiles("tiles/ending/tiles-1000.png", rle(0x3D663, 0xB5C, 0xDA1), &[0x1000], "ending picture", DecodeVerified),
    tiles("tiles/ending/tiles-1dc0.png", rle(0x3E1BF, 0x20F, 0x241), &[0x1DC0], "ending picture", DecodeVerified),
    tiles("tiles/ending/tiles-0000.png", rle(0x3E3CE, 0xB0A, 0xD50), &[0x0000], "ending picture", DecodeVerified),
    tiles("tiles/ending/tiles-0d60.png", rle(0x3EED8, 0x245, 0x2A0), &[0x0D60], "ending picture", DecodeVerified),
    tiles("tiles/launch/background.png", rle(0x3F15F, 0xE19, 0xFC1), &[0x1000], "mothership launch background", Verified),
];

const NAMETABLE: u16 = 0x2000;
const ATTRIBUTES: u16 = 0x23C0;

/// A surface picture: raw nametable and raw attributes (graphics.md 4.4).
const fn surface(nametable: u32, attributes: u32) -> [PicturePart; 2] {
    [
        part(raw(nametable, 0x3C0), NAMETABLE),
        part(raw(attributes, 0x40), ATTRIBUTES),
    ]
}

/// Every nametable picture of graphics.md section 4.
pub const PICTURES: &[Picture] = &[
    Picture {
        path: "pictures/title.ron",
        parts: &[
            part(rle_bias(0x12B99, 0x112, 0x1C0, 0x64), NAMETABLE),
            part(rle(0x12CAB, 0x10, 0x20), ATTRIBUTES),
        ],
        purpose: "title screen",
        certainty: Verified,
    },
    Picture {
        path: "pictures/frames-screen.ron",
        parts: &[
            part(rle(0x1130F, 0x1F, 0x40), ATTRIBUTES),
            part(rle(0x1134C, 0x10C, 0x340), NAMETABLE),
        ],
        purpose: "screen of graphics.md 5.10 (rows of square frames; RAM $63 bit 7 clear)",
        certainty: Verified,
    },
    Picture {
        path: "pictures/frames-screen-alt-attributes.ron",
        parts: &[part(rle(0x112EE, 0x21, 0x40), ATTRIBUTES)],
        purpose: "screen of graphics.md 5.10, variant attributes (RAM $63 bit 7 set)",
        certainty: Verified,
    },
    Picture {
        path: "pictures/unidentified-a.ron",
        parts: &[part(raw(0x12FE7, 0x16), 0x23D1)],
        purpose: "attributes of the screen of bank 2 entry $8036 (context unknown)",
        certainty: Read,
    },
    Picture {
        path: "pictures/launch.ron",
        parts: &[
            part(rle(0x21F1F, 0x33F, 0x3C0), NAMETABLE),
            part(rle(0x21EFC, 0x23, 0x40), ATTRIBUTES),
        ],
        purpose: "mothership launch",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-0.ron",
        parts: &surface(0x287E3, 0x2828A),
        purpose: "planet-surface picture 0",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-1.ron",
        parts: &surface(0x28F03, 0x282CA),
        purpose: "planet-surface picture 1",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-2.ron",
        parts: &surface(0x28BA3, 0x2830A),
        purpose: "planet-surface picture 2",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-4.ron",
        parts: &surface(0x292C3, 0x2834A),
        purpose: "planet-surface picture 4",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-5.ron",
        parts: &surface(0x28423, 0x2838A),
        purpose: "planet-surface picture 5",
        certainty: Verified,
    },
    Picture {
        path: "pictures/surface-6.ron",
        parts: &surface(0x29683, 0x283CA),
        purpose: "planet-surface picture 6",
        certainty: Verified,
    },
    Picture {
        path: "pictures/comet.ron",
        parts: &[part(rle(0x34296, 0x20E, 0x400), NAMETABLE)],
        purpose: "comet picture (bank 6 entry $8012; context unknown): nametable and attributes",
        certainty: DecodeVerified,
    },
    Picture {
        path: "pictures/approach.ron",
        parts: &[part(rle(0x35AD5, 0xC4, 0x2EF), NAMETABLE)],
        purpose: "approach screen (\"APPROACHING PLANET\")",
        certainty: Verified,
    },
    Picture {
        path: "pictures/menu-a.ron",
        parts: &[
            PicturePart {
                blob: rle(0x36149, 0x1A9, 0x301),
                address: NAMETABLE,
                add_tile_base: true,
            },
            part(rle(0x362F2, 0x24, 0x30), ATTRIBUTES),
        ],
        purpose: "menu screen A (bank 6 entry $8006)",
        certainty: DecodeVerified,
    },
    Picture {
        path: "pictures/menu-b.ron",
        parts: &[
            part(rle(0x36788, 0xB1, 0x240), 0x2080),
            part(rle(0x36839, 0x16, 0x28), 0x23C8),
        ],
        purpose: "menu screen B (bank 6 entry $8009)",
        certainty: DecodeVerified,
    },
    Picture {
        path: "pictures/menu-c.ron",
        parts: &[
            part(rle(0x36540, 0x1E8, 0x300), NAMETABLE),
            part(raw(0x36728, 0x31), ATTRIBUTES),
        ],
        purpose: "menu screen C (bank 6 entry $800C): further message screens",
        certainty: DecodeVerified,
    },
    Picture {
        path: "pictures/menu-d.ron",
        parts: &[
            part(rle(0x36316, 0x22A, 0x300), NAMETABLE),
            part(raw(0x36758, 0x31), ATTRIBUTES),
        ],
        purpose: "menu screen D (bank 6 entry $800F): the equipment/shop frame",
        certainty: DecodeVerified,
    },
    Picture {
        path: "pictures/ending.ron",
        parts: &[
            part(rle(0x3D379, 0x2EA, 0x3C1), NAMETABLE),
            part(rle(0x3F11D, 0x2E, 0x40), ATTRIBUTES),
        ],
        purpose: "ending picture",
        certainty: DecodeVerified,
    },
];

/// Attract sequence: animation frames of the floating Jetman picture (graphics.md 4.5).
pub mod attract_frames {
    use super::{Blob, raw};

    pub const PATH: &str = "pictures/attract-jetman-frames.ron";
    pub const FRAMES: usize = 11;
    /// Nametable bytes: 11 frames × 11 rows × 16 bytes.
    pub const NAMETABLE: Blob = raw(0x3234A, 0x790);
    pub const NAMETABLE_ROW: usize = 16;
    /// Attribute bytes: 12 per frame.
    pub const ATTRIBUTES: Blob = raw(0x32ADA, 0x84);
    pub const ATTRIBUTE_ROW: usize = 12;
}

/// Palettes (graphics.md section 7).
pub mod palettes {
    /// Number of palette presets in bank 0 (7.2).
    pub const PRESETS: usize = 28;
    /// 16-bit little-endian bank 0 addresses of each preset's colours.
    pub const PRESET_POINTERS: usize = 0x0771E;
    /// First palette entry each preset writes.
    pub const PRESET_FIRST_ENTRY: usize = 0x07756;
    /// Number of entries each preset writes.
    pub const PRESET_COUNT: usize = 0x07772;
    /// Where all preset colours lie, inclusive start and exclusive end.
    pub const PRESET_DATA: (usize, usize) = (0x07526, 0x0771E);

    /// Levels with approach-screen and surface-picture colours: the 14 planets (7.3).
    pub const PLANETS: usize = 14;
    /// Approach screen globe: 3 colours per level.
    pub const APPROACH: usize = 0x07463;
    pub const APPROACH_COLOURS: usize = 3;
    /// Surface picture: 12 colours per level.
    pub const SURFACE: usize = 0x0748A;
    pub const SURFACE_COLOURS: usize = 12;

    /// Level headers (graphics.md 6.2 and 7.3): 42 levels, bank 2 address of each header
    /// as low bytes then high bytes.
    pub const LEVELS: usize = 42;
    pub const HEADER_POINTERS_LOW: usize = 0x1156E;
    pub const HEADER_POINTERS_HIGH: usize = 0x11598;
    /// Where all level headers lie, inclusive start and exclusive end.
    pub const HEADERS: (usize, usize) = (0x115C8, 0x11A36);
    /// Bytes 12-23 of a header are the level's gameplay colours.
    pub const HEADER_COLOURS: (usize, usize) = (12, 24);

    /// Screen of graphics.md 5.10: two 15-colour palettes for entries 1-15.
    pub const FRAMES_SCREEN: [usize; 2] = [0x1132E, 0x1133D];
    pub const FRAMES_SCREEN_COLOURS: usize = 15;
    /// Bank 7 backdrop: 16 colours for entries 16-31.
    pub const BACKDROP: usize = 0x3832A;
    pub const BACKDROP_COLOURS: usize = 16;
    /// Bank 7 backdrop: then 3 entries of 4 bytes for entries 12-15.
    pub const BACKDROP_VARIANTS: usize = 0x3833A;
    pub const BACKDROP_VARIANT_COUNT: usize = 3;
    pub const BACKDROP_VARIANT_COLOURS: usize = 4;

    /// Title logo fade-in (7.4): six columns of 7 colours, one per step.
    pub const TITLE_FADE: usize = 0x077B3;
    pub const TITLE_FADE_COLUMNS: usize = 6;
    pub const TITLE_FADE_STEPS: usize = 7;
    /// The 3 colours entries 9-11 take after the last step (three separate bytes).
    pub const TITLE_FADE_END: [usize; 3] = [0x00276, 0x0027A, 0x0027E];
    /// The analysis screen's pairs of colours for entries 9 and 10 (7.4): 17 pairs by list.
    pub const ANALYSIS_PAIRS: usize = 0x05599;
    pub const ANALYSIS_PAIR_COUNT: usize = 17;
    /// The sun flicker's second colour (7.4, scenes.md 2.1).
    pub const TITLE_FLASH_WHITE: usize = 0x0019F;
}

/// The interplanetary flight's data (scenes.md 5).
pub mod flight {
    pub const MESSAGE_PATH: &str = "pictures/flight.ron";
    /// Printer message 49: the mothership picture in picture mode (text.md 2.1, 3).
    pub const MESSAGE: usize = 0x0A40B;
    pub const MESSAGE_LENGTH: usize = 0x5B;
    pub const TABLES_PATH: &str = "tables/flight.ron";
    /// The 12 colours of the lit window (scenes.md 5.1).
    pub const WINDOW: usize = 0x00996;
    pub const WINDOW_LENGTH: usize = 12;
    /// The sine table of the ring of big stars (scenes.md 5.4a).
    pub const SINE: usize = 0x38C74;
    pub const SINE_LENGTH: usize = 128;
}

/// Small tables the original builds tiles from (graphics.md section 3).
pub mod generated {
    pub const PATH: &str = "tables/generated-tiles.ron";
    /// Gauge pieces: the 8 byte values gauge tiles are filled with, chosen by gauge level.
    pub const GAUGE_ROWS: usize = 0x01973;
    /// Twinkling star: 8 one-bit-per-column masks.
    pub const STAR_MASKS: usize = 0x02112;
    pub const LENGTH: usize = 8;

    /// The backdrop interlude's tables (scenes.md 11.1): the 15 bars' widths and first columns,
    /// the four 32-tile phase rows (the tile is the byte plus 1), and for each of the three
    /// backdrops 32 entries of the gems', rocks' and mines' x and type (one entry more, which
    /// the last backdrop's last spawn reads).
    pub const BAR_WIDTHS: usize = 0x38413;
    pub const BAR_COLUMNS: usize = 0x38422;
    pub const BARS: usize = 15;
    pub const PHASE_TILES: usize = 0x3A80D;
    pub const PHASE_TILES_LENGTH: usize = 128;
    pub const SPAWN_X: usize = 0x38033;
    pub const SPAWN_TYPES: usize = 0x38094;
    pub const SPAWN_ENTRIES: usize = 97;
}
