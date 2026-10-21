use super::entry::{Entry, EntryOutcome};
use super::flight::{Flight, FlightOutcome};
use super::launch::{Launch, LaunchOutcome};
use super::title::{Title, TitleOutcome};
use super::*;
use super::{credits, ending};
use crate::game::graphics::tests::synthetic_graphics;
use crate::game::graphics::{Palette, PictureWrite};
use std::collections::BTreeMap;

/// Synthetic graphics with the streams, pictures and colours the scenes read.
fn graphics() -> Graphics {
    let mut graphics = synthetic_graphics();
    let mut streams: Vec<&str> = FONT_LOADS.iter().map(|&(path, _)| path).collect();
    streams.extend([
        "tiles/title/logo.png",
        "tiles/title/sprites-0800.png",
        "tiles/title/sprites-0860.png",
        "tiles/title/sprites-0b80.png",
        "tiles/sprites/title.png",
        "tiles/attract/space-loonies-logo.png",
        "tiles/menus/frame-and-font.png",
        "tiles/unidentified-b/tiles.png",
        "tiles/launch/pod-type-0.png",
        "tiles/launch/pod-type-1.png",
        "tiles/launch/pod-type-2.png",
        "tiles/launch/sprites.png",
        "tiles/launch/background.png",
    ]);
    streams.extend(
        crate::game::gameplay::sprites::COMMON_SPRITES
            .iter()
            .map(|&(p, _)| p),
    );
    for path in streams {
        graphics
            .tile_streams
            .insert(path.to_owned(), vec![0x55; 64]);
    }
    let write = |address: u16, len: usize| PictureWrite {
        address,
        bytes: vec![0; len],
    };
    graphics.pictures = BTreeMap::from([
        (
            "pictures/title.ron".to_owned(),
            vec![write(0x2000, 0x1C0), write(0x23C0, 32)],
        ),
        (
            "pictures/menu-b.ron".to_owned(),
            vec![write(0x2080, 0x240), write(0x23C8, 0x28)],
        ),
        (
            "pictures/flight.ron".to_owned(),
            vec![write(0x2006, 4), write(0x23CA, 1)],
        ),
        (
            "pictures/launch.ron".to_owned(),
            vec![write(0x2000, 0x3C0), write(0x23C0, 64)],
        ),
    ]);
    graphics.flight_window = (0..12).map(|i| 0x10 + i).collect();
    // A half turn of a sine, as the original's table.
    graphics.flight_sine = (0..128)
        .map(|i| (128.0 * (f64::from(i) * std::f64::consts::PI / 128.0).sin()) as u8)
        .collect();
    graphics.presets = (0..28)
        .map(|p| crate::game::graphics::PalettePreset {
            first_entry: 1,
            colours: vec![p as u8 & 0x3F; 31],
        })
        .collect();
    // The flight's presets (graphics.md 7.2): 1-16, then 17-31 and 16-19 for the flash.
    let preset = |first_entry, colours: Vec<u8>| crate::game::graphics::PalettePreset {
        first_entry,
        colours,
    };
    graphics.presets[1] = preset(1, [vec![0x10; 15], vec![0x0F]].concat());
    graphics.presets[2] = preset(17, vec![0x20; 15]);
    graphics.presets[7] = preset(16, vec![0x30, 0x0F, 0x0F, 0x0F]);
    graphics.title_fade = (0..6)
        .map(|c| (0..7).map(|s| 0x10 + c * 8 + s).collect())
        .collect();
    graphics.title_fade_end = vec![0x25, 0x35, 0x30];
    graphics.title_white = 0x30;
    graphics
}

fn layouts() -> SpriteFile {
    use crate::game::gameplay::sprites::{LAYOUTS_A, LAYOUTS_B, ObjectSets, POD_LAYOUTS};
    SpriteFile {
        pod: vec![vec![]; POD_LAYOUTS],
        table_a: vec![vec![]; LAYOUTS_A],
        table_b: vec![vec![]; LAYOUTS_B],
        sets: ObjectSets {
            lists_a: vec![],
            lists_b: vec![],
            records_a: vec![],
            records_b: vec![],
        },
    }
}

fn none() -> Buttons {
    Buttons::default()
}

/// Runs `passes` passes of a title with nothing pressed.
fn run_title(title: &mut Title, clock: &mut Clock, graphics: &Graphics, passes: u32) {
    for _ in 0..passes {
        let outcome = title.step(graphics, clock, none(), none()).unwrap();
        assert_eq!(outcome, TitleOutcome::Running);
    }
}

#[test]
fn the_title_draws_its_random_numbers_as_the_original() {
    // $5A at the title's first passes in the original (counter 2, 3, ...): the generator starts
    // at zero, the counter at 1 and the job counter at 0.
    let mut clock = Clock {
        counter: 1,
        ..Clock::default()
    };
    let seen: Vec<u8> = (0..5)
        .map(|_| {
            clock.advance(none(), none());
            clock.rng.out
        })
        .collect();
    assert_eq!(seen, [0x11, 0x24, 0x39, 0x50, 0x69]);
}

#[test]
fn the_title_follows_the_original_timeline() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut title = Title::new(&graphics, &mut clock).unwrap();
    assert_eq!(clock.counter, 1);
    assert_eq!(title.substate(), 0);
    // The 56 stars, none removed, the cursor and walker not yet there.
    assert_eq!(title.screen().sprites.len(), 56 - 5); // 5 start off-screen (y 240 and up)
    run_title(&mut title, &mut clock, &graphics, 510);
    assert_eq!(title.substate(), 0);
    run_title(&mut title, &mut clock, &graphics, 1);
    assert_eq!((title.substate(), clock.counter), (1, 0x200));
    // Fade, slide, writing, box, menu: sub-states change at the passes the log shows.
    run_title(&mut title, &mut clock, &graphics, 64);
    assert_eq!((title.substate(), clock.counter), (2, 0x240));
    run_title(&mut title, &mut clock, &graphics, 48);
    assert_eq!((title.substate(), clock.counter), (3, 0x270));
    run_title(&mut title, &mut clock, &graphics, 3);
    assert_eq!(title.screen().scroll, (4, 224));
    assert_eq!(title.substate(), 4);
    run_title(&mut title, &mut clock, &graphics, 27);
    assert_eq!((title.substate(), clock.counter), (5, 0x28E));
    assert!(title.screen().sprites.iter().all(|s| s.behind));
    // The cursor's two sprites come first, one pass into the menu (scenes.md 2.3).
    run_title(&mut title, &mut clock, &graphics, 1);
    // The text and the box are in the page (writes show one pass later).
    assert_eq!(title.screen().tile(12, 18), b'S');
    assert_eq!(title.screen().tile(10, 16), 0x10);
    assert_eq!(title.screen().tile(4, 25), 0x3A);
    let sprites = &title.screen().sprites;
    assert_eq!(
        (sprites[0].x, sprites[0].y, sprites[0].tile),
        (86, 162, 0x85)
    );
    assert_eq!(sprites[1].x, 173);
    // The slide hid the stars that ended below the cut, and the Jetman took two slots.
    let stars = sprites.len() - 2;
    assert!((15..30).contains(&stars), "{stars}");
    assert!(sprites[2..].iter().all(|s| s.behind));
    // The badge starts when the walker has crossed (994 passes), with the counter at 1.
    run_title(&mut title, &mut clock, &graphics, 992);
    assert_eq!(title.substate(), 5);
    run_title(&mut title, &mut clock, &graphics, 1);
    assert_eq!((title.substate(), clock.counter), (6, 1));
    assert_eq!(title.screen().sprites.len(), 62);
    run_title(&mut title, &mut clock, &graphics, 255);
    let outcome = title.step(&graphics, &mut clock, none(), none()).unwrap();
    assert_eq!(outcome, TitleOutcome::Timeout);
}

#[test]
fn the_sun_flickers_and_stars_flash_with_the_counter() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut title = Title::new(&graphics, &mut clock).unwrap();
    // Counter 14 after 13 passes: second half of the cycle, white sun.
    run_title(&mut title, &mut clock, &graphics, 13);
    assert_eq!(title.screen().palette.0[13], 0x30);
    // Counter 16: first pair (a pass later on the screen); one star in palette 3 until counter 22.
    run_title(&mut title, &mut clock, &graphics, 3);
    assert_ne!(title.screen().palette.0[13], 0x30);
    let flashing = |t: &Title| t.screen().sprites.iter().filter(|s| s.palette == 3).count();
    assert_eq!(flashing(&title), 1);
    run_title(&mut title, &mut clock, &graphics, 6);
    assert_eq!(flashing(&title), 0);
}

#[test]
fn start_skips_the_sun_and_select_moves_the_cursor() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut title = Title::new(&graphics, &mut clock).unwrap();
    run_title(&mut title, &mut clock, &graphics, 10);
    let start = Buttons(Buttons::START);
    title.step(&graphics, &mut clock, start, start).unwrap();
    assert_eq!(title.substate(), 1);
    // Only the last fade step is left: bright 8 frames on.
    run_title(&mut title, &mut clock, &graphics, 8);
    assert_eq!(title.screen().palette.0[9], 0x25);
    run_title(&mut title, &mut clock, &graphics, 600);
    assert_eq!(title.substate(), 5);
    let select = Buttons(Buttons::SELECT);
    title.step(&graphics, &mut clock, select, select).unwrap();
    assert_eq!(title.screen().sprites[0].y, 162 + 16);
    title.step(&graphics, &mut clock, select, none()).unwrap();
    assert_eq!(title.screen().sprites[0].y, 162 + 16);
    title.step(&graphics, &mut clock, select, select).unwrap();
    assert_eq!(title.screen().sprites[0].y, 162);
    assert_eq!(
        title.step(&graphics, &mut clock, start, start).unwrap(),
        TitleOutcome::NewGame
    );
    title.step(&graphics, &mut clock, select, select).unwrap();
    assert_eq!(
        title.step(&graphics, &mut clock, start, start).unwrap(),
        TitleOutcome::Continue
    );
}

fn press(entry: &mut Entry, mask: u8) -> EntryOutcome {
    let held = Buttons(mask);
    let outcome = entry.step(held, held);
    entry.step(Buttons(0), Buttons(0));
    outcome
}

#[test]
fn password_cursor_and_letters() {
    let graphics = graphics();
    let mut entry = Entry::new(&graphics, None).unwrap();
    assert_eq!(&entry.letters()[..], b"AAAAAAAAAAAA");
    assert_eq!(entry.screen().tile(10, 12), b'A');
    // Down steps forward, Up back, both wrapping.
    press(&mut entry, Buttons::UP);
    assert_eq!(entry.letters()[0], b'Z');
    press(&mut entry, Buttons::DOWN);
    press(&mut entry, Buttons::DOWN);
    assert_eq!(entry.letters()[0], b'B');
    assert_eq!(entry.screen().tile(10, 12), b'B');
    // The cursor stops at both ends.
    press(&mut entry, Buttons::LEFT);
    assert_eq!(entry.cursor(), 0);
    for _ in 0..14 {
        press(&mut entry, Buttons::RIGHT);
    }
    assert_eq!(entry.cursor(), 11);
    let cursor = entry.screen().sprites[0];
    assert_eq!((cursor.x, cursor.y, cursor.tile), (80 + 88, 101, 0x5C));
}

#[test]
fn password_repeat_after_32_then_every_14() {
    let graphics = graphics();
    let mut entry = Entry::new(&graphics, None).unwrap();
    let down = Buttons(Buttons::DOWN);
    entry.step(down, down);
    assert_eq!(entry.letters()[0], b'B');
    let mut steps = vec![];
    for frame in 1..=80 {
        let before = entry.letters()[0];
        entry.step(down, Buttons(0));
        if entry.letters()[0] != before {
            steps.push(frame);
        }
    }
    assert_eq!(steps, [32, 46, 60, 74]);
    // Up wins when both are held.
    let both = Buttons(Buttons::UP | Buttons::DOWN);
    let before = entry.letters()[0];
    entry.step(both, Buttons(Buttons::UP));
    assert_eq!(entry.letters()[0], before - 1);
}

#[test]
fn password_validation() {
    let graphics = graphics();
    let mut entry = Entry::new(&graphics, None).unwrap();
    // All A: not in the alphabet. A and B leave the screen; Start begins a new game.
    assert_eq!(press(&mut entry, Buttons::A), EntryOutcome::Running);
    assert_eq!(press(&mut entry, Buttons::B), EntryOutcome::Running);
    assert_eq!(press(&mut entry, Buttons::START), EntryOutcome::NewGame);
    // A valid one (the spec's level 5 example).
    entry.set_letters(b"NLDKHKGDMVLB");
    let EntryOutcome::Valid(saved) = press(&mut entry, Buttons::A) else {
        panic!("not accepted");
    };
    assert_eq!(
        (saved.level, saved.lives, saved.score_value()),
        (5, 7, 12345)
    );
    // A direction pressed together with A is ignored.
    entry.set_letters(b"NLDKHKGDMVLB");
    let held = Buttons(Buttons::A | Buttons::DOWN);
    assert!(matches!(entry.step(held, held), EntryOutcome::Valid(_)));
    assert_eq!(entry.letters()[0], b'N');
    // A level above 12 is rejected (docs/deviations.md).
    let high = Saved {
        level: 20,
        ..Saved::NEW_GAME
    };
    entry.set_letters(&high.encode());
    assert_eq!(press(&mut entry, Buttons::A), EntryOutcome::Running);
    assert_eq!(press(&mut entry, Buttons::START), EntryOutcome::NewGame);
}

#[test]
fn the_licence_screen_lasts_512_frames_then_the_title_starts_with_its_song() {
    let assets = Assets {
        world: crate::game::level::tests::synthetic_world(),
        graphics: graphics(),
        layouts: layouts(),
    };
    let mut game = Game::new(&assets, Start::Cold).unwrap();
    assert_eq!(game.scene_name(), "licence");
    assert!(game.take_sound().is_empty());
    for _ in 0..511 {
        game.pass(&assets, none()).unwrap();
    }
    assert_eq!(game.scene_name(), "licence");
    game.pass(&assets, none()).unwrap();
    assert_eq!(game.scene_name(), "title");
    assert_eq!(game.take_sound(), [SoundRequest::SongLooped(1)]);
}

#[test]
fn planet_songs_follow_the_level() {
    assert_eq!(planet_song(0), 8);
    assert_eq!(planet_song(5), 18);
    assert_eq!(planet_song(12), 24);
    assert_eq!(planet_song(14), 10);
    assert_eq!(planet_song(26), 33);
    assert_eq!(planet_song(30), 5);
}

/// Runs the flight for `passes` passes (or until it ends) with nothing pressed.
fn run_flight(flight: &mut Flight, clock: &mut Clock, passes: u32) -> u32 {
    for pass in 1..=passes {
        if flight.step(clock, none(), none()) == FlightOutcome::Ended {
            return pass;
        }
    }
    passes
}

#[test]
fn the_flight_lasts_1343_passes_and_starts_song_4() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut flight = Flight::new(&graphics, &mut clock).unwrap();
    assert_eq!(flight.take_song_request(), Some(SoundRequest::SongIfNew(4)));
    assert_eq!(flight.take_song_request(), None);
    assert_eq!(run_flight(&mut flight, &mut clock, 5000), 1343);
    // c runs from 2 to 1344: five carries and 64 in the low byte (scenes.md 5).
    assert_eq!((clock.counter >> 8, clock.counter & 0xFF), (5, 64));
}

#[test]
fn a_newly_pressed_a_ends_the_flight_at_once() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut flight = Flight::new(&graphics, &mut clock).unwrap();
    run_flight(&mut flight, &mut clock, 100);
    let before = clock.counter;
    let a = Buttons(Buttons::A);
    assert_eq!(flight.step(&mut clock, a, a), FlightOutcome::Ended);
    // Nothing else is done in that pass.
    assert_eq!(clock.counter, before);
    // B does not end it.
    assert_eq!(
        flight.step(&mut clock, Buttons(Buttons::B), Buttons(Buttons::B)),
        FlightOutcome::Running
    );
}

#[test]
fn the_flight_camera_follows_the_verified_points() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut flight = Flight::new(&graphics, &mut clock).unwrap();
    // (c, scroll) from scenes.md 5.2: c is one more than the passes run.
    for (c, scroll) in [
        (2, (112, 139)),
        (21, (113, 138)),
        (51, (121, 130)),
        (101, (137, 114)),
    ] {
        while clock.counter < c {
            flight.step(&mut clock, none(), none());
        }
        assert_eq!(flight.screen().scroll, scroll, "c = {c}");
    }
}

#[test]
fn the_ring_implodes_into_a_white_flash_and_fades_out() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut flight = Flight::new(&graphics, &mut clock).unwrap();
    let mut backdrops = Vec::new();
    for _ in 0..1343 {
        flight.step(&mut clock, none(), none());
        backdrops.push(flight.screen().palette.universal_background());
    }
    // Black, then the flash's white (preset 7's first colour), then one luminance row darker
    // every 8 passes, then black again (scenes.md 5.4, 5.5).
    assert_eq!(backdrops[0], backdrops[1]);
    assert_eq!(backdrops[0], 0x0F);
    let first_flash = backdrops.iter().position(|&c| c != 0x0F).unwrap();
    // The pass with c = 1205 (the original's picture of it is shown one frame later).
    assert_eq!(first_flash + 2, 1205);
    assert_eq!(backdrops[first_flash], 0x30);
    // Then 0x20, 0x10, 0x00 at c = 1280, 1288, 1296, black again at 1304 and on.
    assert_eq!(backdrops[1280 - 2], 0x20);
    assert_eq!(backdrops[1288 - 2], 0x10);
    assert_eq!(backdrops[1296 - 2], 0x00);
    assert_eq!(backdrops[1304 - 2], 0x0F);
    assert_eq!(backdrops[1342], 0x0F);
    assert!(!flight.screen().show_background);
}

#[test]
fn the_flight_window_cycles_through_its_colours() {
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut flight = Flight::new(&graphics, &mut clock).unwrap();
    let mut seen = Vec::new();
    for _ in 0..32 {
        flight.step(&mut clock, none(), none());
        seen.push(flight.screen().palette.0[7]);
    }
    // Index (c AND 31) / 2, minus 12 when that is 12 or more; c = 2 first.
    let table: Vec<u8> = (0..12).map(|i| 0x10 + i).collect();
    for (pass, &colour) in seen.iter().enumerate() {
        let c = pass + 2;
        let mut index = (c & 31) / 2;
        if index >= 12 {
            index -= 12;
        }
        assert_eq!(colour, table[index], "c = {c}");
    }
}

fn launch_layouts() -> SpriteFile {
    use crate::game::gameplay::sprites::Piece;
    let mut layouts = layouts();
    let piece = |dx, dy| Piece {
        dx,
        dy,
        tile: 1,
        attributes: 0,
    };
    for frame in 0xC1..=0xD9 {
        layouts.table_b[frame] = vec![piece(0, 0)];
    }
    layouts
}

/// Runs the launch with nothing pressed until it ends; returns the passes it took.
fn run_launch(launch: &mut Launch, layouts: &SpriteFile, clock: &mut Clock) -> u32 {
    for pass in 1..=1000 {
        if launch.step(layouts, clock, none()) == LaunchOutcome::Ended {
            return pass;
        }
    }
    1000
}

#[test]
fn the_launch_takes_358_passes_when_the_counter_starts_at_92() {
    let graphics = graphics();
    let layouts = launch_layouts();
    let mut clock = Clock {
        counter: 92,
        ..Clock::default()
    };
    let mut launch = Launch::new(&graphics, 0).unwrap();
    launch.show(&layouts);
    // The first pass in which the pod's y goes below 0 ends the scene: pass 357, 358 passes in
    // all (scenes.md 9; the picture stops changing two passes earlier).
    assert_eq!(run_launch(&mut launch, &layouts, &mut clock), 358);
    // The counter rose by one each pass and the generator was left alone.
    assert_eq!(clock.counter, 92 + 358);
    assert_eq!(clock.rng, Clock::default().rng);
}

#[test]
fn any_newly_pressed_button_ends_the_launch() {
    let graphics = graphics();
    let layouts = launch_layouts();
    let mut clock = Clock::default();
    let mut launch = Launch::new(&graphics, 0).unwrap();
    for _ in 0..50 {
        assert_eq!(
            launch.step(&layouts, &mut clock, none()),
            LaunchOutcome::Running
        );
    }
    assert_eq!(
        launch.step(&layouts, &mut clock, Buttons(Buttons::B)),
        LaunchOutcome::Ended
    );
}

#[test]
fn the_pilot_walks_and_the_pod_rises_on_the_spec_timeline() {
    let graphics = graphics();
    let layouts = launch_layouts();
    let mut clock = Clock {
        counter: 92,
        ..Clock::default()
    };
    let mut launch = Launch::new(&graphics, 0).unwrap();
    launch.original_rules = true;
    launch.show(&layouts);
    let sprite_at = |launch: &Launch, x: i32, y: i32| {
        launch.screen().sprites.iter().any(|s| s.x == x && s.y == y)
    };
    // Sprites sit one line below their y and, in the original (Q35), one pixel right of their x.
    // Pass n is the n-th (0-based): the pilot is not there before pass 5 ...
    for _ in 0..5 {
        launch.step(&layouts, &mut clock, none());
    }
    assert!(!sprite_at(&launch, 144, 64));
    launch.step(&layouts, &mut clock, none());
    assert!(sprite_at(&launch, 144, 64));
    // ... walks 4 px every 6 passes ...
    for _ in 0..6 {
        launch.step(&layouts, &mut clock, none());
    }
    assert!(sprite_at(&launch, 148, 64));
    // ... and is gone from pass 131, when the pod starts to rise.
    while !launch.screen().sprites.iter().all(|s| s.x != 65) {
        launch.step(&layouts, &mut clock, none());
    }
    for _ in 0..200 {
        launch.step(&layouts, &mut clock, none());
    }
    assert!(!launch.screen().sprites.iter().any(|s| s.x == 144));
}

#[test]
fn the_remake_draws_the_launch_without_the_drawers_stray_pixel() {
    let graphics = graphics();
    let layouts = launch_layouts();
    let mut clock = Clock {
        counter: 92,
        ..Clock::default()
    };
    let mut launch = Launch::new(&graphics, 0).unwrap();
    launch.show(&layouts);
    for _ in 0..6 {
        launch.step(&layouts, &mut clock, none());
    }
    // Q35: the pilot's first sprite is at x = 143 where the original has 144.
    let at = |launch: &Launch, x: i32| {
        launch
            .screen()
            .sprites
            .iter()
            .any(|s| s.x == x && s.y == 64)
    };
    assert!(at(&launch, 143));
    assert!(!at(&launch, 144));
}

#[test]
fn the_suns_camera_follows_the_original_scroll() {
    // The scroll after k passes (scenes.md 2.2's table: frame 533 + k).
    let table: [(u32, (usize, usize)); 16] = [
        (0, (88, 236)),
        (10, (85, 213)),
        (20, (78, 185)),
        (30, (67, 158)),
        (40, (50, 137)),
        (50, (29, 125)),
        (60, (4, 120)),
        (70, (236, 122)),
        (80, (220, 129)),
        (100, (212, 156)),
        (120, (226, 196)),
        (150, (20, 209)),
        (200, (9, 156)),
        (250, (5, 189)),
        (300, (255, 176)),
        (350, (5, 177)),
    ];
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut title = Title::new(&graphics, &mut clock).unwrap();
    let mut passes = 0;
    for (k, scroll) in table {
        let want = k + 1;
        run_title(&mut title, &mut clock, &graphics, want - passes);
        passes = want;
        assert_eq!(title.screen().scroll, scroll, "k = {k}");
    }
}

#[test]
fn the_menu_jetman_follows_the_recorded_samples() {
    // Sprite-memory y of his bottom row at s = 18, 22, ... (scenes.md 2.3's table, first rows) and
    // his flame (player.md 4.8): rising frames show tile $11/$12 by bit 1 of the counter.
    let graphics = graphics();
    let mut clock = Clock::default();
    let mut title = Title::new(&graphics, &mut clock).unwrap();
    run_title(
        &mut title,
        &mut clock,
        &graphics,
        510 + 1 + 64 + 48 + 3 + 27,
    );
    let bottom_row = |t: &Title| {
        t.screen()
            .sprites
            .iter()
            .filter(|s| matches!(s.tile, 0x0C..=0x0F))
            .map(|s| (s.y - 1, s.tile))
            .next()
    };
    let first = [
        137, 140, 144, 148, 152, 156, 160, 164, 168, 172, 176, 180, 184, 188, 192, 196,
    ];
    let mut s = 0;
    for (n, y) in first.iter().enumerate() {
        let target = 18 + 4 * n as u32;
        run_title(&mut title, &mut clock, &graphics, target - s);
        s = target;
        let (got, tile) = bottom_row(&title).unwrap();
        assert_eq!((got, tile), (*y, 0x0E), "s = {target}");
    }
    // The first rise (s 102-145) has the flame; the fall after it keeps it for 2 passes.
    run_title(&mut title, &mut clock, &graphics, 102 - s);
    let flame = |t: &Title| {
        t.screen()
            .sprites
            .iter()
            .find(|s| s.palette == 1 && s.tile >= 0x11 && s.tile <= 0x12)
            .map(|s| (s.x, s.y, s.tile))
    };
    assert_eq!(flame(&title), Some((20, 210, 0x11)));
    run_title(&mut title, &mut clock, &graphics, 146 - 102);
    assert_eq!(flame(&title), Some((31, 188, 0x12)));
    run_title(&mut title, &mut clock, &graphics, 2);
    assert_eq!(flame(&title), None);
}

#[test]
fn approach_songs_and_planet_flags_follow_the_levels() {
    // scenes.md 15: 7 + 2m, 8 from planet 12 on; game-flow.md 6: planet 12 shares planet 10's flag,
    // and a second or third level uses its planet's.
    assert_eq!([0, 5, 11, 12].map(super::approach_song), [7, 17, 29, 23]);
    assert_eq!(
        [0, 10, 12, 13, 25, 29, 41].map(super::flag_bit),
        [0, 10, 10, 0, 10, 0, 10]
    );
}

#[test]
fn the_message_catalogue_has_its_positions_and_typed_parts() {
    use super::strings::message;
    let shields = message(7).unwrap();
    assert!(shields.immediate.is_empty());
    assert_eq!((shields.typed[0].column, shields.typed[0].row), (7, 15));
    let analysis = message(28 + 2).unwrap();
    assert_eq!(analysis.immediate[1].text, "HYDROGEN");
    let crystal = message(25).unwrap();
    assert_eq!(crystal.immediate.len(), 2);
    assert_eq!(crystal.typed.len(), 2);
    assert!(message(13).is_none());
}

/// Synthetic graphics for the bonus flight and the badge: the streams they load and the
/// backdrop's tables (bars of one column each, three kinds of objects in turn).
fn bonus_graphics() -> Graphics {
    let mut graphics = graphics();
    for path in [
        "tiles/sprites/pod-type-0.png",
        "tiles/backdrop/table-0.png",
        "tiles/backdrop/table-1.png",
        "tiles/hud/status-panel.png",
        "tiles/hud/status-panel-1d60.png",
    ] {
        graphics
            .tile_streams
            .insert(path.to_owned(), vec![0x55; 64]);
    }
    graphics.gauge_rows = vec![0; 8];
    graphics.backdrop = crate::game::graphics::Backdrop {
        sprite_colours: vec![0x0F; 16],
        variants: vec![vec![0x0F; 4]; 3],
        bar_widths: vec![1; 15],
        bar_columns: (1..16).collect(),
        phase_tiles: (0..128).collect(),
        spawn_x: (0..97).map(|i| 16 + 2 * i as u8).collect(),
        spawn_types: (0..97).map(|i| 1 + i as u8 % 3).collect(),
    };
    graphics
}

fn hud() -> crate::game::hud::Hud {
    crate::game::hud::Hud {
        fuel: 100,
        weapon: 100,
        jetpods: 4,
        lives: 4,
        score: 1000,
        negative: false,
    }
}

fn bonus_flight(graphics: &Graphics) -> interlude::Interlude {
    interlude::Interlude::new(graphics, 0, 0, 104, &Palette([0x0F; 32]), hud(), false).unwrap()
}

#[test]
fn the_bonus_flight_lasts_2048_steps_whatever_the_player_does() {
    let graphics = bonus_graphics();
    let layouts = layouts();
    for held in [0, Buttons::LEFT, Buttons::RIGHT | Buttons::UP] {
        let mut flight = bonus_flight(&graphics);
        let mut clock = Clock::default();
        let mut steps = 0;
        loop {
            steps += 1;
            if let interlude::InterludeOutcome::Ended { .. } =
                flight.step(&layouts, &mut clock, Buttons(held), none())
            {
                break;
            }
            assert!(steps < 3000);
        }
        assert_eq!(steps, interlude::STEPS);
    }
}

#[test]
fn the_pod_steers_by_the_table_and_stops_at_the_walls() {
    let graphics = bonus_graphics();
    let layouts = layouts();
    let mut flight = bonus_flight(&graphics);
    let mut clock = Clock::default();
    let mut step = |flight: &mut interlude::Interlude, held: u8| {
        flight.step(&layouts, &mut clock, Buttons(held), none());
    };
    // Right: the speed grows by $28 a step, so after 4 steps the pod has moved
    // ($28 + $50 + $78 + $A0) / 256 = 1.5 pixels.
    for _ in 0..4 {
        step(&mut flight, Buttons::RIGHT);
    }
    assert_eq!(flight.pod_x(), 121);
    // Held long enough it stops at the wall, whole part 240.
    for _ in 0..60 {
        step(&mut flight, Buttons::RIGHT);
    }
    assert_eq!(flight.pod_x(), 240);
    // Left from the wall: the speed was cleared there, so it takes a few steps to get away.
    for _ in 0..200 {
        step(&mut flight, Buttons::LEFT);
    }
    assert_eq!(flight.pod_x(), 16);
    // Both held: Left is used.
    for _ in 0..30 {
        step(&mut flight, Buttons::LEFT | Buttons::RIGHT);
    }
    assert_eq!(flight.pod_x(), 16);
}

#[test]
fn the_badge_lands_counts_a_gem_every_16_passes_and_ends_after_358_passes() {
    let graphics = bonus_graphics();
    let mut clock = Clock {
        counter: 0x8A,
        ..Clock::default()
    };
    let mut badge = badge::Badge::new(&graphics, &[], &Palette([0x0F; 32]), hud(), 11).unwrap();
    let mut passes = 0;
    let gain = loop {
        passes += 1;
        if let badge::BadgeOutcome::Ended { gain } = badge.step(&mut clock, none(), none()) {
            break gain;
        }
        assert!(passes < 1000);
    };
    assert_eq!(passes, 358);
    // 11 gems: 100 each for the first nine, 1,100 from the tenth.
    assert_eq!(gain, 9 * 100 + 1100 + 1100);
}

#[test]
fn the_interlude_on_planet_4_counts_its_spawn_index_from_0_unless_the_original_rules_are_on() {
    let graphics = bonus_graphics();
    let layouts = layouts();
    let run = |original: bool| {
        let mut flight =
            interlude::Interlude::new(&graphics, 4, 0, 1504, &Palette([0x0F; 32]), hud(), original)
                .unwrap();
        let mut clock = Clock::default();
        flight.step(&layouts, &mut clock, none(), none());
        flight.slot_kinds()
    };
    // Q40: the original starts at index 125 and its first spawn is a phantom object (never
    // drawn or touched) in slot 11; the remake starts at index 0 with the backdrop's first
    // entry (a mine in this synthetic list).
    assert_eq!(run(true), "p...");
    assert_eq!(run(false), "m...");
}

#[test]
fn the_credits_scroll_a_pixel_every_two_frames_and_last_2274_frames() {
    let mut graphics = graphics();
    let mut credits = credits::Credits::new(&graphics).unwrap();
    graphics.tile_streams.clear();
    assert_eq!(credits.screen().hide_from, Some(233));
    let mut frames = 1;
    while credits.step() == credits::CreditsOutcome::Running {
        frames += 1;
        assert!(frames < 3000);
        if frames == 1000 {
            assert_eq!(credits.screen().scroll.1, 499 % 240);
        }
    }
    // The first frame is the construction, then 2,273 steps, the last of which ends it.
    assert_eq!(frames, credits::FRAMES);
}

#[test]
fn the_credits_write_a_centred_line_every_16_frames() {
    let graphics = graphics();
    let mut credits = credits::Credits::new(&graphics).unwrap();
    // The first line is row 11, written at frame 16 × 12 = 192 into page row 11.
    for _ in 0..191 {
        credits.step();
    }
    assert_eq!(credits.screen().tile(6, 11), 0);
    credits.step();
    let row: Vec<u8> = (6..26).map(|c| credits.screen().tile(c, 11)).collect();
    assert_eq!(row, b"SOLAR\0JETMAN\0CREDITS");
    assert_eq!(credits.screen().tile(5, 11), 0);
    assert_eq!(credits.screen().tile(26, 11), 0);
}

fn ending_graphics() -> Graphics {
    let mut graphics = graphics();
    for path in [
        "tiles/ending/tiles-1000.png",
        "tiles/ending/tiles-1dc0.png",
        "tiles/ending/tiles-0000.png",
        "tiles/ending/tiles-0d60.png",
    ] {
        graphics
            .tile_streams
            .insert(path.to_owned(), vec![0x55; 64]);
    }
    graphics.pictures.insert(
        "pictures/ending.ron".to_owned(),
        vec![PictureWrite {
            address: 0x2000,
            bytes: vec![0x40; 0x3C0],
        }],
    );
    graphics
}

#[test]
fn the_ending_lasts_902_frames_and_the_creature_jumps_six_times() {
    let graphics = ending_graphics();
    // The counter as the message leaves it (a multiple of 32, plus 2).
    let mut clock = Clock {
        counter: 0x1A2,
        ..Clock::default()
    };
    let mut ending = ending::Ending::new(&graphics).unwrap();
    let top = |ending: &ending::Ending| ending.screen().sprites.iter().map(|s| s.y).min();
    let mut peaks = Vec::new();
    let (mut steps, mut lowest, mut jumping) = (0, i32::MAX, false);
    while ending.step(&mut clock) == ending::EndingOutcome::Running {
        steps += 1;
        assert!(steps < 2000);
        if top(&ending).is_some() && steps > 2 {
            // The creature's top row: 134 at rest.
            let creature = ending
                .screen()
                .sprites
                .iter()
                .filter(|s| (184..200).contains(&s.x) && (0xDC..=0xE9).contains(&s.tile))
                .map(|s| s.y)
                .min()
                .unwrap_or(134);
            if creature < 134 {
                jumping = true;
                lowest = lowest.min(creature);
            } else if jumping {
                peaks.push(134 - lowest);
                (jumping, lowest) = (false, i32::MAX);
            }
        }
    }
    // The picture is on for 902 frames; the credits start in the next pass.
    assert_eq!(steps + 1, 902);
    assert_eq!(peaks, [4, 13, 27, 49, 75, 108]);
}

#[test]
fn the_credits_keep_the_originals_cuts_only_under_its_rules() {
    let graphics = graphics();
    for original in [false, true] {
        let mut credits = credits::Credits::new(&graphics).unwrap();
        credits.set_original_rules(original);
        credits.step();
        let first = credits.screen().sprites.len();
        credits.step();
        let full = credits.screen().sprites.len();
        assert_eq!(full - first, if original { 2 } else { 0 }, "{original}");
        assert_eq!(
            credits.screen().hide_line_from.is_some(),
            original,
            "{original}"
        );
        while credits.step() == credits::CreditsOutcome::Running {
            if credits.screen().hide_from == Some(82) {
                break;
            }
        }
        assert_eq!(credits.screen().hide_from == Some(82), original);
    }
}

fn test_assets() -> Assets {
    Assets {
        world: crate::game::level::tests::synthetic_world(),
        graphics: graphics(),
        layouts: layouts(),
    }
}

/// A varied input for pass `n`.
fn pattern(n: u32) -> Buttons {
    let mut bits = 0;
    if n % 7 < 4 {
        bits |= Buttons::B;
    }
    if n % 11 < 3 {
        bits |= Buttons::A;
    }
    if n % 13 < 5 {
        bits |= Buttons::LEFT;
    }
    if n % 17 > 12 {
        bits |= Buttons::RIGHT;
    }
    Buttons(bits)
}

/// A sound player on a made-up song (song 1: a looping melody, a bass and noise).
fn made_up_player() -> Player {
    use crate::game::sound::Event::*;
    let melody = vec![
        Setup {
            control: 0xB0,
            sweep: 0x08,
            high: 0x00,
        },
        Vibrato {
            length: 3,
            rate: 1,
            depth: 2,
        },
        Note(20, 6),
        Note(24, 6),
        Rest(3),
        Note(27, 9),
        Jump(2),
    ];
    let bass = vec![
        Setup {
            control: 0x80,
            sweep: 0,
            high: 0,
        },
        Note(8, 12),
        Note(12, 12),
        Jump(1),
    ];
    let noise = vec![
        Setup {
            control: 0x30,
            sweep: 0,
            high: 0,
        },
        Volume(0x1F),
        Note(5, 3),
        Rest(5),
        Jump(2),
    ];
    let data = crate::game::sound::tests::data_with(
        0x90,
        vec![melody.clone(), melody, bass, noise],
        vec![],
    );
    Player::new(data, 48_000)
}

/// Saving, loading into another game and continuing gives the same states, pass by pass, as
/// not saving.
fn assert_saving_changes_nothing(start: Start, before: u32, after: u32) {
    assert_saving_changes_nothing_in(start, false, before, after);
}

/// [`assert_saving_changes_nothing`] with the levels in the Scaled engine when `scaled`.
fn assert_saving_changes_nothing_in(start: Start, scaled: bool, before: u32, after: u32) {
    let mut assets = test_assets();
    // Add whatever tile streams the scene asks for.
    let mut original = loop {
        match Game::with_engine(&assets, start, scaled) {
            Ok(game) => break game,
            Err(MissingGraphics(what)) => {
                let path = what.strip_prefix("tile stream ").expect(&what).to_owned();
                assets.graphics.tile_streams.insert(path, vec![0x55; 64]);
            }
        }
    };
    // The sound player runs alongside, as in the program: what the game asks for goes to it
    // and it is pulled for samples (a length that does not divide the frame).
    let mut music = made_up_player();
    music.sound_mut().play_song(1);
    let feed = |requests: Vec<SoundRequest>, music: &mut Player| {
        for request in requests {
            match request {
                SoundRequest::SongIfNew(song) => music.sound_mut().play_song_if_new(song),
                SoundRequest::SongLooped(song) => music.sound_mut().play_song_looped(song),
                SoundRequest::Request(request) => music.sound_mut().apply(request),
                SoundRequest::Pause => music.pause(),
                SoundRequest::Resume => music.resume(),
            }
        }
        let mut samples = vec![0.0; 701];
        music.fill(&mut samples);
        samples
    };
    for n in 0..before {
        original.pass(&assets, pattern(n)).unwrap();
        feed(original.take_sound(), &mut music);
    }
    music.sound_mut().request_effect_as_the_game_does(0x2);
    let bytes = original.save_state(Some(&music));
    // Loading into a game in another state replaces all of it, the music too.
    let mut restored = Game::new(&assets, Start::Cold).unwrap();
    let mut restored_music = made_up_player();
    restored_music.sound_mut().play_song(2);
    restored_music.fill(&mut [0.0; 3000]);
    let saved_music = restored
        .load_state(&bytes, &assets)
        .unwrap()
        .expect("the state holds the sound");
    restored_music.restore(saved_music).unwrap();
    assert_eq!(
        restored.save_state(Some(&restored_music)),
        bytes,
        "a loaded state saves as it was"
    );
    assert_eq!(restored.scene_name(), original.scene_name());
    assert_eq!(restored.level(), original.level());
    for n in before..before + after {
        original.pass(&assets, pattern(n)).unwrap();
        restored.pass(&assets, pattern(n)).unwrap();
        assert_eq!(
            original.save_state(Some(&music)),
            restored.save_state(Some(&restored_music)),
            "pass {n}"
        );
        let (asked, asked_again) = (original.take_sound(), restored.take_sound());
        assert_eq!(asked, asked_again, "sound {n}");
        assert_eq!(
            feed(asked, &mut music),
            feed(asked_again, &mut restored_music),
            "samples {n}"
        );
        let (mut a, mut b) = (Frame::default(), Frame::default());
        original.draw(&mut a, &assets, 0.5);
        restored.draw(&mut b, &assets, 0.5);
        assert_eq!(a, b, "picture {n}");
    }
}

#[test]
fn a_saved_gameplay_state_continues_exactly() {
    assert_saving_changes_nothing(Start::Level(0), 300, 300);
}

#[test]
fn a_gameplay_state_saved_between_passes_continues_exactly() {
    assert_saving_changes_nothing(Start::Level(0), 301, 300);
}

#[test]
fn a_saved_scaled_gameplay_state_continues_exactly() {
    assert_saving_changes_nothing_in(Start::Level(0), true, 301, 300);
}

/// Game with whatever tile streams the start asks for added to `assets`.
fn game_with_streams(assets: &mut Assets, start: Start, scaled: bool) -> Game {
    loop {
        match Game::with_engine(assets, start, scaled) {
            Ok(game) => break game,
            Err(MissingGraphics(what)) => {
                let path = what.strip_prefix("tile stream ").expect(&what).to_owned();
                assets.graphics.tile_streams.insert(path, vec![0x55; 64]);
            }
        }
    }
}

#[test]
fn a_level_starts_in_the_engine_chosen() {
    let mut assets = test_assets();
    let original = game_with_streams(&mut assets, Start::Level(0), false);
    assert!(!original.scaled_play());
    assert!(original.gameplay().is_some() && original.scaled().is_none());
    let mut scaled = game_with_streams(&mut assets, Start::Level(0), true);
    assert!(scaled.scaled_play());
    assert!(scaled.gameplay().is_none());
    assert_eq!(scaled.level(), Some(0));
    assert_eq!(scaled.scene_name(), "gameplay");
    // The level start is the Original's.
    let (x, y) = scaled.scaled().unwrap().pod_q16();
    let body = &original.gameplay().unwrap().pod.body;
    assert_eq!(((x >> 16) as u16, (y >> 16) as u16), (body.x, body.y));
    for _ in 0..144 {
        scaled.tick(&assets, Buttons(0), Buttons(0), 144).unwrap();
    }
    assert_eq!(scaled.scaled().unwrap().clock().steps(), 30);
    // The choice holds for levels started later; the level under way keeps its engine.
    scaled.set_scaled(false);
    assert!(scaled.scaled_play());
    // The attract demos fly the engine chosen too.
    let demo = game_with_streams(&mut assets, Start::Demo(1), true);
    assert!(demo.scaled_play());
    let demo = game_with_streams(&mut assets, Start::Demo(1), false);
    assert!(!demo.scaled_play() && demo.gameplay().is_some());
}

#[test]
fn a_scaled_state_saved_at_one_rate_loads_at_another() {
    let mut assets = test_assets();
    let mut game = game_with_streams(&mut assets, Start::Level(0), true);
    for _ in 0..77 {
        game.tick(&assets, Buttons(0), Buttons(0), 144).unwrap();
    }
    let bytes = game.save_state(None);
    let mut loaded = game_with_streams(&mut assets, Start::Warm, false);
    loaded.load_state(&bytes, &assets).unwrap();
    assert!(loaded.scaled_play());
    let time = loaded.scaled().unwrap().clock().time();
    assert_eq!(time, game.scaled().unwrap().clock().time());
    for _ in 0..60 {
        loaded.tick(&assets, Buttons(0), Buttons(0), 60).unwrap();
    }
    assert_eq!(
        loaded.scaled().unwrap().clock().time(),
        time + 30 * crate::game::scaled::clock::STEP
    );
}

#[test]
fn a_saved_title_continues_exactly() {
    assert_saving_changes_nothing(Start::Warm, 200, 200);
}

#[test]
fn a_saved_licence_screen_continues_exactly() {
    assert_saving_changes_nothing(Start::Cold, 100, 500);
}

#[test]
fn bad_save_states_are_refused_and_change_nothing() {
    let assets = test_assets();
    let mut game = Game::new(&assets, Start::Warm).unwrap();
    let before = game.save_state(None);
    assert_eq!(
        game.load_state(b"junk", &assets).err(),
        Some(StateError::NotASaveState)
    );
    let mut other = before.clone();
    other[4..8].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        game.load_state(&other, &assets).err(),
        Some(StateError::Version(99))
    );
    let mut other = before.clone();
    other[8..12].copy_from_slice(&98u32.to_le_bytes());
    assert_eq!(
        game.load_state(&other, &assets).err(),
        Some(StateError::ScaledVersion(98))
    );
    let cut = &before[..before.len() / 2];
    assert!(matches!(
        game.load_state(cut, &assets),
        Err(StateError::Damaged(_))
    ));
    assert_eq!(game.save_state(None), before);
}

#[test]
fn saves_leave_out_what_the_rom_gave() {
    // Every imported stream is solid colour 3; a save may hold none of it.
    for start in [Start::Level(0), Start::Warm, Start::Cold] {
        let mut assets = test_assets();
        while let Err(MissingGraphics(what)) = Game::new(&assets, start) {
            let path = what.strip_prefix("tile stream ").expect(&what).to_owned();
            assets.graphics.tile_streams.insert(path, vec![0; 64]);
        }
        for bytes in assets.graphics.tile_streams.values_mut() {
            bytes.fill(0xFF);
        }
        let mut game = Game::new(&assets, start).unwrap();
        for n in 0..30 {
            game.pass(&assets, pattern(n)).unwrap();
        }
        let text = String::from_utf8(game.save_state(None)[8..].to_vec()).unwrap();
        // A run of 16 threes is a row of solid tiles (pixel values) or of tile bytes (255).
        let longest = text
            .split(|c: char| !c.is_ascii_digit() && c != ',')
            .map(|run| {
                let numbers: Vec<_> = run.split(',').collect();
                let (mut best, mut here) = (1, 1);
                for pair in numbers.windows(2) {
                    let high = matches!(pair[0], "3" | "255");
                    here = if high && pair[0] == pair[1] {
                        here + 1
                    } else {
                        1
                    };
                    best = best.max(here);
                }
                best
            })
            .max()
            .unwrap_or(0);
        assert!(
            longest < 16,
            "{start:?}: a save holds {longest} equal numbers in a row"
        );
        assert!(
            text.len() < 400_000,
            "{start:?}: a save is {} bytes",
            text.len()
        );
    }
}

#[test]
fn a_player_from_a_state_at_another_sample_rate_is_refused() {
    let assets = test_assets();
    let game = Game::new(&assets, Start::Warm).unwrap();
    let music = made_up_player();
    let mut game2 = Game::new(&assets, Start::Warm).unwrap();
    let saved = game2
        .load_state(&game.save_state(Some(&music)), &assets)
        .unwrap()
        .unwrap();
    let mut other = Player::new(music.sound().driver().data().clone(), 44_100);
    assert!(other.restore(saved).is_err());
}

#[test]
fn a_button_ends_the_licence_ending_and_credits_only_when_they_are_skippable() {
    let assets = Assets {
        graphics: ending_graphics(),
        ..test_assets()
    };
    for skippable in [false, true] {
        for credits in [false, true] {
            let mut game = Game::new(&assets, Start::Warm).unwrap();
            game.set_skip_scenes(skippable);
            game.scene = if credits {
                Scene::Credits(Box::new(credits::Credits::new(&assets.graphics).unwrap()))
            } else {
                Scene::Ending(Box::new(ending::Ending::new(&assets.graphics).unwrap()))
            };
            let name = game.scene_name();
            game.pass(&assets, Buttons::default()).unwrap();
            assert_eq!(game.scene_name(), name);
            game.pass(&assets, Buttons(Buttons::A)).unwrap();
            let expected = if skippable { "title" } else { name };
            assert_eq!(game.scene_name(), expected);
        }
        let mut game = Game::new(&assets, Start::Cold).unwrap();
        game.set_skip_scenes(skippable);
        game.pass(&assets, Buttons::default()).unwrap();
        assert_eq!(game.scene_name(), "licence");
        game.pass(&assets, Buttons(Buttons::START)).unwrap();
        let expected = if skippable { "title" } else { "licence" };
        assert_eq!(game.scene_name(), expected);
    }
}
