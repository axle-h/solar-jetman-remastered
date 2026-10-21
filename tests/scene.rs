//! The scene flow against the player's ROM: licence, title, menu, password and the start of
//! gameplay. Skips without a ROM.

use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::scene::{Assets, Game, SoundRequest, Start};

fn assets() -> Option<Assets> {
    let repo = solar_jetman::tools::repo_root();
    let Ok((_, rom)) = solar_jetman::tools::load_rom(&repo, None) else {
        eprintln!("skipped: no ROM");
        return None;
    };
    Some(solar_jetman::tools::assets(&rom).unwrap())
}

fn pass(game: &mut Game, assets: &Assets, mask: u8) {
    game.pass(assets, Buttons(mask)).unwrap();
}

/// A press and a release.
fn tap(game: &mut Game, assets: &Assets, mask: u8) {
    pass(game, assets, mask);
    pass(game, assets, 0);
}

fn idle(game: &mut Game, assets: &Assets, frames: u32) {
    for _ in 0..frames {
        pass(game, assets, 0);
    }
}

/// From power-on to the title menu: the licence screen, then Start skips the sun.
fn to_menu(assets: &Assets) -> Game {
    let mut game = Game::new(assets, Start::Cold).unwrap();
    idle(&mut game, assets, 512);
    assert_eq!(game.scene_name(), "title");
    assert_eq!(game.take_sound(), [SoundRequest::SongLooped(1)]);
    tap(&mut game, assets, Buttons::START);
    idle(&mut game, assets, 200);
    game
}

/// From the approach screen to the launch scene: a button lands, A ends the descent.
fn through_approach_and_surface(game: &mut Game, assets: &Assets) {
    assert_eq!(game.scene_name(), "approach");
    tap(game, assets, Buttons::START);
    assert_eq!(game.scene_name(), "surface");
    idle(game, assets, 5);
    tap(game, assets, Buttons::A);
    assert_eq!(game.scene_name(), "launch");
}

#[test]
fn start_game_begins_level_0_with_a_new_games_state() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    tap(&mut game, &assets, Buttons::START);
    // Level 0 has no flight: the approach screen (song 7), the landing (song 3), then the
    // launch, with no music of its own, which lasts 358 passes unless a button is pressed.
    assert_eq!(game.scene_name(), "approach");
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(7)]);
    tap(&mut game, &assets, Buttons::START);
    assert_eq!(game.scene_name(), "surface");
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(3)]);
    idle(&mut game, &assets, 5);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "launch");
    assert_eq!(game.take_sound(), []);
    idle(&mut game, &assets, 340);
    assert_eq!(game.scene_name(), "launch");
    idle(&mut game, &assets, 30);
    assert_eq!(game.level(), Some(0));
    let play = game.gameplay().unwrap();
    assert_eq!((play.lives, play.jetpods, play.score.0), (4, 4, 1000));
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(8)]);
    idle(&mut game, &assets, 100);
    assert_eq!(game.level(), Some(0));
}

#[test]
fn a_button_skips_the_launch() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    tap(&mut game, &assets, Buttons::START);
    idle(&mut game, &assets, 20);
    through_approach_and_surface(&mut game, &assets);
    idle(&mut game, &assets, 20);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.level(), Some(0));
}

#[test]
fn a_valid_password_starts_its_level() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    tap(&mut game, &assets, Buttons::SELECT);
    tap(&mut game, &assets, Buttons::START);
    assert_eq!(game.scene_name(), "password");
    // Type NLDKHKGDMVLB: level 5, 7 lives, score 12345, pod type 1 with boosters.
    for (place, letter) in b"NLDKHKGDMVLB".iter().enumerate() {
        let steps = usize::from(letter - b'A');
        for _ in 0..steps {
            tap(&mut game, &assets, Buttons::DOWN);
        }
        if place < 11 {
            tap(&mut game, &assets, Buttons::RIGHT);
        }
    }
    tap(&mut game, &assets, Buttons::A);
    // Level 5: the flight first, with song 4; A skips it; then the shop (no music of its own),
    // B leaves it for the approach (song 17), the landing and the launch (a button skips).
    assert_eq!(game.scene_name(), "flight");
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(4)]);
    idle(&mut game, &assets, 100);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "shop");
    assert_eq!(game.take_sound(), []);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(17)]);
    through_approach_and_surface(&mut game, &assets);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.level(), Some(5));
    let play = game.gameplay().unwrap();
    assert_eq!((play.lives, play.jetpods, play.score.0), (7, 7, 12345));
    assert!(play.mapping_device);
    assert_eq!(
        game.take_sound(),
        [SoundRequest::SongIfNew(3), SoundRequest::SongIfNew(18)]
    );
}

#[test]
fn an_invalid_password_with_start_begins_a_new_game() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    tap(&mut game, &assets, Buttons::SELECT);
    tap(&mut game, &assets, Buttons::START);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "password");
    tap(&mut game, &assets, Buttons::START);
    // A new game: level 0 has no flight, so the approach screen follows.
    through_approach_and_surface(&mut game, &assets);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.level(), Some(0));
}

#[test]
fn the_title_times_out_into_the_badge_and_comes_back() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    // The walker's 994 passes and the badge's 256, then the attract story (the first timeout),
    // whose 754 passes lead back to the title.
    idle(&mut game, &assets, 994 + 256);
    assert_eq!(game.scene_name(), "story");
    idle(&mut game, &assets, 754);
    assert_eq!(game.scene_name(), "title");
}

#[test]
fn the_flight_runs_its_course_into_the_launch() {
    let Some(assets) = assets() else { return };
    let mut game = to_menu(&assets);
    tap(&mut game, &assets, Buttons::SELECT);
    tap(&mut game, &assets, Buttons::START);
    for (place, letter) in b"NLDKHKGDMVLB".iter().enumerate() {
        for _ in 0..usize::from(letter - b'A') {
            tap(&mut game, &assets, Buttons::DOWN);
        }
        if place < 11 {
            tap(&mut game, &assets, Buttons::RIGHT);
        }
    }
    tap(&mut game, &assets, Buttons::A);
    // The flight lasts 1,343 passes (scenes.md 5); the A tap's second pass counted as one.
    idle(&mut game, &assets, 1340);
    assert_eq!(game.scene_name(), "flight");
    idle(&mut game, &assets, 10);
    assert_eq!(game.scene_name(), "shop");
}

fn pass_until_scene(game: &mut Game, assets: &Assets, name: &str, limit: u32) -> u32 {
    for n in 1..=limit {
        pass(game, assets, 0);
        if game.scene_name() == name {
            return n;
        }
    }
    panic!("never reached {name}");
}

#[test]
fn the_story_shows_754_frames_and_a_button_ends_it() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Story).unwrap();
    assert_eq!(game.scene_name(), "story");
    // The first picture has c = 4, the last c = 757 (scenes.md 3): 753 passes after the first
    // picture, the next one leaves.
    idle(&mut game, &assets, 753);
    assert_eq!(game.scene_name(), "story");
    pass(&mut game, &assets, 0);
    assert_eq!(game.scene_name(), "title");
    let mut game = Game::new(&assets, Start::Story).unwrap();
    idle(&mut game, &assets, 100);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.scene_name(), "title");
}

#[test]
fn the_attract_cycle_alternates_story_and_demos() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Warm).unwrap();
    game.take_sound();
    // The first timeout gives the story, the second the demo on level 6 (game-flow.md 7.1).
    pass_until_scene(&mut game, &assets, "story", 2_000);
    pass_until_scene(&mut game, &assets, "title", 1_000);
    game.take_sound();
    pass_until_scene(&mut game, &assets, "gameplay", 2_500);
    assert_eq!(game.level(), Some(6));
    // Unlike the original's silent demo, the remake's plays the level's song and its effects.
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(20)));
    idle(&mut game, &assets, 100);
    assert!(!game.take_sound().is_empty());
    // A newly pressed button ends it at the next logic step.
    tap(&mut game, &assets, Buttons::A);
    idle(&mut game, &assets, 2);
    assert_eq!(game.scene_name(), "title");
}

#[test]
fn pause_silences_the_channels_and_freezes_the_picture() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Level(0)).unwrap();
    game.take_sound();
    idle(&mut game, &assets, 40);
    let mut before = solar_jetman::game::playfield::Frame::default();
    game.draw(&mut before, &assets, 1.0);
    // Start is seen in an A pass: press until the pause shows.
    let mut at = 0;
    while !game.gameplay().is_some_and(|g| g.paused()) {
        tap(&mut game, &assets, Buttons::START);
        at += 1;
        assert!(at < 4, "Start is seen within two logic steps");
    }
    assert!(game.take_sound().contains(&SoundRequest::Pause));
    let counter = game.gameplay().unwrap().counter();
    idle(&mut game, &assets, 30);
    // The frame counter runs on, nothing else moves and no sound is asked for.
    assert_eq!(game.gameplay().unwrap().counter(), counter + 30);
    assert_eq!(game.take_sound(), []);
    let mut paused = solar_jetman::game::playfield::Frame::default();
    game.draw(&mut paused, &assets, 1.0);
    let mut again = solar_jetman::game::playfield::Frame::default();
    idle(&mut game, &assets, 5);
    game.draw(&mut again, &assets, 1.0);
    assert_eq!(paused.pixels, again.pixels);
    // Start again leaves the pause and turns the channels on.
    tap(&mut game, &assets, Buttons::START);
    assert!(!game.gameplay().unwrap().paused());
    assert_eq!(game.take_sound(), [SoundRequest::Resume]);
    idle(&mut game, &assets, 10);
    let mut after = solar_jetman::game::playfield::Frame::default();
    game.draw(&mut after, &assets, 1.0);
    assert_ne!(before.pixels, after.pixels);
}

/// The game-over letters against the original's sprite memory (`tools/mesen/gameover-capture.lua`
/// with `SJ_FORCE_AT=2100`, frames 2090-2400 into `vendor/captures/gameover/`): from c = 15 on, every frame's 32
/// letter sprites are where the wave puts them.
#[test]
fn the_game_over_letters_match_the_originals_sprites() {
    let dir = solar_jetman::tools::repo_root().join("vendor/captures/gameover");
    let mut checked = 0;
    for frame in 2116..=2340u32 {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("shot-{frame}.txt"))) else {
            continue;
        };
        let oam: Vec<u8> = text
            .lines()
            .nth(1)
            .unwrap()
            .split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect();
        let mut theirs: Vec<(i32, i32, u8)> = oam
            .chunks(4)
            .filter(|s| s[1] < 0x1A && s[0] < 0xEF && s[2] & 3 == 1)
            .map(|s| (i32::from(s[3]), i32::from(s[0]), s[1]))
            .collect();
        let mut ours = solar_jetman::game::scene::gameover::sprites(frame - 2101);
        theirs.sort_unstable();
        ours.sort_unstable();
        assert_eq!(ours, theirs, "frame {frame}");
        checked += 1;
    }
    eprintln!("{checked} frames compared");
}

/// The loops the surface closes: the new game's approach, landing and launch; a docking with
/// EXPLORE back to the launch and gameplay; the automatic blast-off to the next planet's flight
/// and approach.
#[test]
fn the_surface_closes_the_loops_of_play() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Approach(0)).unwrap();
    through_approach_and_surface(&mut game, &assets);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.scene_name(), "gameplay");
    // The menu after a docking with nothing owned and the flag clear offers EXPLORE only: A, the
    // launch, and gameplay on the same level again.
    let mut game = Game::new(
        &assets,
        Start::Surface {
            level: 0,
            dock: true,
            owned: false,
            flag: false,
            clear: false,
        },
    )
    .unwrap();
    idle(&mut game, &assets, 20);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "launch");
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.level(), Some(0));
    // The surface menu after a docking: the planet's flag is set and nothing is left to collect,
    // so the mothership leaves at once, then the flight to planet 1 (odd: the shop follows).
    let mut game = Game::new(
        &assets,
        Start::Surface {
            level: 0,
            dock: true,
            owned: false,
            flag: true,
            clear: true,
        },
    )
    .unwrap();
    assert_eq!(game.scene_name(), "surface");
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(5)]);
    pass_until_scene(&mut game, &assets, "flight", 1_000);
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(4)]);
    tap(&mut game, &assets, Buttons::A);
    pass_until_scene(&mut game, &assets, "shop", 20);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.scene_name(), "approach");
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(9)]);
}

#[test]
fn the_menu_moves_the_cursor_and_blast_off_leaves_the_planet() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(
        &assets,
        Start::Surface {
            level: 0,
            dock: true,
            owned: false,
            flag: true,
            clear: false,
        },
    )
    .unwrap();
    // Layout 3: EXPLORE and BLAST OFF. Down, then A.
    idle(&mut game, &assets, 20);
    assert_eq!(game.scene_name(), "surface");
    tap(&mut game, &assets, Buttons::DOWN);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.take_sound(), []);
    // 15 passes of the wipe, then song 5 and the flight up and off.
    idle(&mut game, &assets, 20);
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(5)]);
    pass_until_scene(&mut game, &assets, "flight", 1_000);
}

#[test]
fn a_message_screen_lasts_its_timer_and_returns_to_play() {
    use solar_jetman::game::scene::message::Subject;
    let Some(assets) = assets() else { return };
    // Shields found (message 7): 64 idle passes, then the timer counts down while the counter's
    // low byte is 64 or more, and the screen lasts 367 passes in all (366 visible frames: scenes.md 10; compared with Mesen).
    let subject = Subject {
        kind: 1,
        text: 7,
        item: 3,
        cargo: 0,
    };
    let mut game = Game::new(
        &assets,
        Start::Message {
            level: 0,
            subject,
            counter: 0,
        },
    )
    .unwrap();
    assert_eq!(game.scene_name(), "message");
    let mut passes = 0;
    while game.scene_name() == "message" {
        pass(&mut game, &assets, 0);
        passes += 1;
        assert!(passes < 1_000);
    }
    assert_eq!(passes, 367);
    assert_eq!(game.scene_name(), "gameplay");
}

/// Types the password of level 5 (7 lives, score 12345, Nippon pod, mapping device) at the
/// title's CONTINUE screen and starts: the flight to planet 5 follows.
fn start_at_planet_5(assets: &Assets) -> Game {
    let mut game = to_menu(assets);
    tap(&mut game, assets, Buttons::SELECT);
    tap(&mut game, assets, Buttons::START);
    for (place, letter) in b"NLDKHKGDMVLB".iter().enumerate() {
        for _ in 0..usize::from(letter - b'A') {
            tap(&mut game, assets, Buttons::DOWN);
        }
        if place < 11 {
            tap(&mut game, assets, Buttons::RIGHT);
        }
    }
    tap(&mut game, assets, Buttons::A);
    assert_eq!(game.scene_name(), "flight");
    game
}

#[test]
fn an_odd_planet_has_a_shop_after_the_flight_and_b_leaves_to_the_approach() {
    let Some(assets) = assets() else { return };
    let mut game = start_at_planet_5(&assets);
    idle(&mut game, &assets, 100);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "shop");
    // Buy the first item, the homing missiles (400): the score drops digit by digit.
    tap(&mut game, &assets, Buttons::A);
    idle(&mut game, &assets, 60);
    // A move to the right takes 32 frames, then anti gravity (1000) is on show.
    tap(&mut game, &assets, Buttons::RIGHT);
    idle(&mut game, &assets, 40);
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.scene_name(), "approach");
    // The approach screen's password carries what was left: 12345 - 400.
    assert_eq!(game.saved().score_value(), 11945);
}

#[test]
fn equip_pod_shows_the_equip_list_and_b_goes_on_to_the_launch() {
    let Some(assets) = assets() else { return };
    let mut game = start_at_planet_5(&assets);
    idle(&mut game, &assets, 100);
    tap(&mut game, &assets, Buttons::A);
    // Buy the first item, leave, land: something is owned, so the menu after the descent
    // offers EQUIP POD with the cursor on it.
    tap(&mut game, &assets, Buttons::A);
    idle(&mut game, &assets, 60);
    tap(&mut game, &assets, Buttons::B);
    tap(&mut game, &assets, Buttons::START);
    assert_eq!(game.scene_name(), "surface");
    idle(&mut game, &assets, 5);
    tap(&mut game, &assets, Buttons::A);
    idle(&mut game, &assets, 10);
    tap(&mut game, &assets, Buttons::A);
    assert_eq!(game.scene_name(), "equip list");
    tap(&mut game, &assets, Buttons::B);
    assert_eq!(game.scene_name(), "launch");
}

#[test]
fn select_in_the_pause_opens_the_planet_map_and_start_goes_back() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(
        &assets,
        Start::Map {
            level: 5,
            mapping: true,
            super_mapping: false,
            score: 1000,
            lives: 4,
            equipment: 0xC0,
            weapon: 0,
            device: 0,
        },
    )
    .unwrap();
    assert_eq!(game.scene_name(), "gameplay");
    idle(&mut game, &assets, 10);
    tap(&mut game, &assets, Buttons::START);
    idle(&mut game, &assets, 3);
    assert!(game.gameplay().unwrap().paused());
    let _ = game.take_sound();
    tap(&mut game, &assets, Buttons::SELECT);
    assert_eq!(game.scene_name(), "map");
    // Song 6 is started, whatever played.
    assert!(!game.take_sound().is_empty());
    idle(&mut game, &assets, 20);
    assert_eq!(game.scene_name(), "map");
    tap(&mut game, &assets, Buttons::START);
    assert_eq!(game.scene_name(), "gameplay");
    assert!(!game.gameplay().unwrap().paused());
}

#[test]
fn without_a_mapping_device_select_does_nothing() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(
        &assets,
        Start::Map {
            level: 5,
            mapping: false,
            super_mapping: false,
            score: 1000,
            lives: 4,
            equipment: 0xC0,
            weapon: 0,
            device: 0,
        },
    )
    .unwrap();
    idle(&mut game, &assets, 10);
    tap(&mut game, &assets, Buttons::START);
    idle(&mut game, &assets, 3);
    tap(&mut game, &assets, Buttons::SELECT);
    assert_eq!(game.scene_name(), "gameplay");
    assert!(game.gameplay().unwrap().paused());
}

/// The title's Jetman asks for the engine sound as the pod does (`player.md` 4.8): the effect
/// starts at the frames the original's driver started it (`tools/mesen/title-capture.lua`
/// logged them, counted from the start of the title song).
#[test]
fn the_menu_jetmans_engine_sounds_when_the_original_does() {
    use solar_jetman::game::sound::{Channel, Sound};
    let Some(assets) = assets() else { return };
    let repo = solar_jetman::tools::repo_root();
    let (_, rom) = solar_jetman::tools::load_rom(&repo, None).unwrap();
    let data = solar_jetman::tools::sound::sound_data(&rom).unwrap();
    // Runs of starts four frames apart: (first, last), as frames after the song's start in
    // the original, which loads the title for 13 frames the remake does not have (the song is
    // requested at the licence's last frame, the title shows 13 frames later).
    const LOADING_FRAMES: usize = 13;
    let runs = [
        (770, 810),
        (858, 962),
        (1010, 1050),
        (1098, 1138),
        (1186, 1226),
        (1274, 1314),
        (1362, 1382),
        (1478, 1514),
        (1562, 1598),
        (1646, 1658),
    ];
    let expected: Vec<usize> = runs
        .iter()
        .flat_map(|&(first, last)| (first..=last).step_by(4))
        .map(|frame| frame - LOADING_FRAMES)
        .collect();
    let mut game = Game::new(&assets, Start::Cold).unwrap();
    let mut sound = Sound::new(data);
    sound.record_starts(true);
    let mut sink = Vec::new();
    let (mut song_at, mut starts) = (None, Vec::new());
    for pass in 0..512 + 1700usize {
        sink.clear();
        sound.update(&mut sink);
        sound.service();
        for request in game.take_sound() {
            match request {
                SoundRequest::SongLooped(1) => song_at = Some(pass),
                SoundRequest::Request(request) => sound.apply(request),
                _ => {}
            }
        }
        if sound.take_starts().contains(&(Channel::Noise, 0x10))
            && let Some(first) = song_at
        {
            starts.push(pass - first);
        }
        game.pass(&assets, Buttons(0)).unwrap();
    }
    assert_eq!(starts, expected);
}

/// Every typed character but a blank clicks (`text.md` 2.3): effect $3A, started at once on the
/// noise channel.
#[test]
fn typed_text_clicks_for_every_character_but_a_blank() {
    use solar_jetman::game::scene::message::Subject;
    use solar_jetman::game::sound::{Channel, Request};
    let Some(assets) = assets() else { return };
    let subject = Subject {
        kind: 1,
        text: 7,
        item: 3,
        cargo: 0,
    };
    let mut game = Game::new(
        &assets,
        Start::Message {
            level: 0,
            subject,
            counter: 0,
        },
    )
    .unwrap();
    let click = SoundRequest::Request(Request::Direct {
        channel: Channel::Noise,
        id: 0x3A,
    });
    let mut clicks = 0;
    while game.scene_name() == "message" {
        pass(&mut game, &assets, 0);
        clicks += game.take_sound().iter().filter(|r| **r == click).count();
    }
    // "YOUR PROBES ARE NOW", "EQUIPPED WITH SHIELDS" and "-PRESS DOWN TO ACTIVATE" without
    // their blanks.
    assert_eq!(clicks, 16 + 19 + 20);
}

/// Q34: the original leaves the globe of planets 2, 4, 7 and 9 in preset 0's colours; the
/// remake gives every planet its own.
#[test]
fn the_approach_globe_follows_the_validation_mode() {
    let Some(assets) = assets() else { return };
    let colours = |level: usize, original: bool| {
        let game = Game::with_rules(&assets, Start::Approach(level), original).unwrap();
        let palette = game.current_screen().unwrap().palette.0;
        (palette[9..12].to_vec(), palette[21..24].to_vec())
    };
    let own = &assets.graphics.approach_globe[4].clone().unwrap();
    let (globe, unused) = colours(4, false);
    assert_eq!((&globe, &unused == own), (own, false));
    let (globe, unused) = colours(4, true);
    assert_eq!((&unused, globe), (own, vec![0x22, 0x2A, 0x19]));
    // Planet 1 has no exception.
    let own = assets.graphics.approach_globe[1].clone().unwrap();
    assert_eq!(colours(1, true).0, own);
    assert_eq!(colours(1, false).0, own);
}

/// Q35 and Q36 on the landing: the remake draws every sprite one pixel left of the original's
/// and keeps the flame on the mothership.
#[test]
fn the_landing_follows_the_validation_mode() {
    let Some(assets) = assets() else { return };
    let sprites = |original: bool, passes: u32| {
        let mut game = Game::with_rules(
            &assets,
            Start::Surface {
                level: 0,
                dock: false,
                owned: true,
                flag: false,
                clear: false,
            },
            original,
        )
        .unwrap();
        idle(&mut game, &assets, passes);
        game.current_screen().unwrap().sprites.clone()
    };
    // Picture 4 (planet 0) brakes in y between passes 300 and 467.
    let (original, remake) = (sprites(true, 400), sprites(false, 400));
    assert_eq!(original.len(), remake.len());
    let mut flame_moved = 0;
    for (o, r) in original.iter().zip(&remake) {
        assert_eq!(o.x, r.x + 1, "Q35");
        flame_moved += usize::from(o.y != r.y);
    }
    assert!(flame_moved > 0, "Q36: the original's flame trails the ship");
    assert!(flame_moved < original.len(), "the ship itself does not lag");
}

/// The backdrop interlude: 2,048 steps from the star marker's request (scenes.md 11.1), then the
/// badge intermission (11.2: 359 frames from the handler's start, the first of which is not a
/// pass), then the level again.
#[test]
fn the_bonus_flight_and_the_badge_lead_back_to_the_level() {
    use solar_jetman::game::gameplay::economy::Transition;
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Level(0)).unwrap();
    idle(&mut game, &assets, 10);
    assert_eq!(game.scene_name(), "gameplay");
    let score = game.gameplay().unwrap().score.0;
    game.request_transition(Transition::Interlude);
    pass(&mut game, &assets, 0);
    assert_eq!(game.scene_name(), "bonus flight");
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(31)));
    let mut steps = 0;
    while game.scene_name() == "bonus flight" {
        pass(&mut game, &assets, 0);
        steps += 1;
        assert!(steps < 3000);
    }
    // The step that ends it counts: 2,048 steps in all.
    assert_eq!((steps, game.scene_name()), (2048, "badge"));
    // The badge asks for silence (a gem taken in the last step may have a sound still waiting).
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(0)));
    let mut passes = 0;
    while game.scene_name() == "badge" {
        pass(&mut game, &assets, 0);
        passes += 1;
        assert!(passes < 1000);
    }
    assert_eq!((passes, game.scene_name()), (358, "gameplay"));
    // The gems collected on the way were counted, 100 points each (a pod left alone at x = 120
    // happens to take one).
    let gained = game.gameplay().unwrap().score.0 - score;
    assert!(gained > 0 && gained.is_multiple_of(100), "{gained}");
}

/// A warp level: the countdown ends it 1,409 frames after the first gameplay frame (economy.md
/// 5.4), the badge counts the crystals (none here) and the change leads back to the level's
/// planet, the pod 128 px above the mothership (game-flow.md 7.4).
#[test]
fn a_warp_level_ends_in_the_badge_and_leads_back_to_its_planet() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Level(33)).unwrap();
    assert_eq!(game.level(), Some(33));
    assert_eq!(game.take_sound(), [SoundRequest::SongIfNew(5)]);
    let mut frames = 0;
    while game.scene_name() == "gameplay" {
        pass(&mut game, &assets, 0);
        frames += 1;
        assert!(frames < 2000);
    }
    assert_eq!((frames, game.scene_name()), (1409, "badge"));
    // (A pod left alone drifts into a crystal or two, each with its effect.)
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(0)));
    let mut passes = 0;
    while game.scene_name() == "badge" {
        pass(&mut game, &assets, 0);
        passes += 1;
        assert!(passes < 1000);
    }
    // The badge lands, counts a crystal every 16 frames and ends 255 frames after landing (the
    // remake has no blank frames: scenes.md 11.2, 0.1).
    assert_eq!(passes, 358);
    assert_eq!(game.level(), Some(4));
    let play = game.gameplay().unwrap();
    let home = assets.world.levels[4].home;
    assert_eq!(
        (play.pod.body.x, play.pod.body.y),
        (home.x, home.y - 3 - 128)
    );
    assert_eq!(play.pod.invulnerable, 0);
    // The crystals counted on the badge screen, 100 points each.
    assert!(play.score.0 > 1000 && (play.score.0 - 1000).is_multiple_of(100));
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(16)));
}

#[test]
fn the_final_message_leads_to_the_ending_the_credits_and_the_title() {
    use solar_jetman::game::scene::message::Subject;
    let Some(assets) = assets() else { return };
    let subject = Subject {
        kind: 7,
        text: 23,
        item: 0,
        cargo: 0,
    };
    // Whatever the frame counter was when the last core fell, the message lasts 319 passes
    // (docs/deviations.md, Q33); the ending 902 and the credits 2,274, with song 32 playing on.
    for counter in [0x23, 0x63, 0xA3] {
        let mut game = Game::new(
            &assets,
            Start::Message {
                level: 26,
                subject,
                counter,
            },
        )
        .unwrap();
        let mut lengths = Vec::new();
        for scene in ["message", "ending", "credits"] {
            let mut passes = 0;
            while game.scene_name() == scene {
                pass(&mut game, &assets, 0);
                passes += 1;
                assert!(passes < 4_000);
            }
            lengths.push(passes);
        }
        assert_eq!(lengths, [319, 902, 2274]);
        assert_eq!(game.scene_name(), "title");
    }
}

/// The end of the gate state on a second level whose planet's flag is set (the gate itself is
/// the replays' business): the warp-level introduction (kind 6) is shown for 256 frames, then
/// the warp level starts with song 5, nothing to lose (game-flow.md 7.4, scenes.md 0.1).
#[test]
fn the_end_of_a_gate_on_a_flagged_planet_shows_the_warp_level_screen() {
    use solar_jetman::game::gameplay::economy::Transition;
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::Level(13)).unwrap();
    assert_eq!(game.level(), Some(13));
    // The guard refuses a second code 3 for 64 steps after the change into the level.
    idle(&mut game, &assets, 130);
    game.request_transition(Transition::ChangeLevel);
    pass(&mut game, &assets, 0);
    assert_eq!(game.scene_name(), "message");
    let mut shown = 0;
    while game.scene_name() == "message" {
        pass(&mut game, &assets, 0);
        shown += 1;
        assert!(shown < 400);
    }
    assert_eq!(shown, 256);
    assert_eq!(game.level(), Some(29));
    assert_eq!(game.gameplay().unwrap().pod.invulnerable, 0);
    assert_eq!(game.take_sound().last(), Some(&SoundRequest::SongIfNew(5)));
}

/// The last-password screen (`scenes.md` 12): shown for any pressed button, the status panel
/// goes with the press and the program restarts one pass later; no lock-out.
#[test]
fn the_last_password_screen_waits_for_a_button_then_restarts() {
    let Some(assets) = assets() else { return };
    let mut game = Game::new(&assets, Start::LastPassword(0)).unwrap();
    assert_eq!(game.scene_name(), "last password");
    idle(&mut game, &assets, 100);
    assert_eq!(game.scene_name(), "last password");
    let before = game.current_screen().unwrap().panel.is_some();
    pass(&mut game, &assets, Buttons::A);
    assert!(before);
    assert_eq!(game.scene_name(), "last password");
    assert!(game.current_screen().unwrap().panel.is_none());
    pass(&mut game, &assets, 0);
    assert_eq!(game.scene_name(), "title");
}

/// A small generator of button patterns (no dependency for a test).
fn pattern(seed: &mut u32) -> Buttons {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    // Mostly thrust and turns, now and then the gun, the shield and Start.
    let mut buttons = 0;
    for (mask, odds) in [
        (Buttons::B, 2),
        (Buttons::LEFT, 4),
        (Buttons::RIGHT, 4),
        (Buttons::A, 3),
        (Buttons::DOWN, 16),
        (Buttons::UP, 16),
        (Buttons::SELECT, 24),
        (Buttons::START, 200),
    ] {
        if (*seed >> (mask.trailing_zeros() * 3)).is_multiple_of(odds) {
            buttons |= mask;
        }
    }
    Buttons(buttons)
}

#[test]
fn the_scaled_engine_plays_every_level_through_its_scene_changes() {
    // Random play in the Scaled engine at changing tick rates: the pod dies, Jetman walks and
    // dies, the game ends and begins again, and the scenes in between run at 60 Hz, as the
    // program runs them. Nothing may panic, and the clock must go on in play.
    let Some(assets) = assets() else {
        eprintln!("skipped: no ROM");
        return;
    };
    let rates = [60, 75, 90, 100, 120, 137, 144, 165, 200];
    for level in [0, 1, 2, 5, 9, 12, 13, 20, 26, 30] {
        let mut game = Game::with_engine(&assets, Start::Level(level), true).unwrap();
        let mut seed = 0x1234_5678 ^ level as u32;
        let (mut ticks, mut scenes) = (0, 0);
        let mut buttons = Buttons(0);
        for frame in 0..9000usize {
            // A new pattern every seventh frame, held in between.
            if frame.is_multiple_of(7) {
                buttons = pattern(&mut seed);
            }
            if game.scaled_play() {
                let rate = rates[(frame / 600) % rates.len()];
                game.tick(&assets, buttons, buttons, rate).unwrap();
                ticks += 1;
            } else {
                game.pass(&assets, buttons).unwrap();
                scenes += 1;
            }
            game.take_sound();
        }
        assert!(ticks > 0, "level {level}: no ticks");
        eprintln!("level {level}: {ticks} ticks, {scenes} scene frames");
    }
}

/// Runs the attract demo `index` in the engine chosen, with no input, until gameplay ends;
/// returns the frames it played and the scene after it.
fn demo(assets: &Assets, index: usize, scaled: bool, press_at: Option<u32>) -> (u32, &'static str) {
    let mut game = Game::with_engine(assets, Start::Demo(index), scaled).unwrap();
    for frame in 0..130 * 60 {
        let held = if press_at == Some(frame) {
            Buttons(Buttons::A)
        } else {
            Buttons(0)
        };
        if game.scaled_play() {
            game.tick(assets, held, held, 60).unwrap();
        } else {
            game.pass(assets, held).unwrap();
        }
        if game.scene_name() != "gameplay" {
            return (frame, game.scene_name());
        }
    }
    (u32::MAX, game.scene_name())
}

#[test]
fn the_attract_demo_tows_an_item_home_then_returns_to_the_title() {
    let Some(assets) = assets() else { return };
    for scaled in [false, true] {
        // Demo 2, level 4: a cargo container, in well under the two minutes a demo may take.
        let (frames, scene) = demo(&assets, 1, scaled, None);
        assert_eq!(scene, "title", "scaled {scaled}");
        assert!(frames < 80 * 60, "scaled {scaled}: {frames} frames");
    }
}

#[test]
fn a_button_ends_the_attract_demo() {
    let Some(assets) = assets() else { return };
    for scaled in [false, true] {
        assert_eq!(
            demo(&assets, 0, scaled, Some(300)),
            (300, "title"),
            "scaled {scaled}"
        );
    }
}
