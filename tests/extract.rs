//! The level data extraction against the player's ROM (`$SJ_ROM`, or the first
//! `vendor/rom/*.nes`); each test skips when there is none.

use solar_jetman::game::graphics::LevelGraphics;
use solar_jetman::game::level::{Line, Point, Segment, World};
use solar_jetman::rom::extract::{extract, sprite_file};
use solar_jetman::tools::test_rom as rom;
use std::sync::OnceLock;

fn world() -> Option<&'static World> {
    static WORLD: OnceLock<Option<World>> = OnceLock::new();
    WORLD
        .get_or_init(|| rom().map(|rom| extract(rom).unwrap_or_else(|error| panic!("{error}"))))
        .as_ref()
}

/// The import's level data reads back as what the extraction made.
#[test]
fn the_pack_holds_the_extracted_world() {
    let (Some(rom), Some(world)) = (rom(), world()) else {
        return;
    };
    let pack = solar_jetman::rom::import(rom).unwrap();
    assert_eq!(solar_jetman::rom::world(&pack.files).unwrap(), *world);
    assert_eq!(
        solar_jetman::rom::sprites(&pack.files).unwrap(),
        sprite_file(rom).unwrap()
    );
    let periods = solar_jetman::rom::note_periods(&pack.files).unwrap();
    assert_eq!(
        periods,
        solar_jetman::rom::sound::note_periods(rom).unwrap()
    );
}

/// One row of levels.md 3.1: camera x, y, home x, y, W, H, byte 10, graphics type, gravity,
/// object count.
type HeaderRow = (u16, u16, u16, u16, u8, u8, u8, u8, i8, usize);

/// levels.md 3.1: camera, home, W × H, byte 10, graphics type, gravity, object count.
#[test]
fn headers_match_the_spec_table() {
    let Some(world) = world() else { return };
    #[rustfmt::skip]
    let table: [HeaderRow; 29] = [
        (512, 96, 635, 194, 8, 8, 144, 3, 8, 6),
        (1536, 128, 1659, 194, 32, 26, 64, 6, 24, 24),
        (256, 128, 379, 194, 24, 20, 192, 4, 16, 17),
        (384, 96, 507, 194, 40, 15, 144, 2, 8, 21),
        (1792, 128, 1915, 194, 30, 26, 224, 0, 32, 20),
        (1536, 512, 1659, 578, 48, 29, 64, 1, 40, 25),
        (1792, 128, 1915, 194, 36, 32, 192, 3, 32, 25),
        (1408, 128, 1531, 194, 30, 28, 64, 0, -24, 23),
        (896, 512, 1019, 578, 38, 26, 64, 1, 24, 23),
        (1152, 0, 1275, 66, 34, 22, 208, 4, 24, 21),
        (2176, 64, 2299, 194, 40, 22, 64, 1, 56, 24),
        (512, 1088, 635, 1218, 46, 49, 64, 2, 72, 25),
        (2816, 64, 2939, 194, 32, 20, 64, 5, 32, 20),
        (0, 0, 127, 44, 2, 3, 32, 1, 0, 3),
        (0, 0, 128, 128, 2, 8, 40, 1, 0, 3),
        (128, 512, 256, 608, 6, 6, 184, 1, 0, 3),
        (0, 128, 96, 288, 6, 4, 40, 2, 0, 3),
        (0, 0, 128, 128, 6, 4, 240, 0, 0, 3),
        (64, 0, 192, 112, 4, 4, 40, 1, 0, 3),
        (0, 0, 96, 128, 8, 2, 40, 0, 0, 3),
        (384, 192, 512, 256, 6, 6, 24, 2, 0, 3),
        (0, 0, 128, 96, 6, 6, 40, 0, 0, 3),
        (0, 768, 128, 928, 6, 8, 40, 1, 0, 3),
        (1024, 0, 1184, 128, 10, 2, 40, 2, 0, 3),
        (0, 256, 128, 384, 10, 4, 24, 1, 0, 3),
        (256, 256, 368, 384, 6, 6, 40, 0, 0, 3),
        (0, 896, 96, 1024, 56, 12, 128, 1, 0, 3),
        (64, 32, 192, 128, 8, 2, 200, 2, 0, 3),
        (64, 160, 192, 256, 6, 4, 200, 2, 0, 2),
    ];
    let warp = (192, 160, 320, 256, 6, 6, 24, 0, 0, 10);
    for (number, level) in world.levels.iter().enumerate() {
        let row = table.get(number).copied().unwrap_or(warp);
        let actual = (
            level.camera_start.x,
            level.camera_start.y,
            level.home.x,
            level.home.y,
            level.width,
            level.height,
            level.object_margin,
            level.graphics_type,
            level.gravity,
            level.objects.len(),
        );
        assert_eq!(actual, row, "level {number}");
        // Block set: graphics types 2, 3, 5 → 0; 0, 4 → 1; 1, 6 → 2 (levels.md 4.1).
        let set = [1, 2, 0, 0, 1, 0, 2][usize::from(level.graphics_type)];
        assert_eq!(level.block_set, set, "level {number}");
        assert_eq!(
            world.map(level).chunks.height(),
            usize::from(level.height) + 1
        );
    }
    // Planets start with the mothership at home (levels.md 3.3).
    for level in &world.levels[..13] {
        let first = level.objects[0];
        assert_eq!(
            Point {
                x: first.x,
                y: first.y
            },
            level.home
        );
        assert!([21, 22, 24].contains(&first.kind));
    }
    // Level 26 uses level 25's list; the warp levels share one map.
    assert_eq!(world.levels[26].objects, world.levels[25].objects);
    assert!(world.levels[29..].iter().all(|l| l.map == "level-29"));
    // Maps share bytes (levels.md 4.2): level 9's map starts at level 7's row 27 (bank 3:$A40A =
    // $A0E0 + 27 × 30), and the extra row of level 3 is the first row of level 5.
    let row = |name: &str, y: usize| {
        let map = &world.maps[name].chunks;
        map.cells()[y * map.width()..(y + 1) * map.width()].to_vec()
    };
    assert_eq!(row("level-09", 0)[..30], row("level-07", 27)[..30]);
    assert_eq!(row("level-03", 15)[..40], row("level-05", 0)[..40]);
}

#[test]
fn blocks_chunks_and_classes_match_the_spec() {
    let Some(world) = world() else { return };
    // Chunk 0 is all block 0; block 0 of every set is four copies of tile 0 (levels.md 4.3).
    assert!(world.blocks.chunks[0].iter().all(|&b| b == 0));
    for set in &world.blocks.block_sets {
        assert_eq!(set[..4], [0, 0, 0, 0]);
    }
    // Every chunk number used by a map exists, and every block number is in its set.
    for level in &world.levels {
        for &chunk in world.map(level).chunks.cells() {
            assert!(usize::from(chunk) < 246, "level {}", level.number);
            for &block in world.blocks.chunks[usize::from(chunk)].iter() {
                assert!(block < 170, "level {} chunk {chunk}", level.number);
            }
        }
    }
    // levels.md 6.3.
    let classes = &world.collision.classes;
    for empty in [0, 3, 6, 9, 20, 23, 26, 29, 33, 35, 37] {
        assert!(classes[empty].segments.is_empty(), "class {empty}");
    }
    assert_eq!(classes[12], classes[14]);
    assert_eq!(classes[12], classes[16]);
    assert_eq!(
        classes[45].segments,
        [
            Segment {
                direction: 0,
                line: Line::Flat {
                    x0: 0,
                    width: 31,
                    y: 0
                }
            },
            Segment {
                direction: 8,
                line: Line::Wall {
                    x: 15,
                    y0: -2,
                    height: 4
                }
            },
        ]
    );
    assert_eq!(
        classes[11].segments,
        [Segment {
            direction: 13,
            line: Line::Ceiling {
                y_top: 0,
                x0: 9,
                width: 8,
                y_bottom: 19
            }
        }]
    );
    assert_eq!(classes[52].segments.len(), 2);
}

/// Every level renders from the imported graphics without anything missing.
#[test]
fn every_level_has_its_graphics() {
    let (Some(rom), Some(world)) = (rom(), world()) else {
        return;
    };
    let pack = solar_jetman::rom::import(rom).unwrap();
    let graphics = solar_jetman::rom::graphics(&pack.files).unwrap();
    for level in &world.levels {
        let level_graphics = LevelGraphics::new(&graphics, level).unwrap();
        let mut frame = solar_jetman::game::playfield::Frame::default();
        solar_jetman::game::playfield::draw_playfield(
            &mut frame,
            world,
            level,
            &level_graphics,
            solar_jetman::game::view::Viewport::original(level.clamp_camera(level.camera_start)),
        );
        assert!(frame.pixels.iter().all(|&c| c < 0x40));
    }
}

#[test]
fn placement_lists_decode_as_the_spec_says() {
    let Some(world) = world() else { return };
    let data = &world.objects;
    // objects.md 3.2: level 0 has nine objects, #0 at ($0372, $01AF).
    let level0 = &data.placements[0];
    let types: Vec<u8> = level0.iter().map(|p| p.kind).collect();
    assert_eq!(
        types,
        [0x5F, 0x0B, 0x0F, 0x0C, 0x06, 0x08, 0x49, 0x05, 0x0E]
    );
    assert_eq!((level0[0].x, level0[0].y), (0x0372, 0x01AF));
    assert_eq!((level0[1].x, level0[1].y), (0x0122, 0x01A7));
    // Level 1 starts with a group: #0 at ($0B2F, $03F6), #1 at +85/-100, #2 at +50/+26,
    // #3 at -63/+24.
    let level1 = &data.placements[1];
    assert_eq!((level1[0].x, level1[0].y), (0x0B2F, 0x03F6));
    assert_eq!((level1[1].x, level1[1].y), (0x0B2F + 85, 0x03F6 - 100));
    assert_eq!((level1[2].x, level1[2].y), (0x0B2F + 50, 0x03F6 + 26));
    assert_eq!((level1[3].x, level1[3].y), (0x0B2F - 63, 0x03F6 + 24));
    // Counts of the summary table (3.3), and the shared dummy entry of levels 29-41.
    let counts: Vec<usize> = data.placements.iter().map(Vec::len).collect();
    assert_eq!(&counts[..6], [9, 79, 27, 71, 38, 85]);
    assert_eq!(counts.iter().sum::<usize>(), 963);
    assert!(data.placements[29..].iter().all(|list| list.len() == 1));
    // 4.1 and 2.4: the turret of type 01, the egg and the generator.
    assert_eq!(data.type_start(0x01), (0x1B, 0x86));
    assert_eq!(data.type_start(0x5F), (0x9A, 0xB8));
    assert_eq!(data.type_start(0x49), (0xFF, 0xAE));
    assert_eq!(data.initial_hit_points(0x86), 3);
    assert_eq!(data.initial_hit_points(0xB8), 1);
    assert_eq!(data.initial_hit_points(0xA7), 16);
    assert_eq!(data.flags(0x15), 0x61);
    assert_eq!(data.flags(0x86), 0x80);
    // 8: the domed installation of frame $34 is 20 x 21 (half sizes).
    assert_eq!(data.half_size(0x34), (20, 21));
}

#[test]
fn sprite_layouts_and_sets_decode_as_the_spec_says() {
    let Some(rom) = rom() else { return };
    let sprites = sprite_file(rom).unwrap();
    // objects.md 6.1: table A has 193 entries, B 218; A$10-$17 and B$13-$17 are unused.
    assert_eq!((sprites.table_a.len(), sprites.table_b.len()), (193, 218));
    assert!(sprites.table_a[0x10..=0x17].iter().all(Vec::is_empty));
    assert!(sprites.table_b[0x13..=0x17].iter().all(Vec::is_empty));
    assert!(!sprites.table_a[0x1B].is_empty());
    // 6.5a: the satellite's layout keeps the 10 sprites it really has.
    assert_eq!(sprites.table_a[0xBF].len(), 10);
    // graphics.md 6.4: level 0 loads list A = 18 83 9A and list B = 59 5C 8A.
    assert_eq!(&*sprites.sets.lists_a[0], [0x18, 0x83, 0x9A]);
    assert_eq!(&*sprites.sets.lists_b[0], [0x59, 0x5C, 0x8A]);
    let record = sprites
        .sets
        .records_a
        .iter()
        .find(|r| r.first == 0x18)
        .unwrap();
    assert_eq!(
        (record.end, record.address, record.count),
        (0x34, 0xE525, 38)
    );
}
