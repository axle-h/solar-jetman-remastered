//! Solar Jetman remastered: the SDL3 program. For now the first slice of gameplay (the pod on
//! a level, `play` mode) and, with `--viewer`, a level viewer: the playfield of a level,
//! scrolled with the controller's d-pad.

mod audio;
mod controls;
mod folder;
mod font;
mod menu;
mod romfile;
mod settings;
mod ui;
mod video;
mod web;

use folder::{Folder, PackState};
use menu::Menu;
use settings::Settings;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::graphics::LevelGraphics;
use solar_jetman::game::level::World;
use solar_jetman::game::playfield::Frame;
use solar_jetman::game::scaled::rate::{Choice, Controller};
use solar_jetman::game::scene::{Assets, Game, SoundRequest, Start};
use solar_jetman::game::sound::SoundData;
use solar_jetman::game::sound::player::Player;
use solar_jetman::game::viewer::{LevelViewer, ViewerInput};
use solar_jetman::game::{SCREEN_HEIGHT, SCREEN_WIDTH, TICK_HZ};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

include!(concat!(env!("OUT_DIR"), "/embedded_rom.rs"));

// Release builds must never contain the ROM (docs/CLEAN_ROOM.md, rule 7).
#[cfg(not(debug_assertions))]
const _: () = assert!(
    EMBEDDED_ROM.is_none(),
    "a release build must not embed the ROM"
);

const USAGE: &str =
    "usage: solar-jetman [--import PATH] [--rom PATH] [--level N] [--demo N] [--title] [--viewer] [--no-vsync]

--import PATH   import the game's data from a .nes file (or a .zip holding one) into the
                player's folder, then start
--rom PATH      play straight from a ROM without importing (development)
--level N       start in gameplay on level N (development)
--demo N        start with attract demo N, 0-2 (levels 6, 4 and 10)
--title         skip the licence screen and start at the title
--viewer        show level N's playfield (with --level), scrolled with the d-pad

F1 (or a gamepad's guide button, or Select+Start) opens the port menu; F5 and F9 save and load
a state, F12 takes a screenshot, Escape quits. The controller: arrows, X is A (fire), Z is B (thrust),
Backspace is Select and Enter is Start; on a gamepad the d-pad, A, B, Back and Start. Every
control can be rebound in the port menu's Controls pages.";

struct Options {
    rom: Option<PathBuf>,
    import: Option<PathBuf>,
    /// `--level`: gameplay straight away.
    level: Option<usize>,
    /// `--demo`: an attract demo straight away.
    demo: Option<usize>,
    title: bool,
    viewer: bool,
    no_vsync: bool,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        rom: None,
        import: None,
        level: None,
        demo: None,
        title: false,
        viewer: false,
        no_vsync: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rom" => options.rom = Some(args.next().ok_or("--rom needs a path")?.into()),
            "--import" => options.import = Some(args.next().ok_or("--import needs a path")?.into()),
            "--level" => {
                options.level = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .ok_or("--level needs a number 0-41")?,
                );
            }
            "--demo" => {
                options.demo = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .filter(|&n| n < 3)
                        .ok_or("--demo needs a number 0-2")?,
                );
            }
            "--title" => options.title = true,
            "--viewer" => options.viewer = true,
            "--no-vsync" => options.no_vsync = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ => return Err(format!("unexpected argument `{arg}`\n\n{USAGE}")),
        }
    }
    Ok(options)
}

type Files = BTreeMap<String, Vec<u8>>;

/// The level data, the graphics to draw with and the sound to play, from the files of an
/// asset pack.
fn load_assets(files: &Files) -> Result<(Assets, SoundData), String> {
    let world = solar_jetman::rom::world(files).map_err(|error| error.to_string())?;
    let layouts = solar_jetman::rom::sprites(files).map_err(|error| error.to_string())?;
    let graphics = solar_jetman::rom::graphics(files).map_err(|error| error.to_string())?;
    let periods = solar_jetman::rom::note_periods(files).map_err(|error| error.to_string())?;
    let sound = solar_jetman::rom::sound_data(files, periods).map_err(|error| error.to_string())?;
    Ok((
        Assets {
            world,
            graphics,
            layouts,
        },
        sound,
    ))
}

/// Checks and imports a ROM file's bytes (unpacking a zip). Returns the pack's files and
/// the plain `.nes` bytes.
fn import_bytes(file: Vec<u8>) -> Result<(Files, Vec<u8>), String> {
    let nes = romfile::rom_bytes(file)?;
    let rom = solar_jetman::rom::Rom::from_nes_file(&nes).map_err(|error| match error {
        solar_jetman::rom::RomError::WrongChecksum { .. } => {
            "This is not the USA release of Solar Jetman (NES). Other releases and hacks \
             do not work."
                .to_owned()
        }
        other => format!("This is not a usable NES ROM file: {other}."),
    })?;
    let pack = solar_jetman::rom::import(&rom).map_err(|error| error.to_string())?;
    Ok((pack.files, nes))
}

fn import_file(path: &Path) -> Result<(Files, Vec<u8>), String> {
    let file = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    import_bytes(file)
}

/// Imports a file into the player's folder.
fn import_into(folder: &Folder, path: &Path) -> Result<Files, String> {
    let (files, nes) = import_file(path)?;
    folder.write_pack(&files, &nes)?;
    Ok(files)
}

/// Where the level select starts: a planet's approach, or the final level.
fn selected_start(level: u8) -> Start {
    if level >= settings::FINAL_LEVEL {
        Start::Final
    } else {
        Start::Approach(usize::from(level))
    }
}

/// Hands the game's sound requests to the audio player.
fn play_sound(game: &mut Game, audio: &mut Option<audio::Audio>) {
    let requests = game.take_sound();
    let Some(audio) = audio.as_mut() else {
        return;
    };
    if requests.is_empty() {
        return;
    }
    // The game's frame counter picks the slot serviced next (sound.md 8.2): the request lock
    // lets one servicing through, which must be the slot the game filled ($32 on a respawn).
    // One hold of the player, so that no frame runs between the two.
    let frame = game.sound_counter() as u8;
    audio.with_player(|p| {
        p.sound_mut().set_frame(frame);
        for request in requests {
            match request {
                SoundRequest::SongIfNew(song) => p.sound_mut().play_song_if_new(song),
                SoundRequest::SongLooped(song) => p.sound_mut().play_song_looped(song),
                SoundRequest::Request(request) => p.sound_mut().apply(request),
                SoundRequest::Pause => p.pause(),
                SoundRequest::Resume => p.resume(),
            }
        }
    });
}

/// Silences the music and effects of a game that has been replaced.
fn restart_sound(audio: &mut Option<audio::Audio>, settings: &Settings) {
    if let Some(audio) = audio.as_mut() {
        audio.with_player(|player| {
            let data = player.sound().driver().data().clone();
            *player = Player::new(data, player.sample_rate());
            player.set_gain(settings.gain());
        });
    }
}

/// A new game with the port menu's choices applied.
fn new_game(assets: &Assets, start: Start, settings: &Settings) -> Result<Box<Game>, String> {
    let mut game =
        Game::with_engine(assets, start, settings.remastered).map_err(|error| error.to_string())?;
    game.set_cheats(settings.cheats);
    // The pod rest belongs to the Remastered physics.
    game.set_pod_rest(settings.remastered);
    // Any button ends the licence screen, the ending and the credits.
    game.set_skip_scenes(true);
    Ok(Box::new(game))
}

/// Saves the game to a slot. Returns the words for the menu.
fn save_to_slot(
    folder: &Folder,
    mode: &Mode,
    audio: &mut Option<audio::Audio>,
    slot: usize,
) -> String {
    let Mode::Play { game, .. } = mode else {
        return "NO GAME TO SAVE".to_owned();
    };
    // The sound goes in the save: a copy taken with the audio thread held off for a moment.
    let sound = audio
        .as_mut()
        .and_then(|audio| audio.with_player(|player| player.clone()));
    match folder.write_save(slot, &game.save_state(sound.as_ref())) {
        Ok(()) => format!("SAVED IN SLOT {}", slot + 1),
        Err(error) => {
            eprintln!("solar-jetman: save: {error}");
            "SAVING FAILED".to_owned()
        }
    }
}

/// Loads the game from a slot. Returns the words for the menu.
fn load_from_slot(
    folder: &Folder,
    mode: &mut Mode,
    audio: &mut Option<audio::Audio>,
    menu_open: bool,
    slot: usize,
    settings: &Settings,
) -> String {
    let Mode::Play { game, assets } = mode else {
        return "NO GAME TO LOAD INTO".to_owned();
    };
    match folder.read_save(slot) {
        Ok(Some(bytes)) => match game.load_state(&bytes, assets) {
            Ok(saved_sound) => {
                game.set_cheats(settings.cheats);
                if let (Some(saved), Some(audio)) = (saved_sound, audio.as_mut()) {
                    // Music and effects go on from where the state was saved; the menu's pause
                    // follows the menu, not the save.
                    let restored = audio.with_player(|player| {
                        let restored = player.restore(saved);
                        if menu_open {
                            player.pause();
                        } else {
                            player.resume();
                        }
                        restored
                    });
                    if let Some(Err(error)) = restored {
                        eprintln!("solar-jetman: load: {error}");
                    }
                }
                format!("LOADED SLOT {}", slot + 1)
            }
            Err(error) => {
                eprintln!("solar-jetman: load: {error}");
                "CANNOT USE THIS SAVE".to_owned()
            }
        },
        Ok(None) => format!("SLOT {} IS EMPTY", slot + 1),
        Err(error) => {
            eprintln!("solar-jetman: load: {error}");
            "LOADING FAILED".to_owned()
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("solar-jetman: {message}");
            ExitCode::FAILURE
        }
    }
}

/// What the viewer shows: the level's graphics, rebuilt when the level changes.
struct Shown {
    level: usize,
    graphics: LevelGraphics,
}

/// The window's parts that the screens below share.
struct Shell<'a> {
    screen: video::Screen,
    input: video::Input,
    texture: sdl3::render::Texture<'a>,
}

impl Shell<'_> {
    fn message(&mut self, title: &str, body: &str) -> Result<(), String> {
        let frame = Frame::default();
        self.screen.present(&mut self.texture, &frame, |s| {
            ui::draw_message(s, title, body)
        })
    }
}

/// Where the game's data comes from: `--rom`, `--import`, a debug build's embedded ROM, or the
/// player's folder. Without any, asks for the ROM on a message screen. `None`: the player quit.
fn obtain_files(
    options: &Options,
    folder: &Folder,
    shell: &mut Shell,
) -> Result<Option<Files>, String> {
    if let Some(path) = &options.rom {
        return import_file(path).map(|(files, _)| Some(files));
    }
    if let Some(path) = &options.import {
        return import_into(folder, path).map(Some);
    }
    if let Some(rom) = EMBEDDED_ROM {
        return import_bytes(rom.to_vec()).map(|(files, _)| Some(files));
    }
    let mut status = match folder.read_pack() {
        PackState::Current(files) => return Ok(Some(files)),
        PackState::Missing => String::new(),
        PackState::Stale => {
            // A newer importer, or another ROM: run the import again from the kept ROM.
            match folder.stored_rom().map(import_bytes) {
                Some(Ok((files, nes))) => {
                    folder.write_pack(&files, &nes)?;
                    return Ok(Some(files));
                }
                _ => "The game data in the player's folder is out of date, and the ROM kept \
                      with it cannot be used. Please give the ROM again."
                    .to_owned(),
            }
        }
    };
    ask_for_rom(folder, shell, &mut status)
}

/// The import screen: waits for a file dropped on the window, or chosen in the file dialog.
fn ask_for_rom(
    folder: &Folder,
    shell: &mut Shell,
    status: &mut String,
) -> Result<Option<Files>, String> {
    let (send, receive) = mpsc::channel::<PathBuf>();
    let mut dialog_open = false;
    loop {
        let events = shell.input.poll();
        if events.quit {
            return Ok(None);
        }
        let mut chosen = events.dropped;
        #[cfg(target_os = "emscripten")]
        if Path::new(web::IMPORT_FILE).exists() {
            chosen.push(PathBuf::from(web::IMPORT_FILE));
        }
        while let Ok(path) = receive.try_recv() {
            dialog_open = false;
            chosen.push(path);
        }
        if events.nav.contains(&menu::Nav::Accept) && !dialog_open {
            dialog_open = open_dialog(shell.screen.window(), &send);
        }
        if let Some(path) = chosen.last() {
            let imported = import_into(folder, path);
            #[cfg(target_os = "emscripten")]
            if path.as_path() == Path::new(web::IMPORT_FILE) {
                let _ = std::fs::remove_file(path);
            }
            match imported {
                Ok(files) => return Ok(Some(files)),
                Err(message) => *status = message,
            }
        }
        let mut text = format!(
            "Solar Jetman needs the game's data from your own copy of the ROM (USA, NES).\n\n\
             Drop the .nes file, or a .zip holding it, onto this {}",
            if cfg!(target_os = "emscripten") {
                "page"
            } else {
                "window"
            }
        );
        if cfg!(not(target_os = "emscripten")) {
            let start = controls::key_name(shell.input.keys().get(controls::Control::Start));
            text.push_str(&format!(
                ", or press {} (A on a gamepad) to choose it",
                start.to_lowercase()
            ));
        } else {
            text.push_str(", or use the Choose ROM button under it");
        }
        text.push('.');
        if !status.is_empty() {
            text.push_str("\n\n");
            text.push_str(status);
        }
        shell.message("ROM NEEDED", &text)?;
        web::pause(Duration::from_millis(16));
    }
}

/// Shows SDL's file dialog; the chosen path arrives on `send`.
#[cfg(not(target_os = "emscripten"))]
fn open_dialog(window: &sdl3::video::Window, send: &mpsc::Sender<PathBuf>) -> bool {
    let send = std::sync::Mutex::new(send.clone());
    let filters = [
        sdl3::dialog::DialogFileFilter {
            name: "NES ROM or zip",
            pattern: "nes;zip",
        },
        sdl3::dialog::DialogFileFilter {
            name: "All files",
            pattern: "*",
        },
    ];
    sdl3::dialog::show_open_file_dialog(
        &filters,
        None::<&Path>,
        false,
        window,
        Box::new(move |result, _| {
            if let (Ok(paths), Ok(send)) = (result, send.lock())
                && let Some(path) = paths.into_iter().next()
            {
                let _ = send.send(path);
            }
        }),
    )
    .is_ok()
}

#[cfg(target_os = "emscripten")]
fn open_dialog(_: &sdl3::video::Window, _: &mpsc::Sender<PathBuf>) -> bool {
    false
}

/// Writes the picture to `screenshots/` as a PNG and returns its path.
fn save_screenshot(folder: &Folder, rgb: &[u8]) -> Result<PathBuf, String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dir = folder.screenshots();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut path = dir.join(format!("screenshot-{seconds}.png"));
    for n in 2.. {
        if !path.exists() {
            break;
        }
        path = dir.join(format!("screenshot-{seconds}-{n}.png"));
    }
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgb).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

fn run() -> Result<(), String> {
    let options = options()?;
    let folder = Folder::locate()?;
    folder.create()?;
    let mut settings = folder.load_settings();
    if options.no_vsync {
        settings.vsync = false;
    }

    let screen = video::Screen::open(&settings)?;
    let input = video::Input::new(&screen, &settings)?;
    let creator = screen.texture_creator();
    let texture = video::frame_texture(&creator)?;
    let mut shell = Shell {
        screen,
        input,
        texture,
    };
    let Some(files) = obtain_files(&options, &folder, &mut shell)? else {
        return Ok(());
    };
    let (assets, sound) = load_assets(&files)?;
    let world: World = assets.world.clone();
    let graphics = assets.graphics.clone();
    let Shell {
        mut screen,
        mut input,
        mut texture,
    } = shell;

    let level = options.level.unwrap_or(0).min(world.levels.len() - 1);
    let mut mode = if options.viewer {
        Mode::Viewer(LevelViewer::new(&world, level))
    } else {
        let start = match options.level {
            _ if let Some(demo) = options.demo => Start::Demo(demo),
            Some(_) => Start::Level(level),
            None if options.title => Start::Warm,
            None => Start::Cold,
        };
        let game = new_game(&assets, start, &settings)?;
        Mode::Play {
            game,
            assets: Box::new(assets),
        }
    };
    let level_graphics = |level: usize| {
        LevelGraphics::new(&graphics, &world.levels[level]).map_err(|error| error.to_string())
    };
    let mut shown = Shown {
        level: mode.level(),
        graphics: level_graphics(mode.level())?,
    };

    // Without an audio device the viewer still runs, silently.
    let mut audio = audio::Audio::open(screen.sdl(), sound)
        .map_err(|error| eprintln!("solar-jetman: no sound: {error}"))
        .ok();
    let apply_sound = |audio: &mut Option<audio::Audio>, settings: &Settings| {
        if let Some(audio) = audio.as_mut() {
            audio.with_player(|player| player.set_gain(settings.gain()));
        }
    };
    apply_sound(&mut audio, &settings);
    let mut window_title = title(&mode);
    screen.set_title(&window_title)?;
    let mut frame = Frame::default();
    let mut menu = Menu::default();

    let tick = Duration::from_secs_f64(1.0 / TICK_HZ);
    let mut accumulator = Duration::ZERO;
    let mut last = Instant::now();
    // The Scaled engine's tick rate, from the frames presented (`solar_jetman::game::scaled::rate`).
    let start = Instant::now();
    let mut rate = Controller::new(Choice::Auto);
    let mut work = Duration::ZERO;
    loop {
        let frame_start = Instant::now();
        input.set_menu_open(menu.is_open());
        input.set_listening(menu.listening());
        let events = input.poll();
        if events.quit {
            return Ok(());
        }
        if events.screenshot {
            match save_screenshot(&folder, screen.picture()) {
                Ok(path) => {
                    println!("Screenshot saved to {}", path.display());
                    web::sync_folder();
                }
                Err(error) => eprintln!("solar-jetman: screenshot: {error}"),
            }
        }
        for (wanted, saving) in [(events.quick_save, true), (events.quick_load, false)] {
            if !wanted {
                continue;
            }
            let words = if saving {
                save_to_slot(&folder, &mode, &mut audio, settings.slot)
            } else {
                load_from_slot(
                    &folder,
                    &mut mode,
                    &mut audio,
                    menu.is_open(),
                    settings.slot,
                    &settings,
                )
            };
            println!("{words}");
            menu.set_notice(&words);
        }
        if events.menu_toggle {
            menu.toggle();
            if let Some(audio) = audio.as_mut() {
                let open = menu.is_open();
                audio.with_player(|p| if open { p.pause() } else { p.resume() });
            }
        }
        let was_open = menu.is_open();
        let before = settings.clone();
        let mut outcomes = Vec::new();
        if menu.is_open() {
            for nav in events.nav {
                outcomes.push(menu.handle(nav, &mut settings));
            }
        }
        if let Some(pressed) = events.bound {
            outcomes.push(menu.bind(pressed, &mut settings));
        }
        if outcomes.iter().any(|outcome| outcome.settings_changed) {
            folder.save_settings(&settings)?;
            screen.apply(&settings, Some(&before))?;
            input.set_controls(&settings);
            apply_sound(&mut audio, &settings);
            if let Mode::Play { game, .. } = &mut mode {
                game.set_cheats(settings.cheats);
                game.set_scaled(settings.remastered);
                game.set_pod_rest(settings.remastered);
            }
        }
        for outcome in outcomes {
            if outcome.quit {
                return Ok(());
            }
            match outcome.action {
                Some(menu::Action::Save(slot)) => {
                    menu.set_notice(&save_to_slot(&folder, &mode, &mut audio, slot));
                }
                Some(menu::Action::Load(slot)) => {
                    menu.set_notice(&load_from_slot(
                        &folder,
                        &mut mode,
                        &mut audio,
                        menu.is_open(),
                        slot,
                        &settings,
                    ));
                }
                Some(menu::Action::StartLevel(level)) => {
                    // One-shot: the game starts at the level now; nothing is remembered.
                    if let Mode::Play { game, assets } = &mut mode {
                        *game = new_game(assets, selected_start(level), &settings)?;
                        restart_sound(&mut audio, &settings);
                        menu.toggle();
                    }
                }
                Some(menu::Action::Reset) => {
                    // The console's reset button: a warm start, at the title.
                    if let Mode::Play { game, assets } = &mut mode {
                        *game = new_game(assets, Start::Warm, &settings)?;
                        restart_sound(&mut audio, &settings);
                        menu.toggle();
                    }
                }
                None => {}
            }
        }
        if was_open
            && !menu.is_open()
            && let Some(audio) = audio.as_mut()
        {
            audio.with_player(|p| p.resume());
        }
        let paused = menu.is_open();
        let mut scaled_blend = None;

        let now = Instant::now();
        let elapsed = now - last;
        last = now;
        menu.tick(elapsed);
        if !paused {
            if mode.level() != shown.level {
                shown = Shown {
                    level: mode.level(),
                    graphics: level_graphics(mode.level())?,
                };
            }
            let wanted = title(&mode);
            if wanted != window_title {
                screen.set_title(&wanted)?;
                window_title = wanted;
            }

            let held: ViewerInput = input.held();
            let buttons = input.buttons();
            // A press that came and went since the last frame still counts.
            let seen = Buttons(buttons.0 | input.take_pressed().0);
            let rated = rate.frame(
                start.elapsed().as_nanos() as u64,
                work.as_nanos() as u64,
                settings.vsync,
            );
            if let Mode::Play { game, assets } = &mut mode
                && game.scaled_play()
            {
                // The Scaled engine's gameplay: ticks at the controller's rate.
                accumulator = Duration::ZERO;
                let mut seen = seen;
                for _ in 0..rated.ticks {
                    if !game.scaled_play() {
                        break;
                    }
                    game.tick(assets, seen, buttons, rate.rate())
                        .map_err(|error| error.to_string())?;
                    seen = buttons;
                    play_sound(game, &mut audio);
                }
                scaled_blend = Some(rated.blend.unwrap_or(1.0));
            } else {
                // Fixed logic ticks on the NES frame clock; drawing at the display's rate.
                accumulator += elapsed;
                // After a long stall (window dragged, machine asleep) do not run a burst.
                accumulator = accumulator.min(tick * 8);
                // The viewer steps once per tick; the game runs one pass (half a logic step)
                // per tick, on the NES's frame clock, reading the controller on A passes.
                let mut seen = seen;
                while accumulator >= tick {
                    match &mut mode {
                        Mode::Viewer(viewer) => viewer.step(&world, held),
                        Mode::Play { game, assets } => {
                            game.pass(assets, seen).map_err(|error| error.to_string())?;
                            seen = buttons;
                            play_sound(game, &mut audio);
                        }
                    }
                    accumulator -= tick;
                }
            }
        }
        let blend = scaled_blend.unwrap_or_else(|| accumulator.as_secs_f32() / tick.as_secs_f32());
        match &mode {
            Mode::Viewer(viewer) => viewer.draw(&mut frame, &world, &shown.graphics, blend),
            Mode::Play { game, assets } => game.draw(&mut frame, assets, blend),
        }
        let filled = if paused {
            folder.filled_slots()
        } else {
            [false; settings::SLOTS]
        };
        work = frame_start.elapsed();
        screen.present(&mut texture, &frame, |surface| {
            if paused {
                ui::draw_menu(surface, &menu, &settings, filled);
            }
        })?;
        if !settings.vsync {
            // Without vsync, yield a little rather than spin a core at full speed.
            web::pause(Duration::from_millis(1));
        }
    }
}

/// What the window shows: the game, or the level viewer.
#[allow(clippy::large_enum_variant)] // one value for the whole run
enum Mode {
    Viewer(LevelViewer),
    Play {
        game: Box<Game>,
        assets: Box<Assets>,
    },
}

impl Mode {
    fn level(&self) -> usize {
        match self {
            Mode::Viewer(viewer) => viewer.level(),
            Mode::Play { game, .. } => game.level().unwrap_or(0),
        }
    }
}

fn title(mode: &Mode) -> String {
    let level = mode.level();
    match mode {
        Mode::Viewer(_) => format!(
            "Solar Jetman remastered: level viewer, level {level} ({SCREEN_WIDTH}×{SCREEN_HEIGHT})"
        ),
        Mode::Play { game, .. } => match game.level() {
            Some(level) => format!("Solar Jetman remastered: level {level}"),
            None => format!("Solar Jetman remastered: {}", game.scene_name()),
        },
    }
}
