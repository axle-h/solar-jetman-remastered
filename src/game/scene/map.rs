//! The planet map (`docs/spec/scenes.md` 13.2): a top view of the whole planet, one tile for
//! every chunk of the map, with the pod, the mothership and (with the super mapping device)
//! the items marked. Select opens it from the pause, Start returns to the level.

use super::ppu::{Screen, Sprite};
use super::{Clock, FONT_LOADS};
use crate::game::gameplay::input::Buttons;
use crate::game::graphics::{Graphics, MissingGraphics, Palette, gameplay_background_loads};
use crate::game::hud::Hud;
use crate::game::level::{Level, World};

const FRAME_AND_FONT: &str = "tiles/menus/frame-and-font.png";
const FRAME_PICTURE: &str = "pictures/menu-a.ron";
const DOT_TILES: (&str, u16) = ("tiles/title/sprites-0800.png", 0x0800);
const SHIP_TILES: (&str, u16) = ("tiles/surface/extra-picture-1.png", 0x0A80);
const TITLE_SPRITES: [(&str, u16); 2] = [
    ("tiles/title/sprites-0860.png", 0x0860),
    ("tiles/title/sprites-0b80.png", 0x0B80),
];
/// The palette preset of the window (graphics.md 7.2).
const PRESET_MAP: usize = 8;
/// The tile base of the frame art: $62 on the planets whose surface picture number AND 3 is 2,
/// $54 on the others.
const BASE_PICTURE_2: u8 = 0x62;
const BASE_OTHER: u8 = 0x54;
/// The window: 16 × 16 chunks at nametable rows 4-19, columns 8-23.
const WINDOW: usize = 16;
const WINDOW_ROW: usize = 4;
const WINDOW_COLUMN: usize = 8;
/// Screen position of the window's top-left pixel, and where a block's dot goes.
const WINDOW_X: i32 = 64;
const DOT_Y: i32 = 29;
const DOT_TILE: u8 = 0x85;
const SHIP_TILES_FIRST: u8 = 0xAD;
/// The mothership's two sprites are 6 left and 2 right of its position.
const SHIP_LEFT: i32 = -6;
const SHIP_RIGHT: i32 = 2;
/// Chunk numbers whose colour-2 pixels are dropped (scenes.md 13.2).
const HOLLOW_CHUNKS: std::ops::RangeInclusive<u8> = 44..=80;

/// The colours of the player's dot: palette 1, a step every 8 frames; and the items' dots:
/// palette 2, a step every 16 frames (scenes.md 13.2).
const PLAYER_STEPS: [[u8; 3]; 4] = [
    [0x30, 0x20, 0x00],
    [0x22, 0x12, 0x0F],
    [0x14, 0x04, 0x0F],
    [0x28, 0x18, 0x08],
];
const ITEM_STEPS: [[u8; 3]; 2] = [[0x30, 0x20, 0x00], [0x0F, 0x00, 0x30]];

/// What a pass of the map did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MapOutcome {
    Running,
    /// Start: back to the level.
    Leave,
}

/// A marker on the map: a world position in pixels.
pub type Spot = (u16, u16);

/// What the map shows besides the planet.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MapFacts {
    /// The original's panel split, a line higher than in play (Q37).
    pub original_rules: bool,
    /// The player (the pod or Jetman).
    pub player: Spot,
    /// The items still to find, in the items list's order from entry 1 on, with the live
    /// position of the one on the beam.
    pub items: Vec<Spot>,
    /// Whether the super mapping device is owned: the items show.
    pub super_device: bool,
    /// The status panel as in play.
    pub hud: Hud,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PlanetMap {
    screen: Screen,
}

impl PlanetMap {
    /// Builds the screen for `level` over the gameplay's palette (`palette`) and tiles.
    pub fn new(
        graphics: &Graphics,
        world: &World,
        level: &Level,
        palette: &Palette,
        facts: &MapFacts,
    ) -> Result<PlanetMap, MissingGraphics> {
        let mut screen = Screen::default();
        screen.palette = *palette;
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        screen
            .chr
            .apply(graphics, &gameplay_background_loads(level))?;
        let base = if level.block_set == 2 {
            BASE_PICTURE_2
        } else {
            BASE_OTHER
        };
        screen.chr.apply(
            graphics,
            &[crate::game::graphics::ChrLoad {
                stream: FRAME_AND_FONT,
                range: None,
                address: 0x1000 + 16 * u16::from(base),
            }],
        )?;
        // The ship's tiles come last: the title's sprite sets reach over their place.
        let mut loads = FONT_LOADS[..2].to_vec();
        loads.push(DOT_TILES);
        loads.extend(TITLE_SPRITES);
        loads.push(SHIP_TILES);
        screen.load_tiles(graphics, &loads)?;

        // The frame: its nametable plus the tile base, its attributes.
        let plain = graphics
            .pictures
            .get(FRAME_PICTURE)
            .ok_or_else(|| MissingGraphics(FRAME_PICTURE.to_owned()))?;
        let based = graphics
            .based_pictures
            .get(FRAME_PICTURE)
            .ok_or_else(|| MissingGraphics(format!("{FRAME_PICTURE} with its tile base")))?;
        for write in based {
            let bytes: Vec<u8> = write.bytes.iter().map(|b| b.wrapping_add(base)).collect();
            screen.load_picture(&[crate::game::graphics::PictureWrite {
                address: write.address,
                bytes,
            }]);
        }
        screen.load_picture(plain);

        // The colours of the window: preset 8, then the cycled ones for the sprites.
        screen.palette.apply(
            graphics
                .presets
                .get(PRESET_MAP)
                .ok_or_else(|| MissingGraphics(format!("palette preset {PRESET_MAP}")))?,
        );

        // The window.
        let (width, height) = (usize::from(level.width), usize::from(level.height));
        let map = world.map(level);
        let chunks: Vec<u8> = (0..height)
            .flat_map(|row| (0..width).map(move |column| (column, row)))
            .filter_map(|(column, row)| map.chunks.get(column, row))
            .collect();
        let used = chunks.iter().copied().filter(|&c| c >= 3);
        if let (Some(low), Some(high)) = (used.clone().min(), used.max()) {
            for chunk in low..=high {
                let tile = chunk_tile(world, level, chunk);
                screen
                    .chr
                    .write(0x1000 + 16 * (u16::from(chunk - low) + 1), &tile);
            }
            let (px, py) = (
                usize::from(facts.player.0) / 128,
                usize::from(facts.player.1) / 128,
            );
            let (cx0, cy0) = (origin(px, width), origin(py, height));
            for row in 0..WINDOW {
                for column in 0..WINDOW {
                    let (cx, cy) = (cx0 + column, cy0 + row);
                    // Outside the map the window is empty (docs/deviations.md "Planet map
                    // wrap").
                    let tile = if cx < width && cy < height {
                        match map.chunks.get(cx, cy) {
                            Some(c) if c >= 3 => c - low + 1,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    screen.set_tile(WINDOW_COLUMN + column, WINDOW_ROW + row, tile);
                }
            }
            Self::place_sprites(&mut screen, level, facts, (cx0, cy0));
        } else {
            Self::place_sprites(&mut screen, level, facts, (0, 0));
        }

        // The panel as it stood in play.
        screen.panel = Some(facts.hud.panel_page());
        // The same split as in play; the original's sits a line higher (Q37).
        screen.panel_lift = usize::from(facts.original_rules);
        for (address, level) in [(0x1FA0, facts.hud.fuel), (0x1FB0, facts.hud.weapon)] {
            let fill = usize::from(level % 16 / 2) & 7;
            screen.chr.write_gauge(address, &graphics.gauge_rows, fill);
        }
        screen.sprite_table = 0;
        screen.background_table = 1;
        screen.refresh();
        let mut map_scene = PlanetMap { screen };
        map_scene.cycle_colours(0);
        Ok(map_scene)
    }

    /// The markers (scenes.md 13.2): the player, the items, then the mothership behind the
    /// background.
    fn place_sprites(screen: &mut Screen, level: &Level, facts: &MapFacts, origin: (usize, usize)) {
        let (cx0, cy0) = (origin.0 as i32, origin.1 as i32);
        let at = |(x, y): Spot| {
            let (u, v) = (i32::from(x) / 16 - 8 * cx0, i32::from(y) / 16 - 8 * cy0);
            ((0..128).contains(&u) && (0..128).contains(&v)).then_some((WINDOW_X + u, DOT_Y + v))
        };
        let mut sprites = Vec::new();
        if let Some((x, y)) = at(facts.player) {
            sprites.push(Sprite::new(x, y, DOT_TILE, 1));
        }
        if facts.super_device {
            for &spot in facts.items.iter().rev() {
                if let Some((x, y)) = at(spot) {
                    sprites.push(Sprite::new(x, y, DOT_TILE, 2));
                }
            }
        }
        if let Some((x, y)) = at((level.home.x, level.home.y)) {
            for (dx, tile) in [
                (SHIP_LEFT, SHIP_TILES_FIRST),
                (SHIP_RIGHT, SHIP_TILES_FIRST + 1),
            ] {
                let mut ship = Sprite::new(x + dx, y, tile, 0);
                ship.behind = true;
                sprites.push(ship);
            }
        }
        screen.sprites = sprites;
    }

    /// The dots' colours for frame counter `t`.
    fn cycle_colours(&mut self, t: u32) {
        let player = PLAYER_STEPS[(t >> 3) as usize & 3];
        let items = ITEM_STEPS[(t >> 4) as usize & 1];
        for (i, &colour) in player.iter().enumerate() {
            self.screen.palette.0[21 + i] = colour;
        }
        for (i, &colour) in items.iter().enumerate() {
            self.screen.palette.0[25 + i] = colour;
        }
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// One pass: the counter runs on, the colours cycle, Start leaves.
    pub fn step(&mut self, clock: &mut Clock, pressed: Buttons) -> MapOutcome {
        // The colours the console shows are those of the counter before this pass's increment
        // (the palette is written a frame after the pass).
        self.cycle_colours(clock.counter);
        clock.counter = (clock.counter + 1) & 0xFF_FFFF;
        clock.job = (clock.job + 1) & 7;
        if pressed.has(Buttons::START) {
            MapOutcome::Leave
        } else {
            MapOutcome::Running
        }
    }
}

/// The first chunk of the window along one axis: the player's chunk less 8, held so that the
/// window stays on the map (a map narrower than the window starts at 0).
fn origin(player: usize, size: usize) -> usize {
    let a = player.saturating_sub(8);
    if a + 16 < size.saturating_sub(1) {
        a
    } else {
        size.saturating_sub(16)
    }
}

/// The 8 × 8 picture of a chunk, one pixel per block (scenes.md 13.2), as tile bytes.
fn chunk_tile(world: &World, level: &Level, chunk: u8) -> [u8; 16] {
    let mut tile = [0u8; 16];
    let classes = &world.collision.block_classes[usize::from(level.block_set)];
    let blocks = world.blocks.chunks.get(usize::from(chunk));
    for row in 0..8 {
        for column in 0..8 {
            let block = blocks
                .and_then(|b| b.get(row * 8 + column).copied())
                .unwrap_or(0);
            let class = if block == 0 {
                0
            } else {
                classes.get(usize::from(block)).copied().unwrap_or(0)
            };
            let mut value = match (block, class) {
                (0, _) => 0,
                (_, 0) => 2,
                _ => 1,
            };
            if value == 2 && HOLLOW_CHUNKS.contains(&chunk) {
                value = 0;
            }
            tile[row] |= (value & 1) << (7 - column);
            tile[8 + row] |= (value >> 1) << (7 - column);
        }
    }
    tile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::graphics::PictureWrite;
    use crate::game::hex::{HexGrid, HexRows};
    use crate::game::level::ChunkMap;
    use crate::game::level::tests::synthetic_world;

    #[test]
    fn the_window_keeps_the_player_in_view_and_on_the_map() {
        // The player's chunk less 8, held to the map.
        assert_eq!(origin(0, 32), 0);
        assert_eq!(origin(10, 32), 2);
        assert_eq!(origin(31, 32), 16);
        // At W - 17 the window is moved one chunk: the last 16 columns show.
        assert_eq!(origin(25, 33), 17);
        assert_eq!(origin(24, 33), 17);
        assert_eq!(origin(23, 33), 15);
        assert_eq!(origin(10, 33), 2);
        // A narrow map starts at 0.
        assert_eq!(origin(3, 8), 0);
        assert_eq!(origin(20, 15), 0);
    }

    fn graphics() -> Graphics {
        let mut graphics = Graphics::default();
        for (path, tiles) in [
            ("tiles/font/digits.png", 12),
            ("tiles/font/letters.png", 31),
            ("tiles/hud/status-panel.png", 27),
            ("tiles/hud/status-panel-1d60.png", 5),
            ("tiles/terrain/types-1-6.png", 256),
            ("tiles/terrain/types-0-4.png", 181),
            ("tiles/terrain/types-2-3-5.png", 191),
            ("tiles/terrain/common.png", 51),
            FRAME_AND_FONT_TILES,
            (DOT_TILES.0, 8),
            (TITLE_SPRITES[0].0, 8),
            (TITLE_SPRITES[1].0, 2),
            (SHIP_TILES.0, 8),
        ] {
            graphics
                .tile_streams
                .insert(path.to_owned(), vec![0; tiles * 16]);
        }
        graphics.presets = vec![
            PalettePreset {
                first_entry: 9,
                colours: vec![0x3C, 0x2C, 0x1C, 0, 0x31, 0x21, 0x11],
            };
            9
        ];
        graphics.pictures.insert(
            FRAME_PICTURE.to_owned(),
            vec![PictureWrite {
                address: 0x23C0,
                bytes: vec![0; 48],
            }],
        );
        graphics.based_pictures.insert(
            FRAME_PICTURE.to_owned(),
            vec![PictureWrite {
                address: 0x2000,
                bytes: vec![0; 768],
            }],
        );
        graphics.gauge_rows = vec![0; 8];
        graphics
    }

    const FRAME_AND_FONT_TILES: (&str, usize) = (FRAME_AND_FONT, 64);

    use crate::game::graphics::PalettePreset;

    /// A 3 × 2 map of chunks 3, 5 and 4 with chunk 5 hollow (colour 2 kept: it is not in
    /// 44-80) and chunk 3 half solid.
    fn world() -> World {
        let mut world = synthetic_world();
        let chunks = HexGrid::new(3, vec![3, 5, 0, 4, 3, 5]);
        let palettes = crate::game::hex::DigitGrid::new(3, vec![0; 6]);
        world
            .maps
            .insert("small".to_owned(), ChunkMap { chunks, palettes });
        for level in &mut world.levels {
            level.width = 3;
            level.height = 2;
        }
        let mut half = vec![0u8; 64];
        for (i, block) in half.iter_mut().enumerate() {
            *block = if i % 8 < 4 { 1 } else { 0 };
        }
        world.blocks.chunks[3] = HexRows(half);
        world.blocks.chunks[4] = HexRows(vec![1; 64]);
        world.blocks.chunks[5] = HexRows(vec![0; 64]);
        world
    }

    #[test]
    fn a_chunk_is_a_tile_of_one_pixel_per_block() {
        let world = world();
        let level = &world.levels[0];
        // Chunk 3: the left four blocks are solid (class 1 = colour 1), the right four empty.
        assert_eq!(chunk_tile(&world, level, 3)[..2], [0xF0, 0xF0]);
        assert_eq!(chunk_tile(&world, level, 3)[8..10], [0, 0]);
        // Block 1 of the synthetic world has class 1; make it class 0 and it is colour 2.
        let mut world = world;
        world.collision.block_classes[0].0[1] = 0;
        let level = &world.levels[0];
        assert_eq!(chunk_tile(&world, level, 4)[0], 0x00);
        assert_eq!(chunk_tile(&world, level, 4)[8], 0xFF);
        // Chunks 44-80 drop colour 2.
        world.blocks.chunks[50] = HexRows(vec![1; 64]);
        assert_eq!(chunk_tile(&world, &world.levels[0], 50), [0; 16]);
    }

    #[test]
    fn the_window_numbers_tiles_from_the_lowest_chunk_and_marks_the_player() {
        let world = world();
        let level = &world.levels[0];
        let facts = MapFacts {
            original_rules: false,
            // Block (3, 5) of the map: chunk (0, 0).
            player: (3 * 16, 5 * 16),
            items: vec![(16, 32), (5000, 5000)],
            super_device: true,
            hud: Hud {
                fuel: 0xE0,
                weapon: 0xE0,
                jetpods: 4,
                lives: 4,
                score: 0,
                negative: false,
            },
        };
        let map = PlanetMap::new(&graphics(), &world, level, &Palette([0x0F; 32]), &facts).unwrap();
        let screen = map.screen();
        // Chunks 3, 4, 5 are tiles 1, 2, 3; 0 is blank; the rest of the window is blank too.
        let row = |r: usize| -> Vec<u8> {
            (0..4)
                .map(|c| screen.tile(WINDOW_COLUMN + c, WINDOW_ROW + r))
                .collect()
        };
        assert_eq!(row(0), [1, 3, 0, 0]);
        assert_eq!(row(1), [2, 1, 3, 0]);
        assert_eq!(row(2), [0, 0, 0, 0]);
        // The player's dot, the item inside the window, the mothership behind.
        let at: Vec<_> = screen
            .sprites
            .iter()
            .map(|s| (s.x, s.y, s.tile, s.palette, s.behind))
            .collect();
        assert_eq!(
            at,
            [
                (64 + 3, 29 + 5, DOT_TILE, 1, false),
                (64 + 1, 29 + 2, DOT_TILE, 2, false),
                (64 + 100 / 16 - 6, 29 + 120 / 16, 0xAD, 0, true),
                (64 + 100 / 16 + 2, 29 + 120 / 16, 0xAE, 0, true),
            ]
        );
    }

    #[test]
    fn without_the_super_device_there_are_no_item_dots_and_start_leaves() {
        let world = world();
        let level = &world.levels[0];
        let facts = MapFacts {
            original_rules: false,
            player: (0, 0),
            items: vec![(16, 32)],
            super_device: false,
            hud: Hud {
                fuel: 0,
                weapon: 0,
                jetpods: 0,
                lives: 0,
                score: 0,
                negative: false,
            },
        };
        let mut map =
            PlanetMap::new(&graphics(), &world, level, &Palette([0x0F; 32]), &facts).unwrap();
        assert_eq!(
            map.screen()
                .sprites
                .iter()
                .filter(|s| s.palette == 2)
                .count(),
            0
        );
        let mut clock = Clock::default();
        assert_eq!(
            map.step(&mut clock, Buttons(Buttons::A)),
            MapOutcome::Running
        );
        assert_eq!(
            map.step(&mut clock, Buttons(Buttons::START)),
            MapOutcome::Leave
        );
    }

    #[test]
    fn the_dots_cycle_their_colours() {
        let world = world();
        let level = &world.levels[0];
        let facts = MapFacts {
            original_rules: false,
            player: (0, 0),
            items: Vec::new(),
            super_device: true,
            hud: Hud {
                fuel: 0,
                weapon: 0,
                jetpods: 0,
                lives: 0,
                score: 0,
                negative: false,
            },
        };
        let mut map =
            PlanetMap::new(&graphics(), &world, level, &Palette([0x0F; 32]), &facts).unwrap();
        let mut clock = Clock::default();
        let mut seen = Vec::new();
        for _ in 0..33 {
            map.step(&mut clock, Buttons(0));
            seen.push((map.screen().palette.0[21], map.screen().palette.0[25]));
        }
        assert_eq!(seen[0], (0x30, 0x30));
        assert_eq!(seen[7], (0x30, 0x30));
        assert_eq!(seen[8], (0x22, 0x30));
        assert_eq!(seen[16], (0x14, 0x0F));
        assert_eq!(seen[32], (0x30, 0x30));
    }
}
