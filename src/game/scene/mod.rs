//! The scenes and how the game moves between them (`docs/spec/game-flow.md` 7): the licence
//! screen, the title with its menu and attract badge, the password entry and gameplay. Scenes
//! the remake has not built yet are stubs: they end at once and the flow goes on to the next
//! scene that exists.
//!
//! [`Game`] owns the current scene and what outlives it (the frame counter and random
//! generator, the state a password keeps). The program calls [`Game::pass`] once per frame of
//! the NES clock and [`Game::draw`] to show the result.

pub mod approach;
pub mod badge;
pub mod credits;
pub mod ending;
pub mod entry;
pub mod flight;
pub mod gameover;
pub mod interlude;
pub mod launch;
pub mod map;
pub mod message;
pub mod ppu;
pub mod shop;
pub mod story;
pub mod strings;
pub mod surface;
pub mod title;

use crate::game::gameplay::economy::{Outcome, Transition};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::inventory::Inventory;
use crate::game::gameplay::objects::Score;
use crate::game::gameplay::rng::Rng;
use crate::game::gameplay::sprites::{SpriteFile, Sprites, draw_tile};
use crate::game::gameplay::{Cheats, Gameplay, Seed};
use crate::game::graphics::{Chr, ChrLoad, Graphics, LevelGraphics, MissingGraphics, Palette};
use crate::game::hud::Hud;
use crate::game::level::World;
use crate::game::password::Saved;
use crate::game::pilot::Pilot;
use crate::game::playfield::Frame;
use crate::game::scaled::Scaled;
use crate::game::sound::player::Player;
use crate::game::state::StateError;
use approach::{Approach, ApproachOutcome, Facts};
use badge::{Badge, BadgeOutcome};
use credits::{Credits, CreditsOutcome};
use ending::{Ending, EndingOutcome};
use entry::{Entry, EntryOutcome};
use flight::{Flight, FlightOutcome};
use interlude::{Interlude, InterludeOutcome};
use launch::{Launch, LaunchOutcome};
use map::{MapFacts, MapOutcome, PlanetMap};
use message::{Message, MessageOutcome, Subject};
use ppu::Screen;
use shop::{Mode, Shop, ShopOutcome};
use story::{Story, StoryOutcome};
use surface::{Surface, SurfaceOutcome};
use title::{Title, TitleOutcome};

/// The font's loads into CHR memory (graphics.md 5.1): digits, letters, then the frame pieces.
pub(crate) const FONT_LOADS: [(&str, u16); 3] = [
    ("tiles/font/digits.png", 0x1300),
    ("tiles/font/letters.png", 0x1410),
    ("tiles/font/frame.png", 0x1600),
];

/// Frames of the licence screen (scenes.md 1).
const LICENCE_FRAMES: u32 = 512;
const PRESET_LICENCE: usize = 0;

/// The title menu's two choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Select {
    StartGame,
    Continue,
}

impl Select {
    pub fn toggled(self) -> Select {
        match self {
            Select::StartGame => Select::Continue,
            Select::Continue => Select::StartGame,
        }
    }

    /// 0 for the first line, 1 for the second.
    pub fn row(self) -> i32 {
        match self {
            Select::StartGame => 0,
            Select::Continue => 1,
        }
    }
}

/// What a scene asks the sound driver for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SoundRequest {
    /// Whatever the sound driver is asked for in play and by the typewriter (sound.md 8).
    Request(crate::game::sound::Request),
    /// Start the song unless it was the last one started this way (sound.md 8.1).
    SongIfNew(u8),
    /// Start the song unless it is playing, and again whenever it ends, until another song
    /// starts (the title song, which the original plays once).
    SongLooped(u8),
    /// Pause: switch the sound channels off, the driver stands still (game-flow.md 4).
    Pause,
    /// Leave the pause: the channels on again, the song goes on where it was.
    Resume,
}

impl SoundRequest {
    /// An effect on both pulses together: slot A on pulse 1, slot B on pulse 2.
    pub fn both(id: u8) -> [SoundRequest; 2] {
        use crate::game::sound::{Channel, Request};
        [
            SoundRequest::Request(Request::a(Channel::Pulse1, id)),
            SoundRequest::Request(Request::b(Channel::Pulse2, id)),
        ]
    }
}

/// The frame counter ($5D-$5F), the job counter ($60) and the random generator, which scenes
/// share (`game-flow.md` 1.5, 5).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Clock {
    pub counter: u32,
    pub job: u8,
    pub rng: Rng,
}

impl Clock {
    /// One frame of a scene that steps the generator after its own increment (the title):
    /// the job counter advances, the counter goes up and the generator steps with the
    /// controller bytes and the counters.
    pub fn advance(&mut self, held: Buttons, pressed: Buttons) {
        self.job = (self.job + 1) & 7;
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        let c = self.counter;
        self.rng.step([
            held.0,
            pressed.0,
            0,
            0,
            c as u8,
            (c >> 8) as u8,
            (c >> 16) as u8,
            self.job,
        ]);
    }
}

impl Clock {
    /// One frame of a scene whose generator step sits in the status-bar split: it sees the
    /// counters from before the pass's own increment, and the job counter after its advance
    /// (`game-flow.md` 5, "Where it steps": gameplay, messages, the interludes).
    pub fn advance_split(&mut self, held: Buttons, pressed: Buttons) {
        self.job = (self.job + 1) & 7;
        let c = self.counter;
        self.rng.step([
            held.0,
            pressed.0,
            0,
            0,
            c as u8,
            (c >> 8) as u8,
            (c >> 16) as u8,
            self.job,
        ]);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
    }
}

/// What every scene needs from the files: the map and object data and the imported graphics.
#[derive(Debug, Clone)]
pub struct Assets {
    pub world: World,
    pub graphics: Graphics,
    pub layouts: SpriteFile,
}

/// Where a new [`Game`] starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Start {
    /// Power-on: the licence screen, then the title.
    Cold,
    /// A restart of the program: the title.
    Warm,
    /// Straight into gameplay on a level, with a new game's state (development).
    Level(usize),
    /// The final level, entered as after the last warpship piece but with a new game's state
    /// (the port menu's level select).
    Final,
    /// The attract story (development and comparison with the original).
    Story,
    /// The interplanetary flight to a level (not 0) with a new game's state (comparison with the
    /// original).
    Flight(usize),
    /// The attract demo number 0-2 (levels 6, 4 and 10).
    Demo(usize),
    /// The launch scene of a level with a new game's state, as it runs after the surface
    /// picture, whose frame counter then holds `counter` (comparing the scene with the
    /// original).
    Launch { level: usize, counter: u32 },
    /// The approach screen of a level with a new game's state.
    Approach(usize),
    /// The surface of a planet: the descent (`dock` false) or the menu after docking, with
    /// something owned (`owned`) and the planet's flag set (`flag`) or not (comparing the
    /// scene with the original).
    Surface {
        level: usize,
        dock: bool,
        owned: bool,
        flag: bool,
        /// The level has nothing left to collect (with the flag: an automatic blast-off).
        clear: bool,
    },
    /// The shop of a planet's flight with `score` to spend and the frame counter at `counter`
    /// (comparing the scene with the original).
    Shop {
        level: usize,
        score: u32,
        counter: u32,
        /// The mapping device is owned (a password can give it).
        mapping: bool,
    },
    /// The equip list on a planet with one of each item whose bit is set in `owned` (comparing
    /// the scene with the original).
    Equip {
        level: usize,
        owned: u16,
        counter: u32,
    },
    /// Gameplay on a level with a new game's state and the mapping devices as given, for
    /// pausing it and opening the planet map (comparing the scene with the original).
    Map {
        level: usize,
        mapping: bool,
        super_mapping: bool,
        score: u32,
        lives: u8,
        /// RAM $DC, and the fitted weapon ($D1) and device ($D3).
        equipment: u8,
        weapon: u8,
        device: u8,
    },
    /// The backdrop interlude of a level (0, 4 or 8), the pod at world y `pod_y`, with the
    /// frame counter at `counter` and the rotating job at `job` (comparing it with the original).
    Interlude {
        level: usize,
        pod_y: u16,
        counter: u32,
        job: u8,
    },
    /// The badge intermission after a level, counting `gems` crystals, the frame counter at
    /// `counter` (comparing it with the original).
    Badge {
        level: usize,
        gems: u8,
        counter: u32,
    },
    /// A message screen over the gameplay of a level, with a new game's state and the frame
    /// counter at `counter` (the kinds that do not reset it).
    Message {
        level: usize,
        subject: Subject,
        counter: u32,
    },
    /// The last-password screen after a game over on a level with a new game's state
    /// (comparing it with the original).
    LastPassword(usize),
}

/// `$body` with `$game` bound to the play's gameplay, whichever engine it is: the Scaled
/// engine is the Original's copy, with the same names.
macro_rules! either {
    ($play:expr, $game:ident => $body:expr) => {
        match &$play.engine {
            Engine::Original($game) => $body,
            Engine::Scaled($game) => $body,
        }
    };
}

/// [`either`] with the gameplay mutable.
macro_rules! either_mut {
    ($play:expr, $game:ident => $body:expr) => {
        match &mut $play.engine {
            Engine::Original($game) => $body,
            Engine::Scaled($game) => $body,
        }
    };
}

/// The engine that plays a level (`docs/scaled.md`).
#[derive(serde::Serialize, serde::Deserialize)]
#[allow(clippy::large_enum_variant)] // one value per level
enum Engine {
    /// The original's 30 Hz logic step, exactly.
    Original(Gameplay),
    /// Physics at the display's rate (`crate::game::scaled`).
    Scaled(Box<Scaled>),
}

/// Gameplay with its drawing state.
#[derive(serde::Serialize, serde::Deserialize)]
struct Play {
    engine: Engine,
    sprites: Sprites,
    graphics: LevelGraphics,
}

impl From<Gameplay> for Engine {
    fn from(game: Gameplay) -> Engine {
        Engine::Original(game)
    }
}

impl From<Scaled> for Engine {
    fn from(game: Scaled) -> Engine {
        Engine::Scaled(Box::new(game))
    }
}

impl Play {
    fn original(&self) -> Option<&Gameplay> {
        match &self.engine {
            Engine::Original(game) => Some(game),
            Engine::Scaled(_) => None,
        }
    }

    fn original_mut(&mut self) -> Option<&mut Gameplay> {
        match &mut self.engine {
            Engine::Original(game) => Some(game),
            Engine::Scaled(_) => None,
        }
    }

    fn level(&self) -> usize {
        match &self.engine {
            Engine::Original(game) => game.level(),
            Engine::Scaled(scaled) => scaled.level(),
        }
    }

    fn hud(&self) -> Hud {
        match &self.engine {
            Engine::Original(game) => game.hud(),
            Engine::Scaled(scaled) => scaled.hud(),
        }
    }

    fn counter(&self) -> u32 {
        match &self.engine {
            Engine::Original(game) => game.counter(),
            Engine::Scaled(scaled) => scaled.counter(),
        }
    }

    fn draw(&self, frame: &mut Frame, assets: &Assets, blend: f32) {
        match &self.engine {
            Engine::Original(game) => game.draw(
                frame,
                &assets.world,
                &self.graphics,
                &self.sprites,
                &assets.layouts,
                blend,
            ),
            Engine::Scaled(scaled) => scaled.draw(
                frame,
                &assets.world,
                &self.graphics,
                &self.sprites,
                &assets.layouts,
                blend,
            ),
        }
    }
}

/// The launch scene and what follows it: gameplay on `level` with `seed`.
#[derive(serde::Serialize, serde::Deserialize)]
struct LaunchScene {
    launch: Launch,
    level: usize,
    seed: Box<Seed>,
}

/// The surface scene and the level it is on.
#[derive(serde::Serialize, serde::Deserialize)]
struct SurfaceScene {
    surface: Surface,
    level: usize,
}

/// The planet map over the paused gameplay it came from.
#[derive(serde::Serialize, serde::Deserialize)]
struct MapScene {
    map: PlanetMap,
    play: Box<Play>,
}

/// A message screen over the gameplay it interrupted.
#[derive(serde::Serialize, serde::Deserialize)]
struct MessageScene {
    message: Message,
    play: Box<Play>,
}

/// The backdrop interlude over the gameplay it interrupted.
#[derive(serde::Serialize, serde::Deserialize)]
struct InterludeScene {
    interlude: Interlude,
    play: Box<Play>,
}

/// The badge intermission over the gameplay it interrupted.
#[derive(serde::Serialize, serde::Deserialize)]
struct BadgeScene {
    badge: Badge,
    play: Box<Play>,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Scene {
    Licence {
        screen: Box<Screen>,
        frames: u32,
    },
    Title(Box<Title>),
    Entry(Box<Entry>),
    Flight(Box<Flight>),
    Story(Box<Story>),
    Launch(Box<LaunchScene>),
    Approach(Box<Approach>),
    Surface(Box<SurfaceScene>),
    Shop(Box<Shop>),
    Map(Box<MapScene>),
    Message(Box<MessageScene>),
    Interlude(Box<InterludeScene>),
    Badge(Box<BadgeScene>),
    /// The ending picture and the credits after the final message (scenes.md 14).
    Ending(Box<Ending>),
    Credits(Box<Credits>),
    Play(Box<Play>),
    /// The frozen playfield with the bobbing letters of "GAME OVER" (scenes.md 12).
    GameOver(Box<GameOverScene>),
    /// "YOUR LAST PASSWORD WAS" and the letters over the gameplay's status panel, until a
    /// button (scenes.md 12).
    LastPassword {
        screen: Box<Screen>,
        /// A button was pressed: the screen stays one more pass, then the program restarts.
        pressed: bool,
    },
}

/// The game-over animation: the playfield as it was, a clock and the letters' tiles.
#[derive(serde::Serialize, serde::Deserialize)]
struct GameOverScene {
    play: Box<Play>,
    /// The status panel as gameplay left it, which the last-password screen keeps.
    hud: Hud,
    /// The gameplay's palettes, the base of the last-password screen's.
    palette: Palette,
    /// The number of the level whose gameplay's icon tiles of the status panel (the jetpod's
    /// and Jetman's, which differ from the other scenes' copies) the last-password screen keeps.
    level: u8,
    /// The scene's clock c, 0 in its first frame.
    counter: u32,
    /// The letters' tiles (CHR table 0).
    chr: Chr,
}

/// How long an attract demo plays at most, in logic steps: two minutes, about as long as the
/// original's longest (game-flow.md 7.1).
const DEMO_STEPS: f64 = 120.0 * 30.0;

/// The attract demo under way: the pilot plays (`crate::game::pilot`), and a newly pressed
/// real button ends it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AttractDemo {
    pilot: Pilot,
    /// The real controller as last read, and whether a button was newly pressed.
    real_held: Buttons,
    pressed: bool,
}

impl AttractDemo {
    /// The pilot's buttons for this frame or tick; `seen` is the real controller, with
    /// anything pressed since the last read.
    fn input(&mut self, world: &World, engine: &Engine, seen: Buttons, held: Buttons) -> Buttons {
        self.pressed |= seen.pressed_since(self.real_held).0 != 0;
        self.real_held = held;
        let observation = match engine {
            Engine::Original(game) => game.observe(),
            Engine::Scaled(game) => game.observe(),
        };
        self.pilot.decide(world, &observation)
    }

    /// Whether the demo is over: a button was pressed, the time is up, the pod is lost (Jetman
    /// is out on his own, game-flow.md 7.1) or gameplay asks for a scene change (a delivery).
    fn over(&self, play: &Play) -> bool {
        let (time, state, transition) = match &play.engine {
            Engine::Original(game) => (
                f64::from(game.counter()) / 2.0,
                game.pod.state,
                game.transition().is_some(),
            ),
            Engine::Scaled(game) => (
                game.clock().time() as f64 / f64::from(1u32 << 31) / 2.0,
                game.pod.state,
                game.transition().is_some(),
            ),
        };
        self.pressed || time >= DEMO_STEPS || state == 2 || transition
    }
}

/// Logic steps of gameplay behind a forced scene in a comparison with the original, during
/// which the fuel gauge fills up (about 190 frames of the level).
const FUEL_STEPS: usize = 98;

/// What a save state holds, to write.
#[derive(serde::Serialize)]
struct SaveRef<'a> {
    game: &'a Game,
    sound: Option<&'a Player>,
}

/// What a save state holds, to read.
#[derive(serde::Deserialize)]
struct Save {
    game: Game,
    sound: Option<Player>,
}

/// The game: the current scene and what lasts between scenes.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Game {
    scene: Scene,
    clock: Clock,
    /// The state a new level starts from, and what the last approach screen would have shown as
    /// the password.
    saved: Saved,
    last_password: [u8; 12],
    /// What the next level starts with: lives, score, equipment and the rest of the player's
    /// state (rebuilt from a password, handed on by gameplay at a docking).
    carry: Seed,
    /// The owned counts, which the surface menu looks at (EQUIP POD is offered with any).
    inventory: Inventory,
    held: Buttons,
    sound: Vec<SoundRequest>,
    /// RAM $0512: attract timeouts (game-flow.md 7.1).
    attract_timeouts: u8,
    /// The attract demo, while gameplay is one.
    #[serde(default)]
    demo: Option<AttractDemo>,
    /// The validation mode: the original's rules where `docs/deviations.md` lists a deviation
    /// (Q34-Q36 in the scenes, the gameplay ones in [`Gameplay`]). Off in play.
    original_rules: bool,
    /// Bytes the validation mode reads from the user's ROM (`Gameplay::set_validation_input`).
    #[serde(skip)]
    validation: Option<crate::game::gameplay::ValidationInput>,
    /// The port menu's cheats, handed to the gameplay (not part of a saved state).
    #[serde(skip)]
    cheats: Cheats,
    /// The port menu's choice of engine for the levels started from now on: the Scaled engine
    /// when set (not part of a saved state, which holds the engine it was saved with).
    #[serde(skip)]
    scaled: bool,
    /// The port menu's pod rest for the Scaled engine (not part of a saved state).
    #[serde(skip)]
    pod_rest: bool,
    /// Any button ends the licence screen, the ending picture and the credits (an addition the
    /// program always turns on; the tools leave it off). Not part of a saved state.
    #[serde(skip)]
    skip_scenes: bool,
    /// The view's width for gameplay (`docs/widescreen.md`), chosen by the program (not part
    /// of a saved state).
    #[serde(skip, default = "crate::game::view::original_width")]
    view_width: u16,
}

impl Game {
    pub fn new(assets: &Assets, start: Start) -> Result<Game, MissingGraphics> {
        Game::with_rules(assets, start, false)
    }

    /// Like [`Game::new`], in the validation mode when `original_rules` (see
    /// [`Game::set_original_rules`]); it must be given here for the first scene to follow it.
    pub fn with_rules(
        assets: &Assets,
        start: Start,
        original_rules: bool,
    ) -> Result<Game, MissingGraphics> {
        Game::build(assets, start, original_rules, false)
    }

    /// Like [`Game::new`], with the levels in the Scaled engine when `scaled` (see
    /// [`Game::set_scaled`]); given here, it applies to a start on a level too.
    pub fn with_engine(
        assets: &Assets,
        start: Start,
        scaled: bool,
    ) -> Result<Game, MissingGraphics> {
        Game::build(assets, start, false, scaled)
    }

    fn build(
        assets: &Assets,
        start: Start,
        original_rules: bool,
        scaled: bool,
    ) -> Result<Game, MissingGraphics> {
        let mut game = Game {
            // Replaced just below.
            scene: Scene::Licence {
                screen: Box::default(),
                frames: 0,
            },
            clock: Clock::default(),
            saved: Saved::NEW_GAME,
            last_password: Saved::NEW_GAME.encode(),
            carry: Seed::default(),
            inventory: Inventory::default(),
            held: Buttons::default(),
            sound: Vec::new(),
            attract_timeouts: 0,
            demo: None,
            original_rules,
            validation: None,
            cheats: Cheats::default(),
            scaled,
            pod_rest: false,
            skip_scenes: false,
            view_width: crate::game::view::ORIGINAL_WIDTH,
        };
        match start {
            Start::Cold => game.scene = game.licence(assets)?,
            Start::Warm => game.open_title(assets)?,
            Start::Story => game.open_story(assets)?,
            Start::Demo(index) => game.open_demo(assets, index)?,
            Start::Flight(level) => {
                game.saved.level = level.clamp(1, 255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.start_level(assets)?;
            }
            Start::Level(level) => {
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.begin_carried_level(assets)?;
            }
            Start::Final => {
                game.saved.level = crate::game::gameplay::final_level::FINAL_LEVEL;
                game.carry = game.seed_from(&game.saved);
                game.carry.final_entry = true;
                game.begin_carried_level(assets)?;
            }
            Start::Launch { level, counter } => {
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.clock.counter = counter;
                game.open_launch(assets)?;
            }
            Start::Approach(level) => {
                game.saved.level = level.min(13) as u8;
                game.carry = game.seed_from(&game.saved);
                game.open_approach(assets)?;
            }
            Start::Surface {
                level,
                dock,
                owned,
                flag,
                clear,
            } => {
                game.saved.level = level.min(12) as u8;
                game.carry = game.seed_from(&game.saved);
                if owned {
                    game.inventory.weapon = 1;
                }
                if flag {
                    game.carry.planet_flags |= 1 << flag_bit(level);
                }
                if dock {
                    game.open_docked(assets, !clear)?;
                } else {
                    // The approach screen comes first and leaves its tiles behind.
                    game.open_approach(assets)?;
                    let previous = game.current_screen().cloned();
                    game.open_landing(assets, previous.as_ref())?;
                }
            }
            Start::Shop {
                level,
                score,
                counter,
                mapping,
            } => {
                game.saved.level = level.min(12) as u8;
                game.carry = game.seed_from(&game.saved);
                game.carry.score = Score(score);
                game.carry.mapping_device = mapping;
                let flight = Flight::new(&assets.graphics, &mut game.clock)?;
                let previous = flight.screen().clone();
                game.clock.counter = counter;
                game.open_shop(assets, Mode::Shop, Some(&previous))?;
            }
            Start::Equip {
                level,
                owned,
                counter,
            } => {
                game.saved.level = level.min(12) as u8;
                game.carry = game.seed_from(&game.saved);
                for item in 0..12 {
                    if owned & (1 << item) != 0 {
                        game.inventory.counts[item] = 1;
                    }
                }
                game.open_docked(assets, true)?;
                let previous = game.current_screen().cloned();
                game.clock.counter = counter;
                game.open_shop(assets, Mode::Equip, previous.as_ref())?;
            }
            Start::Map {
                level,
                mapping,
                super_mapping,
                score,
                lives,
                equipment,
                weapon,
                device,
            } => {
                game.saved.level = level.min(12) as u8;
                game.carry = game.seed_from(&game.saved);
                game.carry.mapping_device = mapping;
                game.carry.super_mapping_device = super_mapping;
                game.carry.score = Score(score);
                game.carry.lives = lives;
                game.carry.jetpods = lives;
                game.carry.equipment = equipment;
                game.carry.weapon = weapon;
                game.carry.device = device;
                game.begin_carried_level(assets)?;
            }
            Start::LastPassword(level) => {
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.begin_carried_level(assets)?;
                if let Scene::Play(play) = game.take_scene() {
                    // The fuel gauge has filled up by the time a game over comes.
                    let hud = Hud {
                        fuel: 0xE0,
                        ..play.hud()
                    };
                    game.last_password(assets, hud, play.graphics.palette, play.graphics.number)?;
                }
            }
            Start::Interlude {
                level,
                pod_y,
                counter,
                job,
            } => {
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.begin_carried_level(assets)?;
                if let Scene::Play(mut play) = game.take_scene() {
                    either_mut!(play, game => game.pod.body.y = pod_y);
                    game.clock.counter = counter;
                    game.clock.job = job;
                    game.open_interlude(assets, play)?;
                }
            }
            Start::Badge {
                level,
                gems,
                counter,
            } => {
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.begin_carried_level(assets)?;
                if let Scene::Play(mut play) = game.take_scene() {
                    // The fuel gauge fills a little at every step of the level before.
                    for _ in 0..FUEL_STEPS {
                        either_mut!(play, game => game.pod.ease_fuel_display());
                    }
                    game.clock.counter = counter;
                    game.open_badge(assets, play, gems)?;
                }
            }
            Start::Message {
                level,
                subject,
                counter,
            } => {
                game.clock.counter = counter;
                game.saved.level = level.min(255) as u8;
                game.carry = game.seed_from(&game.saved);
                game.begin_carried_level(assets)?;
                if let Scene::Play(play) = std::mem::replace(
                    &mut game.scene,
                    Scene::Licence {
                        screen: Box::default(),
                        frames: 0,
                    },
                ) {
                    let mut play = play;
                    if subject.kind == 7 {
                        // The final level's end routine has asked for the ending.
                        either_mut!(play, game => game.request_scene_change(Transition::Ending));
                    }
                    game.open_message(assets, play, subject)?;
                }
            }
        }
        Ok(game)
    }

    fn licence(&self, assets: &Assets) -> Result<Scene, MissingGraphics> {
        let mut screen = Screen::default();
        screen.load_tiles(&assets.graphics, &FONT_LOADS)?;
        screen.palette.apply(
            assets
                .graphics
                .presets
                .get(PRESET_LICENCE)
                .ok_or_else(|| MissingGraphics("palette preset 0".to_owned()))?,
        );
        for (column, row, text) in [
            (6, 3, "TRADEWEST, INC. PRESENTS"),
            (10, 8, "SOLAR JETMAN™"),
            (2, 10, "HUNT FOR THE GOLDEN WARPSHIP"),
            (8, 13, "© 1990 RARE LTD."),
            (3, 18, "LICENSED TO TRADEWEST, INC."),
            (6, 20, "BY RARE COIN-IT, INC."),
            (10, 23, "LICENSED BY"),
            (4, 25, "NINTENDO OF AMERICA INC."),
        ] {
            screen.print(column, row, text);
        }
        // No scroll is written, but the picture generator's address leaves a fine scroll of 2
        // (graphics.md 1.1).
        screen.scroll = (0, 2);
        screen.blank_left = true;
        screen.blank_left_sprites = true;
        Ok(Scene::Licence {
            screen: Box::new(screen),
            frames: 0,
        })
    }

    /// The attract demo (game-flow.md 7.1, scenes.md 3.1): ordinary gameplay on level 6, 4 or 10
    /// with a new game's state, probe shields and boosters, and the original's weapon of the
    /// demo; no fade. The remake's pilot flies it in the engine chosen, with the level song and
    /// effects (`docs/deviations.md`).
    fn open_demo(&mut self, assets: &Assets, index: usize) -> Result<(), MissingGraphics> {
        const LEVELS: [usize; 3] = [6, 4, 10];
        let level = LEVELS[index.min(2)];
        let saved = Saved::NEW_GAME;
        let seed = Seed {
            rng: self.clock.rng,
            score: Score(saved.score_value()),
            lives: saved.lives,
            jetpods: saved.lives,
            equipment: 0xC0,
            weapon: [1, 8, 6][index.min(2)],
            ..Seed::default()
        };
        self.begin_play(assets, level, 0, seed, true)
    }

    fn open_story(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        let story = Story::new(&assets.graphics, &mut self.clock)?;
        self.scene = Scene::Story(Box::new(story));
        Ok(())
    }

    fn open_title(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        let mut title = Title::new(&assets.graphics, &mut self.clock)?;
        self.sound.extend(title.take_song_request());
        self.scene = Scene::Title(Box::new(title));
        Ok(())
    }

    /// The new-game state (game-flow.md 7.2).
    fn new_game(&mut self) {
        self.saved = Saved::NEW_GAME;
        self.carry = self.seed_from(&self.saved);
        self.inventory = Inventory::default();
    }

    /// The state a new game or a password starts from (game-flow.md 7.2).
    fn seed_from(&self, saved: &Saved) -> Seed {
        let level = usize::from(saved.level);
        Seed {
            rng: self.clock.rng,
            score: Score(saved.score_value()),
            lives: saved.lives,
            jetpods: saved.lives,
            equipment: saved.equipment,
            mapping_device: saved.mapping_device,
            super_mapping_device: saved.super_mapping_device,
            planet_flags: (1u16 << level.min(12)) - 1,
            ..Seed::default()
        }
    }

    /// The level the game is at (RAM $3B).
    fn current_level(&self) -> usize {
        usize::from(self.saved.level)
    }

    /// START LEVEL (game-flow.md 7.2, 7.3): level 0 goes straight to the approach screen, any
    /// other level shows the flight first (and, on odd levels, the shop: a stub).
    fn start_level(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        if self.current_level() == 0 {
            self.open_approach(assets)
        } else {
            let flight = Flight::new(&assets.graphics, &mut self.clock)?;
            let mut flight = flight;
            self.sound.extend(flight.take_song_request());
            self.scene = Scene::Flight(Box::new(flight));
            Ok(())
        }
    }

    /// The password the state holds on level `level` (the approach screen computes it,
    /// game-flow.md 7.3).
    fn password_of(seed: &Seed, level: usize) -> Saved {
        Saved {
            level: level.min(63) as u8,
            lives: seed.lives,
            score: [0; 6],
            equipment: seed.equipment,
            mapping_device: seed.mapping_device,
            super_mapping_device: seed.super_mapping_device,
        }
        .with_score(seed.score.0)
    }

    /// The approach screen (scenes.md 7): the flags are rebuilt as "all planets before this
    /// one" (game-flow.md 6), the password for the state is computed and kept.
    fn open_approach(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        let level = self.current_level();
        self.carry.planet_flags = (1u16 << level.min(12)) - 1;
        self.saved = Game::password_of(&self.carry, level);
        self.last_password = self.saved.encode();
        let world_level = assets.world.levels.get(level.min(12));
        let facts = Facts {
            level,
            gravity: world_level.map_or(0, |l| i32::from(l.gravity)),
            diameter: world_level.map_or(1, |l| i32::from(l.width) - 1),
            password: &self.last_password,
            lives: self.carry.lives,
            jetpods: self.carry.jetpods,
            score: self.carry.score.0,
        };
        let approach = Approach::new(&assets.graphics, &facts, None, self.original_rules)?;
        self.sound
            .push(SoundRequest::SongIfNew(approach_song(level)));
        self.scene = Scene::Approach(Box::new(approach));
        Ok(())
    }

    /// Nothing owned or fitted (game-flow.md 7.3): no EQUIP POD is offered.
    fn nothing_owned(&self) -> bool {
        let inventory = &self.inventory;
        inventory.counts[..12].iter().all(|&n| n == 0)
            && inventory.weapon == 0
            && inventory.device == 0
            && inventory.bullets == 0
    }

    /// The landing: the mothership descends onto the planet's picture (scenes.md 8.2).
    fn open_landing(
        &mut self,
        assets: &Assets,
        previous: Option<&Screen>,
    ) -> Result<(), MissingGraphics> {
        let level = self.current_level().min(12);
        let mut surface = Surface::landing(
            &assets.graphics,
            &assets.world.levels[level],
            level,
            previous,
            self.nothing_owned(),
        )?;
        surface.original_rules = self.original_rules;
        surface.show(&assets.layouts);
        self.clock.counter = 0;
        self.sound.extend(surface.take_song());
        self.scene = Scene::Surface(Box::new(SurfaceScene { surface, level }));
        Ok(())
    }

    /// The surface menu after docking (game-flow.md 7.3 mode 3). `collectables` says whether
    /// the level still holds something to collect.
    fn open_docked(&mut self, assets: &Assets, collectables: bool) -> Result<(), MissingGraphics> {
        let level = self.current_level().min(12);
        let flag = self.carry.planet_flags & (1 << flag_bit(level)) != 0;
        let mut surface = Surface::docked(
            &assets.graphics,
            &assets.world.levels[level],
            level,
            None,
            self.nothing_owned(),
            flag,
            !collectables,
        )?;
        surface.original_rules = self.original_rules;
        surface.show(&assets.layouts);
        self.clock.counter = 0;
        self.sound.extend(surface.take_song());
        self.scene = Scene::Surface(Box::new(SurfaceScene { surface, level }));
        Ok(())
    }

    /// The shop (after the flight to an odd planet) or the equip list (EQUIP POD): what is
    /// fitted goes back to the counts first (economy.md 6.4).
    fn open_shop(
        &mut self,
        assets: &Assets,
        mode: Mode,
        previous: Option<&Screen>,
    ) -> Result<(), MissingGraphics> {
        let level = self.current_level().min(12);
        if mode == Mode::Shop {
            self.inventory.take_from_seed(&self.carry);
        }
        self.inventory.unfit_all();
        self.clock.job = 0;
        let shop = Shop::new(
            &assets.graphics,
            mode,
            level,
            &self.inventory,
            self.carry.score,
            previous,
        )?;
        self.scene = Scene::Shop(Box::new(shop));
        Ok(())
    }

    /// The planet map over the paused gameplay `play` (scenes.md 13.2): what the panel shows,
    /// the markers and the colours come from gameplay.
    fn open_map(&mut self, assets: &Assets, play: Box<Play>) -> Result<(), MissingGraphics> {
        let level = &assets.world.levels[play.level()];
        let (player, spots, super_device, counter) = either!(play, game => {
            let items = &game.objects.items;
            let held = game.tow.slot;
            let mut spots = Vec::new();
            for (index, entry) in items.iter().enumerate().skip(1) {
                if entry.kind == 0 {
                    continue;
                }
                // The item on the beam is where it is now.
                let live = (held != 0)
                    .then(|| game.objects.slots[held])
                    .filter(|o| usize::from(o.index) == index && o.state != 0);
                spots.push(live.map_or((entry.x, entry.y), |o| (o.x, o.y)));
            }
            (
                (game.pod.body.x, game.pod.body.y),
                spots,
                game.super_mapping_device,
                game.counter(),
            )
        });
        let facts = MapFacts {
            original_rules: self.original_rules,
            player,
            items: spots,
            super_device,
            hud: play.hud(),
        };
        let map = PlanetMap::new(
            &assets.graphics,
            &assets.world,
            level,
            &play.graphics.palette,
            &facts,
        )?;
        self.clock.counter = counter;
        // The channels are silent; song 6 is started, always (sound.md 8.1, scenes.md 13.2).
        self.sound
            .push(SoundRequest::Request(crate::game::sound::Request::Song {
                id: 6,
                always: true,
            }));
        self.sound.push(SoundRequest::Resume);
        self.scene = Scene::Map(Box::new(MapScene { map, play }));
        Ok(())
    }

    /// The launch scene (scenes.md 9) into gameplay on the current level with the carried
    /// state; the pod's type is the equipment's low two bits.
    fn open_launch(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        let level = self.current_level().min(assets.world.levels.len() - 1);
        self.open_launch_with(assets, level, Box::new(self.carry.clone()))
    }

    fn open_launch_with(
        &mut self,
        assets: &Assets,
        level: usize,
        seed: Box<Seed>,
    ) -> Result<(), MissingGraphics> {
        let mut launch = Launch::new(&assets.graphics, seed.equipment & 3)?;
        launch.original_rules = self.original_rules;
        launch.show(&assets.layouts);
        self.scene = Scene::Launch(Box::new(LaunchScene {
            launch,
            level,
            seed,
        }));
        Ok(())
    }

    /// Gameplay on the current level with the carried state.
    fn begin_carried_level(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        let level = self.current_level().min(assets.world.levels.len() - 1);
        let mut seed = self.carry.clone();
        seed.rng = self.clock.rng;
        self.begin_play(assets, level, seed.equipment & 3, seed, false)
    }

    /// A message screen over `play`.
    fn open_message(
        &mut self,
        assets: &Assets,
        play: Box<Play>,
        subject: Subject,
    ) -> Result<(), MissingGraphics> {
        let mut hud = play.hud();
        // The delivery's effects on the score and the counts show only after the screen.
        match subject.text {
            25 => hud.score = hud.score.saturating_sub(500),
            50 => {
                hud.score = hud.score.saturating_sub(2000);
                hud.lives = hud.lives.saturating_sub(1);
                hud.jetpods = hud.jetpods.saturating_sub(1);
            }
            _ => {}
        }
        let mut message = Message::new(
            &assets.graphics,
            &assets.world.levels[play.level()],
            &play.graphics.palette,
            subject,
            hud,
            &mut self.clock,
        )?;
        message.original_rules = self.original_rules;
        // The piece comes from a second level (13 and up); elsewhere there is none to show.
        if play.level() >= 13 {
            message.set_warpship(
                either!(play, game => game.planet_flags),
                flag_bit(play.level()),
            );
        }
        self.scene = Scene::Message(Box::new(MessageScene { message, play }));
        Ok(())
    }

    /// The backdrop interlude over `play` (scenes.md 11.1): the pod climbs from where it is.
    fn open_interlude(&mut self, assets: &Assets, play: Box<Play>) -> Result<(), MissingGraphics> {
        let interlude = Interlude::new(
            &assets.graphics,
            play.level(),
            either!(play, game => game.pod.equipment.owned & 3),
            either!(play, game => game.pod.body.y),
            &play.graphics.palette,
            play.hud(),
            self.original_rules,
        )?;
        self.sound.push(interlude.song());
        self.scene = Scene::Interlude(Box::new(InterludeScene { interlude, play }));
        Ok(())
    }

    /// The badge intermission over `play` (scenes.md 11.2) counting `gems`.
    fn open_badge(
        &mut self,
        assets: &Assets,
        play: Box<Play>,
        gems: u8,
    ) -> Result<(), MissingGraphics> {
        let badge = Badge::new(
            &assets.graphics,
            &crate::game::graphics::gameplay_background_loads(&assets.world.levels[play.level()]),
            &play.graphics.palette,
            play.hud(),
            gems,
        )?;
        self.sound.push(badge.song());
        self.scene = Scene::Badge(Box::new(BadgeScene { badge, play }));
        Ok(())
    }

    fn begin_play(
        &mut self,
        assets: &Assets,
        level: usize,
        pod_type: u8,
        seed: Seed,
        demo: bool,
    ) -> Result<(), MissingGraphics> {
        let sprites = Sprites::new(&assets.graphics, &assets.layouts, level, pod_type)?;
        let graphics = LevelGraphics::new(&assets.graphics, &assets.world.levels[level])?;
        let mut sprites = sprites;
        sprites.original_rules = self.original_rules;
        let mut seed = seed;
        seed.view_width = self.view_width;
        let engine = if self.scaled {
            Engine::Scaled(Box::new(Scaled::new(&assets.world, level, seed)))
        } else {
            let mut game = Gameplay::new(&assets.world, level, seed);
            game.set_original_rules(self.original_rules);
            game.set_validation_input(self.validation.clone());
            Engine::Original(game)
        };
        self.demo = demo.then(|| AttractDemo {
            pilot: Pilot::new(),
            real_held: self.held,
            pressed: false,
        });
        self.sound.push(SoundRequest::SongIfNew(planet_song(level)));
        self.scene = Scene::Play(Box::new(Play {
            engine,
            sprites,
            graphics,
        }));
        Ok(())
    }

    /// Gameplay on the level a change of level (code 3) led to: the game was built by
    /// [`Gameplay::change_level`], the level's own song starts (scenes.md 0.1).
    fn enter_level(&mut self, assets: &Assets, mut engine: Engine) -> Result<(), MissingGraphics> {
        let (level, pod_type) = match &engine {
            Engine::Original(game) => (game.level(), game.pod.equipment.owned & 3),
            Engine::Scaled(game) => (game.level(), game.pod.equipment.owned & 3),
        };
        let mut sprites = Sprites::new(&assets.graphics, &assets.layouts, level, pod_type)?;
        sprites.original_rules = self.original_rules;
        let graphics = LevelGraphics::new(&assets.graphics, &assets.world.levels[level])?;
        if let Engine::Original(game) = &mut engine {
            game.set_original_rules(self.original_rules);
            game.set_validation_input(self.validation.clone());
        }
        self.sound.push(SoundRequest::SongIfNew(planet_song(level)));
        self.scene = Scene::Play(Box::new(Play {
            engine,
            sprites,
            graphics,
        }));
        Ok(())
    }

    /// The game-over animation over the playfield as it stands (scenes.md 12).
    fn game_over(&mut self, assets: &Assets, play: Box<Play>) -> Result<(), MissingGraphics> {
        let mut chr = Chr::default();
        chr.apply(
            &assets.graphics,
            &[ChrLoad {
                stream: gameover::TILES,
                range: None,
                address: 0,
            }],
        )?;
        self.scene = Scene::GameOver(Box::new(GameOverScene {
            hud: play.hud(),
            palette: play.graphics.palette,
            level: play.graphics.number,
            play,
            counter: 0,
            chr,
        }));
        Ok(())
    }

    /// The last password screen (scenes.md 12): the letters on a black playfield, the status
    /// panel as gameplay left it.
    fn last_password(
        &mut self,
        assets: &Assets,
        hud: Hud,
        palette: Palette,
        level: u8,
    ) -> Result<(), MissingGraphics> {
        let mut screen = Screen::default();
        screen.load_tiles(&assets.graphics, &FONT_LOADS)?;
        message::load_panel(&mut screen, &assets.graphics, hud)?;
        let world_level = assets
            .world
            .levels
            .iter()
            .find(|l| l.number == level)
            .ok_or_else(|| MissingGraphics(format!("level {level}")))?;
        screen.chr.copy_panel_icons(&assets.graphics, world_level)?;
        screen.refresh();
        screen.palette = palette;
        screen.print(5, 9, "YOUR LAST PASSWORD WAS");
        for (i, &letter) in self.last_password.iter().enumerate() {
            screen.set_tile(10 + i, 12, letter);
        }
        self.scene = Scene::LastPassword {
            screen: Box::new(screen),
            pressed: false,
        };
        Ok(())
    }

    /// A warm reset: the RAM is cleared, the licence screen is skipped.
    fn warm_reset(&mut self, assets: &Assets) -> Result<(), MissingGraphics> {
        self.clock = Clock::default();
        self.saved = Saved::NEW_GAME;
        self.attract_timeouts = 0;
        self.open_title(assets)
    }

    /// Sets the random generator's state (comparison with the original, which starts a scene
    /// with whatever the scenes before left).
    /// Switches the validation mode: the original's rules instead of the remake's wherever
    /// `docs/deviations.md` lists a deviation, so scene comparisons can run either way. Takes
    /// effect for scenes opened afterwards, and for the gameplay running now.
    pub fn set_original_rules(&mut self, on: bool) {
        self.original_rules = on;
        if let Scene::Play(play) = &mut self.scene {
            if let Some(game) = play.original_mut() {
                game.set_original_rules(on);
            }
            play.sprites.original_rules = on;
        }
        if let Scene::Credits(credits) = &mut self.scene {
            credits.set_original_rules(on);
        }
    }

    /// Gives the validation mode the ROM bytes it reads (`Gameplay::set_validation_input`); for
    /// the gameplay under way and that opened afterwards.
    pub fn set_validation_input(&mut self, input: Option<crate::game::gameplay::ValidationInput>) {
        if let Scene::Play(play) = &mut self.scene
            && let Some(game) = play.original_mut()
        {
            game.set_validation_input(input.clone());
        }
        self.validation = input;
    }

    /// Asks the gameplay under way for a scene change as the pod would (tests, comparison with
    /// the original).
    pub fn request_transition(&mut self, transition: Transition) {
        if let Scene::Play(play) = &mut self.scene
            && let Some(game) = play.original_mut()
        {
            game.request_scene_change(transition);
        }
    }

    pub fn set_random(&mut self, rng: Rng) {
        self.clock.rng = rng;
    }

    /// Sets the rotating job counter RAM $60 (comparison with the original, as
    /// [`Game::set_random`]).
    pub fn set_job(&mut self, job: u8) {
        self.clock.job = job & 7;
    }

    /// The frame counter (RAM $5D) that the request slots' servicing sees at the start of the
    /// next pass: its low bit picks slot A or B (sound.md 8.2). The scenes that count at the
    /// start of their pass (all but the title and gameplay) are one ahead of it, as Mesen
    /// showed (`sj-tools scene --compare`).
    pub fn sound_counter(&self) -> u32 {
        match &self.scene {
            Scene::Play(play) => play.counter(),
            Scene::Title(_) => self.clock.counter,
            _ => self.clock.counter.wrapping_add(1),
        }
    }

    /// Takes the sound requests made since the last call.
    pub fn take_sound(&mut self) -> Vec<SoundRequest> {
        std::mem::take(&mut self.sound)
    }

    /// The picture generator's screen of the current scene, when it has one (comparison with the
    /// original: tile memory and nametable).
    pub fn current_screen(&self) -> Option<&Screen> {
        match &self.scene {
            Scene::Licence { screen, .. } | Scene::LastPassword { screen, .. } => Some(screen),
            Scene::Title(title) => Some(title.screen()),
            Scene::Entry(entry) => Some(entry.screen()),
            Scene::Story(story) => Some(story.screen()),
            Scene::Flight(flight) => Some(flight.screen()),
            Scene::Launch(scene) => Some(scene.launch.screen()),
            Scene::Approach(approach) => Some(approach.screen()),
            Scene::Surface(scene) => Some(scene.surface.screen()),
            Scene::Shop(shop) => Some(shop.screen()),
            Scene::Map(scene) => Some(scene.map.screen()),
            Scene::Message(scene) => Some(scene.message.screen()),
            Scene::Interlude(scene) => Some(scene.interlude.screen()),
            Scene::Badge(scene) => Some(scene.badge.screen()),
            Scene::Ending(ending) => Some(ending.screen()),
            Scene::Credits(credits) => Some(credits.screen()),
            _ => None,
        }
    }

    /// The scene's name, for the window title.
    pub fn scene_name(&self) -> &'static str {
        match &self.scene {
            Scene::Licence { .. } => "licence",
            Scene::Title(_) => "title",
            Scene::Entry(_) => "password",
            Scene::Story(_) => "story",
            Scene::Flight(_) => "flight",
            Scene::Launch(_) => "launch",
            Scene::Approach(_) => "approach",
            Scene::Surface(_) => "surface",
            Scene::Shop(shop) if shop.mode() == Mode::Equip => "equip list",
            Scene::Shop(_) => "shop",
            Scene::Map(_) => "map",
            Scene::Message(_) => "message",
            Scene::Interlude(_) => "bonus flight",
            Scene::Badge(_) => "badge",
            Scene::Ending(_) => "ending",
            Scene::Credits(_) => "credits",
            Scene::Play(_) => "gameplay",
            Scene::GameOver(_) => "game over",
            Scene::LastPassword { .. } => "last password",
        }
    }

    /// The level being played, if the game is in gameplay.
    pub fn level(&self) -> Option<usize> {
        match &self.scene {
            Scene::Play(play) => Some(play.level()),
            _ => None,
        }
    }

    /// The gameplay under way, if any.
    pub fn gameplay(&self) -> Option<&Gameplay> {
        match &self.scene {
            Scene::Play(play) => play.original(),
            _ => None,
        }
    }

    /// The state a password would hold now (the last start of a level).
    pub fn saved(&self) -> &Saved {
        &self.saved
    }

    /// Chooses the engine for the levels started from now on (`docs/scaled.md`): the
    /// Scaled engine when `on`, else the Original. A level under way keeps its engine.
    pub fn set_scaled(&mut self, on: bool) {
        self.scaled = on;
    }

    /// Lets the pod come to rest on flat ground in the Scaled engine (`docs/scaled.md`,
    /// "Pod rest"); off, it hops as in the Original.
    pub fn set_pod_rest(&mut self, on: bool) {
        self.pod_rest = on;
        self.sync_cheats();
    }

    /// Lets any button end the licence screen, the ending picture and the credits, on to the
    /// title (an addition); off, they run their full length.
    pub fn set_skip_scenes(&mut self, on: bool) {
        self.skip_scenes = on;
    }

    /// Whether gameplay in the Scaled engine is under way: the program then calls
    /// [`Game::tick`] at its tick rate instead of [`Game::pass`].
    pub fn scaled_play(&self) -> bool {
        matches!(&self.scene, Scene::Play(play) if matches!(&play.engine, Engine::Scaled(game) if !game.paused()))
    }

    /// The Scaled engine's state under way, if any.
    pub fn scaled(&self) -> Option<&Scaled> {
        match &self.scene {
            Scene::Play(play) => match &play.engine {
                Engine::Scaled(scaled) => Some(scaled),
                Engine::Original(_) => None,
            },
            _ => None,
        }
    }

    /// One tick of the Scaled engine at `rate` Hz; outside Scaled gameplay, nothing. `seen`
    /// is the controller with anything pressed since the last tick, `held` as it is now.
    pub fn tick(
        &mut self,
        assets: &Assets,
        seen: Buttons,
        held: Buttons,
        rate: u32,
    ) -> Result<(), MissingGraphics> {
        self.held = held;
        self.sync_cheats();
        let Scene::Play(play) = &mut self.scene else {
            return Ok(());
        };
        let (seen, held) = match self.demo.as_mut() {
            Some(demo) => {
                let buttons = demo.input(&assets.world, &play.engine, seen, held);
                (buttons, buttons)
            }
            None => (seen, held),
        };
        let Engine::Scaled(scaled) = &mut play.engine else {
            return Ok(());
        };
        scaled.tick(&assets.world, seen, held, rate);
        let (to_message, resumed) = self.after_play(assets)?;
        self.act_on_play(assets, to_message, resumed)
    }

    /// What gameplay asked for after its frame or tick: its sounds, a pause, the demo's end,
    /// or a scene change (with the message screen it opens, or none). Returns the message to
    /// open and whether to resume at once.
    fn after_play(&mut self, assets: &Assets) -> Result<(Option<Subject>, bool), MissingGraphics> {
        let Scene::Play(play) = &mut self.scene else {
            return Ok((None, false));
        };
        self.sound.extend(
            either_mut!(play, game => game.take_sound())
                .into_iter()
                .map(SoundRequest::Request),
        );
        if either!(play, game => game.paused()) {
            self.sound.push(SoundRequest::Pause);
        }
        if self.demo.as_ref().is_some_and(|demo| demo.over(play)) {
            self.demo = None;
            self.open_title(assets)?;
        } else if let Some(transition) = either!(play, game => game.transition()) {
            self.clock.rng = either!(play, game => game.rng);
            let flags = either!(play, game => game.planet_flags);
            return Ok(match message_for(transition, play.level(), flags) {
                Some(subject) => (Some(subject), false),
                None => (None, true),
            });
        }
        Ok((None, false))
    }

    /// One frame of the NES clock with the controller as `buttons`.
    pub fn pass(&mut self, assets: &Assets, buttons: Buttons) -> Result<(), MissingGraphics> {
        self.sync_cheats();
        let result = self.pass_scene(assets, buttons);
        self.sync_cheats();
        result
    }

    /// Chooses the cheats (all off by default) for the gameplay under way and that opened
    /// afterwards; the attract demos never use them.
    pub fn set_cheats(&mut self, cheats: Cheats) {
        self.cheats = cheats;
        self.sync_cheats();
    }

    /// Chooses the view's width for gameplay (`docs/widescreen.md`): 256, the original's, by
    /// default. The gameplay under way follows it at once.
    pub fn set_view_width(&mut self, width: u16) {
        self.view_width = crate::game::view::clamp_width(width);
        self.sync_cheats();
    }

    fn sync_cheats(&mut self) {
        if let Scene::Play(play) = &mut self.scene {
            either_mut!(play, game => game.set_view_width(self.view_width));
            match &mut play.engine {
                Engine::Original(game) => {
                    let cheats = if game.is_demo() || self.demo.is_some() {
                        Cheats::default()
                    } else {
                        self.cheats
                    };
                    game.set_cheats(cheats);
                }
                Engine::Scaled(game) => {
                    let cheats = if self.demo.is_some() {
                        Cheats::default()
                    } else {
                        self.cheats
                    };
                    game.set_cheats(cheats);
                    game.set_pod_rest(
                        self.pod_rest
                            .then_some(crate::game::scaled::pod::REST_BELOW),
                    );
                }
            }
        }
    }

    /// The whole state as bytes: a header (magic and the two format versions), the serialised game and,
    /// if given, the sound player, so that music and effects resume where they were. Loading
    /// it gives a game that continues exactly as this one would. What the program chooses
    /// (cheats, validation input, muting) is not part of it, and neither is
    /// anything imported from the ROM: loading builds that again from the assets.
    pub fn save_state(&self, sound: Option<&Player>) -> Vec<u8> {
        let text =
            ron::ser::to_string(&SaveRef { game: self, sound }).expect("the game state serialises");
        let mut bytes = Vec::with_capacity(text.len() + 12);
        bytes.extend_from_slice(crate::game::state::MAGIC);
        bytes.extend_from_slice(&crate::game::state::FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&crate::game::state::SCALED_FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    /// Replaces the state with a saved one, rebuilding the graphics from `assets`. On an error
    /// the game is left as it was. Returns the saved sound player, if the state held one, to
    /// hand to [`Player::restore`] (so that the program can hold its audio thread off for that
    /// alone).
    pub fn load_state(
        &mut self,
        bytes: &[u8],
        assets: &Assets,
    ) -> Result<Option<Player>, StateError> {
        let rest = bytes
            .strip_prefix(crate::game::state::MAGIC.as_slice())
            .ok_or(StateError::NotASaveState)?;
        let (version, text) = rest.split_at_checked(4).ok_or(StateError::NotASaveState)?;
        let version = u32::from_le_bytes([version[0], version[1], version[2], version[3]]);
        if version != crate::game::state::FORMAT_VERSION {
            return Err(StateError::Version(version));
        }
        let (version, text) = text.split_at_checked(4).ok_or(StateError::NotASaveState)?;
        let version = u32::from_le_bytes([version[0], version[1], version[2], version[3]]);
        if version != crate::game::state::SCALED_FORMAT_VERSION {
            return Err(StateError::ScaledVersion(version));
        }
        let text = std::str::from_utf8(text).map_err(|e| StateError::Damaged(e.to_string()))?;
        let Save {
            game: mut loaded,
            sound,
        } = crate::game::state::with_assets(assets, || ron::de::from_str(text))
            .map_err(|e| StateError::Damaged(e.to_string()))?;
        loaded.validation = self.validation.take();
        loaded.cheats = self.cheats;
        loaded.scaled = self.scaled;
        loaded.pod_rest = self.pod_rest;
        loaded.skip_scenes = self.skip_scenes;
        loaded.view_width = self.view_width;
        loaded.original_rules = self.original_rules;
        *self = loaded;
        self.set_original_rules(self.original_rules);
        self.set_validation_input(self.validation.clone());
        self.sync_cheats();
        Ok(sound)
    }

    fn pass_scene(&mut self, assets: &Assets, buttons: Buttons) -> Result<(), MissingGraphics> {
        let pressed = buttons.pressed_since(self.held);
        self.held = buttons;
        // What the scene asked for that needs the scene's gameplay itself: a message screen, or
        // acting on a scene change gameplay requested.
        let mut to_message: Option<Subject> = None;
        let mut resumed = false;
        let mut to_map = false;
        let mut to_map_leave = false;
        let mut to_badge: Option<u8> = None;
        let mut badge_done: Option<u32> = None;
        match &mut self.scene {
            Scene::Licence { frames, .. } => {
                *frames += 1;
                if *frames >= LICENCE_FRAMES || (self.skip_scenes && pressed.0 != 0) {
                    self.open_title(assets)?;
                }
            }
            Scene::Title(title) => {
                match title.step(&assets.graphics, &mut self.clock, buttons, pressed)? {
                    TitleOutcome::Running => {}
                    TitleOutcome::NewGame => {
                        self.new_game();
                        self.start_level(assets)?;
                    }
                    TitleOutcome::Continue => {
                        let entry = Entry::new(&assets.graphics, Some(title.screen()))?;
                        self.new_game();
                        self.scene = Scene::Entry(Box::new(entry));
                    }
                    TitleOutcome::Timeout => {
                        // The attract cycle (game-flow.md 7.1): the count goes up, then an odd
                        // one gives the story and an even one the demos (a stub still).
                        self.attract_timeouts += 1;
                        if self.attract_timeouts % 2 == 1 {
                            self.open_story(assets)?;
                        } else {
                            let index = usize::from(self.attract_timeouts / 2 - 1);
                            if self.attract_timeouts >= 6 {
                                self.attract_timeouts = 0;
                            }
                            self.open_demo(assets, index)?;
                        }
                    }
                }
                if let Scene::Title(title) = &mut self.scene
                    && title.take_engine_sound()
                {
                    // The menu Jetman's engine, requested as the pod's is (player.md 4.8).
                    self.sound
                        .push(SoundRequest::Request(crate::game::sound::Request::b(
                            crate::game::sound::Channel::Noise,
                            0x10,
                        )));
                }
            }
            Scene::Entry(entry) => match entry.step(buttons, pressed) {
                EntryOutcome::Running => {}
                EntryOutcome::NewGame => {
                    self.new_game();
                    self.start_level(assets)?;
                }
                EntryOutcome::Valid(saved) => {
                    self.saved = saved;
                    self.carry = self.seed_from(&saved);
                    self.start_level(assets)?;
                }
            },
            Scene::Story(story) => {
                if story.step(&mut self.clock, pressed) == StoryOutcome::Ended {
                    self.open_title(assets)?;
                }
            }
            Scene::Flight(flight) => {
                match flight.step(&mut self.clock, buttons, pressed) {
                    FlightOutcome::Running => {}
                    // The shop on odd planets (game-flow.md 7.3), else the approach screen.
                    FlightOutcome::Ended => {
                        let level = usize::from(self.saved.level);
                        if level % 2 == 1 && level <= 11 {
                            let previous = flight.screen().clone();
                            self.open_shop(assets, Mode::Shop, Some(&previous))?;
                        } else {
                            self.open_approach(assets)?;
                        }
                    }
                }
            }
            Scene::Launch(scene) => {
                if scene.launch.step(&assets.layouts, &mut self.clock, pressed)
                    == LaunchOutcome::Ended
                {
                    let level = scene.level;
                    let mut seed = scene.seed.clone();
                    seed.rng = self.clock.rng;
                    let pod_type = seed.equipment & 3;
                    self.begin_play(assets, level, pod_type, *seed, false)?;
                }
            }
            Scene::Approach(approach) => {
                if approach.step(&mut self.clock, buttons, pressed) == ApproachOutcome::Land {
                    let previous = approach.screen().clone();
                    self.open_landing(assets, Some(&previous))?;
                }
            }
            Scene::Surface(scene) => {
                let level = scene.level;
                match scene
                    .surface
                    .step(&assets.layouts, &mut self.clock, pressed)
                {
                    SurfaceOutcome::Running => {}
                    SurfaceOutcome::Launch => self.open_launch(assets)?,
                    SurfaceOutcome::EquipPod => {
                        let previous = scene.surface.screen().clone();
                        self.open_shop(assets, Mode::Equip, Some(&previous))?;
                    }
                    SurfaceOutcome::Left => {
                        // The next planet; planet 12 leads to 11 (game-flow.md 6).
                        let next = if level >= 12 { 11 } else { level + 1 };
                        self.saved.level = next as u8;
                        self.start_level(assets)?;
                    }
                }
                if let Scene::Surface(scene) = &mut self.scene {
                    self.sound.extend(scene.surface.take_song());
                }
            }
            Scene::Shop(shop) => {
                let mode = shop.mode();
                let outcome = shop.step(
                    &mut self.clock,
                    pressed,
                    &mut self.inventory,
                    &mut self.carry.score,
                );
                for id in shop.take_sounds() {
                    self.sound.extend(SoundRequest::both(id));
                }
                if outcome == ShopOutcome::Leave {
                    self.inventory.into_seed(&mut self.carry);
                    if mode == Mode::Shop {
                        self.open_approach(assets)?;
                    } else {
                        self.open_launch(assets)?;
                    }
                }
            }
            Scene::Message(scene) => {
                if scene.message.step(&mut self.clock, buttons, pressed) == MessageOutcome::Ended {
                    resumed = true;
                }
                if let Some(id) = scene.message.take_sound() {
                    self.sound.extend(SoundRequest::both(id));
                }
                if scene.message.take_click() {
                    self.sound
                        .push(SoundRequest::Request(crate::game::sound::Request::Direct {
                            channel: crate::game::sound::Channel::Noise,
                            id: 0x3A,
                        }));
                }
            }
            Scene::Interlude(scene) => {
                if let InterludeOutcome::Ended { gems } =
                    scene
                        .interlude
                        .step(&assets.layouts, &mut self.clock, buttons, pressed)
                {
                    to_badge = Some(gems);
                }
                for _ in 0..scene.interlude.take_gem_sounds() {
                    self.sound.extend(SoundRequest::both(interlude::GEM_SOUND));
                }
            }
            Scene::Badge(scene) => {
                if let BadgeOutcome::Ended { gain } =
                    scene.badge.step(&mut self.clock, buttons, pressed)
                {
                    badge_done = Some(gain);
                }
                if let Some(id) = scene.badge.take_sound() {
                    self.sound.extend(SoundRequest::both(id));
                }
            }
            Scene::Ending(_) if self.skip_scenes && pressed.0 != 0 => {
                self.warm_reset(assets)?;
            }
            Scene::Ending(ending) => {
                if ending.step(&mut self.clock) == EndingOutcome::Ended {
                    let mut credits = Credits::new(&assets.graphics)?;
                    credits.set_original_rules(self.original_rules);
                    self.scene = Scene::Credits(Box::new(credits));
                }
            }
            Scene::Credits(credits) => {
                let skipped = self.skip_scenes && pressed.0 != 0;
                if skipped || credits.step() == CreditsOutcome::Ended {
                    self.warm_reset(assets)?;
                }
            }
            Scene::Play(play) if either!(play, game => game.paused()) => {
                // Select opens the planet map on a planet with a mapping device (game-flow.md 4).
                to_map = pressed.has(Buttons::SELECT)
                    && play.level() < 13
                    && either!(play, game => game.mapping_device || game.super_mapping_device);
                if to_map {
                    // The pause handler's own pass (the counters) runs first.
                    either_mut!(play, game => game.pause_frame(buttons));
                } else if either_mut!(play, game => game.pause_frame(buttons)) {
                    self.sound.push(SoundRequest::Resume);
                }
            }
            Scene::Map(scene) => {
                if scene.map.step(&mut self.clock, pressed) == MapOutcome::Leave {
                    to_map_leave = true;
                }
            }
            Scene::Play(play) => {
                let buttons = match self.demo.as_mut() {
                    Some(demo) => demo.input(&assets.world, &play.engine, buttons, buttons),
                    None => buttons,
                };
                match &mut play.engine {
                    Engine::Original(game) => game.pass(&assets.world, buttons),
                    // A frame of the NES's clock is a tick at 60 Hz.
                    Engine::Scaled(game) => game.tick(&assets.world, buttons, buttons, 60),
                }
                (to_message, resumed) = self.after_play(assets)?;
            }
            Scene::GameOver(scene) => {
                // No button is read for 240 frames (scenes.md 12).
                scene.counter += 1;
                if scene.counter > gameover::LAST_C {
                    let (hud, palette, level) = (scene.hud, scene.palette, scene.level);
                    self.last_password(assets, hud, palette, level)?;
                }
            }
            Scene::LastPassword {
                screen,
                pressed: done,
            } => {
                // The press hides the status panel; the screen is shown for one more pass,
                // then the program restarts (scenes.md 12).
                if *done {
                    self.warm_reset(assets)?;
                } else if pressed.0 != 0 {
                    *done = true;
                    screen.panel = None;
                }
            }
        }
        if to_map {
            if let Scene::Play(play) = self.take_scene() {
                self.open_map(assets, play)?;
            }
            return Ok(());
        }
        if to_map_leave {
            if let Scene::Map(scene) = self.take_scene() {
                let MapScene { mut play, .. } = *scene;
                either_mut!(play, game => game.leave_map(&assets.world, buttons));
                let level = play.level();
                self.sound
                    .push(SoundRequest::Request(crate::game::sound::Request::Song {
                        id: planet_song(level),
                        always: true,
                    }));
                self.scene = Scene::Play(play);
            }
            return Ok(());
        }
        if let Some(gems) = to_badge {
            if let Scene::Interlude(scene) = self.take_scene() {
                let InterludeScene { play, .. } = *scene;
                self.open_badge(assets, play, gems)?;
            }
            return Ok(());
        }
        if let Some(gain) = badge_done {
            if let Scene::Badge(scene) = self.take_scene() {
                let BadgeScene { mut play, .. } = *scene;
                if play.level() >= crate::game::level::FIRST_WARP_LEVEL {
                    // After a warp level: back to its planet (code 3, game-flow.md 7.4).
                    let rng = self.clock.rng;
                    let next = either_mut!(play, game => {
                        game.rng = rng;
                        Engine::from(game.after_badge_counting(&assets.world, gain))
                    });
                    self.enter_level(assets, next)?;
                    return Ok(());
                }
                either_mut!(play, game => game.resume_after_bonus(&assets.world, gain));
                // The level song starts again as play resumes (scenes.md 15).
                self.sound
                    .push(SoundRequest::SongIfNew(planet_song(play.level())));
                self.scene = Scene::Play(play);
            }
            return Ok(());
        }
        self.act_on_play(assets, to_message, resumed)
    }

    /// Opens the message screen gameplay asked for, or acts on its scene change at once.
    fn act_on_play(
        &mut self,
        assets: &Assets,
        to_message: Option<Subject>,
        resumed: bool,
    ) -> Result<(), MissingGraphics> {
        if let Some(subject) = to_message
            && let Scene::Play(play) = self.take_scene()
        {
            if subject.kind == 7 {
                // The final level's end routine asks for the congratulations song (scenes.md 15).
                self.sound.push(SoundRequest::SongIfNew(32));
            }
            self.open_message(assets, play, subject)?;
        } else if resumed {
            match self.take_scene() {
                Scene::Play(mut play) => {
                    let outcome = either_mut!(play, game => game.resume(&assets.world));
                    self.after_resume(assets, play, outcome)?;
                }
                Scene::Message(scene) => {
                    let MessageScene { mut play, .. } = *scene;
                    let outcome = either_mut!(play, game => game.resume(&assets.world));
                    self.after_resume(assets, play, outcome)?;
                }
                other => self.scene = other,
            }
        }
        Ok(())
    }

    /// Takes the current scene out, leaving a placeholder.
    fn take_scene(&mut self) -> Scene {
        std::mem::replace(
            &mut self.scene,
            Scene::Licence {
                screen: Box::default(),
                frames: 0,
            },
        )
    }

    /// Goes on after gameplay's scene change was acted on (game-flow.md 7.4): back into the level,
    /// the surface menu after a docking, game over, or one of the scene changes that are still
    /// stubs (the level starts again with what the player carries).
    fn after_resume(
        &mut self,
        assets: &Assets,
        mut play: Box<Play>,
        outcome: Outcome,
    ) -> Result<(), MissingGraphics> {
        let level = play.level();
        let mut seed = either!(play, game => game.seed());
        seed.rng = self.clock.rng;
        self.sound.extend(
            either_mut!(play, game => game.take_sound())
                .into_iter()
                .map(SoundRequest::Request),
        );
        match outcome {
            Outcome::Playing => {
                self.scene = Scene::Play(play);
                Ok(())
            }
            Outcome::GameOver => self.game_over(assets, play),
            Outcome::Docked => {
                self.inventory.take_from_seed(&seed);
                let collectables = either!(play, game => game.collectables_left());
                self.carry = seed;
                self.open_docked(assets, collectables)
            }
            Outcome::Elsewhere(Transition::Ending) => {
                let ending = Ending::new(&assets.graphics)?;
                self.scene = Scene::Ending(Box::new(ending));
                Ok(())
            }
            Outcome::Elsewhere(Transition::Interlude) => self.open_interlude(assets, play),
            Outcome::Elsewhere(Transition::FinalLevel) => {
                // No launch scene, no mothership: the level starts at once.
                let rng = self.clock.rng;
                let next = either_mut!(play, game => {
                    game.rng = rng;
                    Engine::from(game.enter_final_level(&assets.world))
                });
                self.enter_level(assets, next)
            }
            Outcome::Elsewhere(Transition::ChangeLevel) => {
                let rng = self.clock.rng;
                let next = either_mut!(play, game => {
                    game.rng = rng;
                    Engine::from(game.change_level(&assets.world))
                });
                self.enter_level(assets, next)
            }
            Outcome::Elsewhere(Transition::WarpEnd) => {
                // The countdown of a warp level ran out: the badge counts the crystals.
                let gems = either!(play, game => game.warp_crystals);
                self.open_badge(assets, play, gems)
            }
            Outcome::Elsewhere(_) => {
                let pod_type = seed.equipment & 3;
                self.begin_play(assets, level, pod_type, seed, false)
            }
        }
    }

    /// Draws the current scene; `blend` interpolates gameplay between its last two passes.
    pub fn draw(&self, frame: &mut Frame, assets: &Assets, blend: f32) {
        match &self.scene {
            Scene::Licence { screen, .. } | Scene::LastPassword { screen, .. } => {
                screen.draw(frame)
            }
            Scene::GameOver(scene) => {
                let play = &scene.play;
                play.draw(frame, assets, 1.0);
                if scene.counter >= gameover::LETTERS_FROM {
                    let letters = scene.chr.pattern_table(0);
                    let loaded = gameover::tiles_loaded(scene.counter);
                    for (x, y, tile) in gameover::sprites(scene.counter) {
                        if usize::from(tile) >= loaded {
                            continue;
                        }
                        draw_tile(
                            frame,
                            &play.graphics.palette,
                            &letters,
                            tile,
                            gameover::PALETTE,
                            (x + frame.centre() as i32, y + 1),
                        );
                    }
                }
            }
            Scene::Title(title) => title.screen().draw(frame),
            Scene::Entry(entry) => entry.screen().draw(frame),
            Scene::Story(story) => story.screen().draw(frame),
            Scene::Flight(flight) => flight.screen().draw(frame),
            Scene::Launch(scene) => scene.launch.screen().draw(frame),
            Scene::Approach(approach) => approach.screen().draw(frame),
            Scene::Surface(scene) => scene.surface.screen().draw(frame),
            Scene::Shop(shop) => shop.screen().draw(frame),
            Scene::Map(scene) => scene.map.screen().draw(frame),
            Scene::Interlude(scene) => scene.interlude.screen().draw(frame),
            Scene::Badge(scene) => scene.badge.screen().draw(frame),
            Scene::Ending(ending) => ending.screen().draw(frame),
            Scene::Credits(credits) => credits.screen().draw(frame),
            Scene::Message(scene) => {
                scene.message.screen().draw(frame);
                let palette = &scene.message.screen().palette;
                // The first sprite is in front: draw from the last.
                for (x, y, tile, attributes) in scene.message.item_sprites().into_iter().rev() {
                    draw_tile(
                        frame,
                        palette,
                        scene
                            .message
                            .sprite_tiles()
                            .unwrap_or_else(|| scene.play.sprites.pattern_table()),
                        tile,
                        attributes,
                        (x + frame.centre() as i32, y),
                    );
                }
            }
            Scene::Play(play) => play.draw(frame, assets, blend),
        }
    }
}

/// The bit of a planet's flag in `Seed::planet_flags` (game-flow.md 6): planet 12 uses planet 10's,
/// and a second or third level uses its planet's.
pub fn flag_bit(level: usize) -> usize {
    crate::game::gameplay::gate::planet_flag_bit(level)
}

/// The song of the approach screen (scenes.md 15): 7 + 2m, with m the level and 8 from 12 on.
pub fn approach_song(level: usize) -> u8 {
    let m = if level >= 12 { 8 } else { level };
    (7 + 2 * m) as u8
}

/// The message screen a scene change asks for, if it has one (game-flow.md 7.4): a delivery, the
/// warp-level introduction at a wormhole of a second or third level on a planet whose flag is
/// set, or the end of the final level.
fn message_for(transition: Transition, level: usize, planet_flags: u16) -> Option<Subject> {
    match transition {
        Transition::Delivery(d) => Some(Subject {
            kind: d.message_kind,
            text: d.text,
            item: d.kind,
            cargo: d.cargo,
        }),
        Transition::ChangeLevel => {
            let flagged = planet_flags & (1 << flag_bit(level)) != 0;
            (level >= 13 && flagged).then_some(Subject {
                kind: 6,
                text: 0,
                item: 0,
                cargo: 0,
            })
        }
        Transition::Ending => Some(Subject {
            kind: 7,
            text: 23,
            item: 0,
            cargo: 0,
        }),
        _ => None,
    }
}

/// The planet song of a level (sound.md 8.1): 2m + 8, with m from the level; the final level
/// and the warp levels have their own.
pub fn planet_song(level: usize) -> u8 {
    match level {
        26 => 33,
        29.. => 5,
        _ => {
            let m = match level {
                0..=12 => level,
                13..=25 => level - 13,
                _ => 1,
            };
            let m = if m >= 12 { 8 } else { m };
            (2 * m + 8) as u8
        }
    }
}

#[cfg(test)]
mod tests;
