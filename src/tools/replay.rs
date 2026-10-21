//! State replays: a recorded input sequence is played in the original
//! (`tools/mesen/replay-capture.lua`) and in `src/game`, and the pod's state is compared after
//! every logic step.

use crate::game::gameplay::economy::{Outcome::Elsewhere, Transition};
use crate::game::gameplay::input::Buttons;
use crate::game::gameplay::objects::Score;
use crate::game::gameplay::rng::Rng;
use crate::game::gameplay::sprites::{SpriteFile, Sprites};
use crate::game::gameplay::{Gameplay, PodLeftovers, Seed, ValidationInput};
use crate::game::graphics::{Graphics, LevelGraphics};
use crate::game::level::World;
use crate::game::playfield::Frame;
use crate::game::sound::{Channel, Request, Sound, SoundData};
use crate::tools::screen::{self, MesenPalette, ScreenDiff};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A field that differs: its name, the original's value and ours.
pub type FieldDiff = (String, u32, u32);

/// An object set into the table at a step: slot, state, x, y, frame, list index.
pub type ObjectSetup = (usize, u8, u16, u16, u8, u8);

/// A recorded input sequence: the buttons held in each logic step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Replay {
    pub steps: Vec<Buttons>,
    /// RAM $DC at the start of play (the pod's equipment).
    pub equipment: u8,
    /// RAM $D1 at the start of play: the fitted special weapon (a test setup; economy.md 6).
    pub weapon: u8,
    /// RAM $D3 (the fitted pod device) and RAM $050E (the bullet upgrade) at the start, test
    /// setups like `weapon`.
    pub device: u8,
    pub bullet_upgrade: u8,
    /// The weapon gauge RAM $C9:$C8 at the start (a test setup); `None` leaves the launch's full one.
    pub weapon_gauge: Option<u16>,
    /// Jetman's lives RAM $3E at the start (a test setup); `None` leaves the new game's.
    pub lives: Option<u8>,
    /// Fuel set before a step (1-based), a test setup that empties the tank.
    pub fuel_pokes: BTreeMap<usize, u16>,
    /// The Nth attract demo of a power-on run (1-3) instead of input: the autopilot flies.
    pub demo: Option<usize>,
    /// The level played: a planet (0-12), a second level (13-25), the final level (26) or a
    /// warp level (29-41).
    pub level: usize,
    /// Test setups before a step (1-based): the pod at rest at (x, y), the camera at (cx, cy).
    pub warps: BTreeMap<usize, [u16; 4]>,
    /// Items list entries written before a step (1-based): index, kind, x, y.
    pub items: BTreeMap<usize, Vec<(usize, u8, u16, u16)>>,
    /// The tractor beam's swing count RAM $6E set before a step.
    pub spins: BTreeMap<usize, u8>,
    /// From this step on the fuel is set to $E000 before every step.
    pub keep_fuel: Option<usize>,
    /// The pod's fields are taken from the original after every step (the final level's
    /// push along the route is an open question of the specs).
    pub pod_sync: bool,
    /// Object slots written before a step: slot, state, x, y, frame, list index.
    pub objs: BTreeMap<usize, Vec<ObjectSetup>>,
    /// Planets whose warpship piece was delivered (a test setup), one bit each.
    pub flags: u16,
}

/// Parses the replay format: lines of "COUNT TOKEN..." (tokens A B SEL START U D L R, or `-`),
/// "dc HEX", "weapon HEX" (RAM $D1 at the start), "device HEX" (RAM $D3), "bullet HEX" (RAM $050E),
/// "gauge HEX" (the weapon gauge RAM $C9:$C8, four digits), "fuel HEX" (sets the fuel before the next step), "warp X Y CX CY" (the pod at rest
/// at X, Y and the camera at CX, CY before the next step), "level P" (the planet, default 0)
/// "flag P" (the warpship piece of planet P is delivered, at the start), "spin HEX" (sets the beam's swing count RAM $6E before the next step), "item N KIND X Y" (writes entry N of the items list before the next step: KIND in hex) and
/// `#` comments.
pub fn parse_replay(text: &str) -> Result<Replay, String> {
    let mut replay = Replay::default();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default();
        let mut tokens = line.split_whitespace();
        let Some(first) = tokens.next() else { continue };
        let fail = |what: &str| format!("line {}: {what}", number + 1);
        if first == "demo" {
            let value = tokens.next().ok_or_else(|| fail("demo needs a number"))?;
            replay.demo = Some(
                value
                    .parse()
                    .ok()
                    .filter(|n| (1..=3).contains(n))
                    .ok_or_else(|| fail("demo is 1, 2 or 3"))?,
            );
            continue;
        }
        if first == "level" {
            let value = tokens.next().ok_or_else(|| fail("level needs a number"))?;
            replay.level = value
                .parse()
                .ok()
                .filter(|n| *n <= 26 || (29..=41).contains(n))
                .ok_or_else(|| fail("level is 0-26 or 29-41"))?;
            continue;
        }
        if first == "dc" {
            let value = tokens.next().ok_or_else(|| fail("dc needs a value"))?;
            replay.equipment =
                u8::from_str_radix(value, 16).map_err(|_| fail("dc is not hexadecimal"))?;
            continue;
        }
        if first == "weapon" {
            let value = tokens.next().ok_or_else(|| fail("weapon needs a value"))?;
            replay.weapon =
                u8::from_str_radix(value, 16).map_err(|_| fail("weapon is not hexadecimal"))?;
            continue;
        }
        if first == "device" || first == "bullet" {
            let value = tokens
                .next()
                .ok_or_else(|| fail("device and bullet need a value"))?;
            let value = u8::from_str_radix(value, 16).map_err(|_| fail("not hexadecimal"))?;
            if first == "device" {
                replay.device = value;
            } else {
                replay.bullet_upgrade = value;
            }
            continue;
        }
        if first == "lives" {
            let value = tokens.next().ok_or_else(|| fail("lives needs a value"))?;
            replay.lives =
                Some(u8::from_str_radix(value, 16).map_err(|_| fail("lives is not hexadecimal"))?);
            continue;
        }
        if first == "gauge" {
            let value = tokens.next().ok_or_else(|| fail("gauge needs a value"))?;
            replay.weapon_gauge =
                Some(u16::from_str_radix(value, 16).map_err(|_| fail("gauge is not hexadecimal"))?);
            continue;
        }
        if first == "warp" {
            let mut values = [0u16; 4];
            for value in &mut values {
                *value = tokens
                    .next()
                    .and_then(|t| t.parse().ok())
                    .ok_or_else(|| fail("warp needs X Y CX CY"))?;
            }
            replay.warps.insert(replay.steps.len() + 1, values);
            continue;
        }
        if first == "obj" {
            let mut next = || {
                tokens
                    .next()
                    .ok_or_else(|| fail("obj needs SLOT STATE X Y FRAME INDEX"))
            };
            let slot = next()?.parse().map_err(|_| fail("obj slot"))?;
            let state = u8::from_str_radix(next()?, 16).map_err(|_| fail("obj state is hex"))?;
            let x = next()?.parse().map_err(|_| fail("obj x"))?;
            let y = next()?.parse().map_err(|_| fail("obj y"))?;
            let frame = u8::from_str_radix(next()?, 16).map_err(|_| fail("obj frame is hex"))?;
            let index = u8::from_str_radix(next()?, 16).map_err(|_| fail("obj index is hex"))?;
            replay
                .objs
                .entry(replay.steps.len() + 1)
                .or_default()
                .push((slot, state, x, y, frame, index));
            continue;
        }
        if first == "flag" {
            let value = tokens.next().ok_or_else(|| fail("flag needs a planet"))?;
            let planet: u16 = value
                .parse()
                .ok()
                .filter(|p| *p < 12)
                .ok_or_else(|| fail("flag is a planet 0-11"))?;
            replay.flags |= 1 << planet;
            continue;
        }
        if first == "podsync" {
            replay.pod_sync = true;
            continue;
        }
        if first == "keepfuel" {
            replay.keep_fuel = Some(replay.steps.len() + 1);
            continue;
        }
        if first == "spin" {
            let value = tokens.next().ok_or_else(|| fail("spin needs a value"))?;
            let value =
                u8::from_str_radix(value, 16).map_err(|_| fail("spin is not hexadecimal"))?;
            replay.spins.insert(replay.steps.len() + 1, value);
            continue;
        }
        if first == "item" {
            let mut next = || tokens.next().ok_or_else(|| fail("item needs N KIND X Y"));
            let index = next()?.parse().map_err(|_| fail("item index"))?;
            let kind = u8::from_str_radix(next()?, 16).map_err(|_| fail("item kind is hex"))?;
            let x = next()?.parse().map_err(|_| fail("item x"))?;
            let y = next()?.parse().map_err(|_| fail("item y"))?;
            replay
                .items
                .entry(replay.steps.len() + 1)
                .or_default()
                .push((index, kind, x, y));
            continue;
        }
        if first == "fuel" {
            let value = tokens.next().ok_or_else(|| fail("fuel needs a value"))?;
            let value =
                u16::from_str_radix(value, 16).map_err(|_| fail("fuel is not hexadecimal"))?;
            replay.fuel_pokes.insert(replay.steps.len() + 1, value);
            continue;
        }
        let count: usize = first.parse().map_err(|_| fail("expected a count"))?;
        let mut held = 0u8;
        for token in tokens {
            held |= match token {
                "A" => Buttons::A,
                "B" => Buttons::B,
                "SEL" => Buttons::SELECT,
                "START" => Buttons::START,
                "U" => Buttons::UP,
                "D" => Buttons::DOWN,
                "L" => Buttons::LEFT,
                "R" => Buttons::RIGHT,
                "-" => 0,
                other => return Err(fail(&format!("unknown button `{other}`"))),
            };
        }
        replay
            .steps
            .extend(std::iter::repeat_n(Buttons(held), count));
    }
    Ok(replay)
}

/// Named values of one moment.
pub type Fields = BTreeMap<String, u32>;

/// One logic step of the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub step: usize,
    /// The step's A pass was cut short (game-flow.md 1.4).
    pub cut: bool,
    pub fields: Fields,
    /// The sound requests of the step, as `replay-capture.lua` logs them: "e P.C.ID" for an
    /// effect that started (pass 0 or 1, channel 0-3, id) and "m P.K.SONG" for a song request.
    pub sound: Vec<String>,
}

/// The original's log.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Capture {
    pub init: Fields,
    pub samples: Vec<Sample>,
    /// Scene changes that came back to play: the number of steps consumed and the state at the
    /// first pass after.
    pub reloads: BTreeMap<usize, Fields>,
    /// Gameplay was left after this many steps ("away") and did not come back.
    pub left_at: Option<usize>,
    /// Sound requests made outside gameplay's passes, by the step that left gameplay.
    pub scene_sound: BTreeMap<usize, Vec<String>>,
}

fn parse_fields<'a>(tokens: impl Iterator<Item = &'a str>) -> Result<Fields, String> {
    let mut fields = Fields::new();
    for token in tokens {
        let (name, value) = token
            .split_once('=')
            .ok_or_else(|| format!("`{token}` is not name=hex"))?;
        let value = u32::from_str_radix(value, 16).map_err(|_| format!("`{token}`: not hex"))?;
        fields.insert(name.to_owned(), value);
    }
    Ok(fields)
}

/// Parses the log of `replay-capture.lua`.
pub fn parse_capture(text: &str) -> Result<Capture, String> {
    let mut capture = Capture::default();
    let mut away = None;
    let mut sound = Vec::new();
    let mut early: Vec<String> = Vec::new();
    let mut left_for: Option<usize> = None;
    for line in text.lines() {
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some(kind @ ("e" | "m")) => {
                if let Some(token) = tokens.next() {
                    if token.starts_with("x.") {
                        // Started while a scene loads or shows: kept apart, under the step
                        // that left gameplay (before its "away" line, the next one).
                        let text = format!("{kind}{token}");
                        match left_for {
                            Some(step) => capture.scene_sound.entry(step).or_default().push(text),
                            None => early.push(text),
                        }
                    } else {
                        sound.push(format!("{kind}{token}"));
                    }
                }
            }
            Some("init") => {
                sound.clear();
                capture.init = parse_fields(tokens)?
            }
            Some("s") => {
                let step = tokens.next().and_then(|t| t.parse().ok());
                let cut = tokens.next().map(|t| t == "1");
                let (Some(step), Some(cut)) = (step, cut) else {
                    return Err(format!("bad step line `{line}`"));
                };
                capture.samples.push(Sample {
                    step,
                    cut,
                    fields: parse_fields(tokens)?,
                    sound: std::mem::take(&mut sound),
                });
            }
            Some("away") => {
                sound.clear();
                away = tokens.next().and_then(|t| t.parse().ok());
                left_for = away;
                if let Some(step) = away {
                    capture
                        .scene_sound
                        .entry(step)
                        .or_default()
                        .append(&mut early);
                }
            }
            Some("reload") => {
                let step = tokens.next().and_then(|t| t.parse().ok());
                let Some(step) = step else {
                    return Err(format!("bad reload line `{line}`"));
                };
                capture.reloads.insert(step, parse_fields(tokens)?);
                sound.clear();
                away = None;
                left_for = None;
            }
            _ => {}
        }
    }
    capture.left_at = away;
    if capture.init.is_empty() {
        return Err("the capture has no init line".to_owned());
    }
    Ok(capture)
}

/// How one field fared.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FieldReport {
    pub matched: usize,
    pub total: usize,
    /// First divergence: step, the original's value, ours.
    pub first: Option<(usize, u32, u32)>,
    /// For object fields, the slot and field where it first differs (e.g. `ox23`).
    pub at: String,
}

/// The object fields compared, by the letter in their names (`Gameplay::object_fields`).
pub const OBJECT_FIELDS: [(char, &str); 6] = [
    ('s', "state"),
    ('x', "x"),
    ('y', "y"),
    ('f', "frame"),
    ('h', "hit points"),
    ('i', "list index"),
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub steps: usize,
    pub compared: usize,
    pub cut: usize,
    /// Steps in which every field matched.
    pub exact: usize,
    pub fields: Vec<(&'static str, FieldReport)>,
    /// How each object field fared over the steps that were compared (a field counts as
    /// matching in a step when every slot's value does).
    pub objects: Vec<(&'static str, FieldReport)>,
    /// Steps in which every object field matched, and the number of slot values compared.
    pub objects_exact: usize,
    pub object_values: usize,
    /// Steps in which every field of the pod and the objects matched.
    pub all_exact: usize,
    /// The slots attached to the pod, 1 and 2 (`Gameplay::attached_fields`), counted apart:
    /// steps in which they matched, and the first difference (step, field, original, ours).
    pub attached_exact: usize,
    pub attached_first: Option<(usize, String, u32, u32)>,
    /// The sound requests (effects that started, song requests): the steps compared and the
    /// steps in which the list was the original's; the first difference (step, original, ours).
    pub sound_compared: usize,
    pub sound_exact: usize,
    pub sound_first: Option<(usize, Vec<String>, Vec<String>)>,
    /// Differences between the start states.
    pub init: Vec<(&'static str, u32, u32)>,
    /// The first step in which a field other than `rng` differs: the step number, then the
    /// original's and our fields before it and after it.
    pub first_divergence: Option<Divergence>,
    /// The step in which the original left gameplay for good, and the step and code of the
    /// scene change requested in ours.
    pub original_left: Option<usize>,
    pub our_transition: Option<(usize, u8)>,
    /// Scene changes that came back to play: the step, and the fields that differed from the
    /// original's state at the first pass after.
    pub reloads: Vec<(usize, Vec<FieldDiff>)>,
    /// The pictures ([`ScreenCheck`]): the steps compared, those identical outside the masked
    /// lines, and the others with how they differ.
    pub screen_compared: usize,
    pub screen_exact: usize,
    /// Steps not compared around a `warp` setup: the step before it (the capture redraws the
    /// original's background before that step's picture is taken) and the three from it; after
    /// a warp before step 1, every step up to the next warp.
    pub screen_skipped: usize,
    pub screen_steps: Vec<(usize, ScreenDiff)>,
}

/// The sprites' height in gameplay, for the lines the console's sprite limit changes.
const SPRITE_HEIGHT: usize = 8;

/// The first line of the status panel (the playfield is lines 0-192, `levels.md` 5.1).
const PANEL_TOP: usize = 193;

/// How many steps the original's status panel can lag the state: its slowest part, the score,
/// is redrawn once in 8 frames (`game-flow.md` 1.2).
const PANEL_LAG: usize = 4;

/// A step whose picture waits for the next one: the original's, its mask, ours, and ours of
/// the steps just before it that were drawn.
struct Pending {
    step: usize,
    theirs: Vec<[u8; 3]>,
    masked: Vec<bool>,
    ours: Vec<u8>,
    before: Vec<Vec<u8>>,
    /// The step's state while the beam shows, to draw it again with the next step's image of
    /// the beam's tile.
    beam: Option<Gameplay>,
}

/// The original's picture after every step (`SJ_REPLAY_SHOTS` of the capture script) and what
/// drawing ours needs.
///
/// The status panel is judged against ours of the step itself, the [`PANEL_LAG`] steps before
/// and the step after, whichever matches best: the original redraws its parts in rotating
/// video jobs and the captured frame already holds the next A pass's panel work, where the
/// remake redraws all of it every frame (`docs/deviations.md`, "Display refresh"). Likewise
/// the beam's tile may show the next step's image (`player.md` 8.4, "Timing of the image"),
/// and on a warp level the mesh tile the previous step's (the captured frame is the next
/// step's A-pass frame, whose tile was built a step earlier, `graphics.md` 5.7.2).
pub struct ScreenCheck<'a> {
    pub graphics: &'a Graphics,
    pub layouts: &'a SpriteFile,
    pub palette: MesenPalette,
    /// The capture's `step-N.png` and `step-N.txt`.
    pub dir: PathBuf,
    /// Where the side-by-side pictures of differing steps go, `step-N.png`; at most `keep`.
    pub out: Option<PathBuf>,
    pub keep: usize,
    /// Only the pictures of these steps are written, when given.
    pub only: Option<(usize, usize)>,
    /// The pod type whose tiles the original loaded, when a replay's setup changed the type
    /// after the launch (the tiles stay those of the launch).
    pub pod_tiles: Option<u8>,
    /// The panel's weapon icon is left out: a replay's setup fitted the weapon after the launch,
    /// which loads the icon (`graphics.md` 1.2).
    pub skip_icon: bool,
    /// The original's terrain list has spilled in this level (Q45): its F gauge and the
    /// objects of slots 15-30 are masked.
    pub spilled: bool,
    /// The level, pod type and drawing data last used.
    drawing: Option<(usize, u8, LevelGraphics, Sprites)>,
    /// Ours of the last steps drawn, the latest last.
    recent: Vec<(usize, Vec<u8>)>,
    /// The warp mesh's helper position (slot 23) of the last step drawn, and its step.
    helper: Option<(usize, (u16, u16))>,
    /// The level, step and camera y of the last step drawn, and the map tile rows the original's
    /// drawer has skipped since: it draws one row per step, so a camera move across two or more
    /// row boundaries in one step leaves rows stale until they leave the ring of 30 (`levels.md`,
    /// Appendix A, "Rows the game's own drawer skips").
    rows: Option<(usize, usize, u16)>,
    skipped: std::collections::BTreeSet<u32>,
    pending: Option<Pending>,
}

impl<'a> ScreenCheck<'a> {
    pub fn new(
        graphics: &'a Graphics,
        layouts: &'a SpriteFile,
        palette: MesenPalette,
        dir: PathBuf,
    ) -> ScreenCheck<'a> {
        ScreenCheck {
            graphics,
            layouts,
            palette,
            dir,
            out: None,
            keep: 0,
            only: None,
            pod_tiles: None,
            skip_icon: false,
            spilled: false,
            drawing: None,
            recent: Vec::new(),
            helper: None,
            rows: None,
            skipped: std::collections::BTreeSet::new(),
            pending: None,
        }
    }

    /// Our picture of `game`: NES colour numbers.
    fn draw(&mut self, world: &World, game: &Gameplay) -> Result<Vec<u8>, String> {
        let pod_type = self.pod_tiles.unwrap_or(game.pod.equipment.owned & 3);
        let level = game.level();
        if !matches!(&self.drawing, Some((l, p, ..)) if (*l, *p) == (level, pod_type)) {
            let graphics = LevelGraphics::new(self.graphics, &world.levels[level])
                .map_err(|error| error.to_string())?;
            let mut sprites = Sprites::new(self.graphics, self.layouts, level, pod_type)
                .map_err(|error| error.to_string())?;
            sprites.original_rules = true;
            self.drawing = Some((level, pod_type, graphics, sprites));
        }
        let Some((_, _, graphics, sprites)) = &self.drawing else {
            unreachable!()
        };
        let mut frame = Frame::default();
        game.draw(&mut frame, world, graphics, sprites, self.layouts, 1.0);
        Ok(frame.pixels)
    }

    /// Draws `game` after `step` and judges the picture of the step before it was waiting for;
    /// the verdicts come out one step late (see [`ScreenCheck::finish`]).
    /// Follows the rows the original's drawer skips (`levels.md` Appendix A) with the camera
    /// after `step`; every step goes through here, compared or not. A `warp` setup before
    /// `step` redraws the whole picture with its camera `warp_y`, from which that step's move
    /// counts.
    fn track_rows(&mut self, level: usize, step: usize, camera_y: u16, warp_y: Option<u16>) {
        if let Some(warp_y) = warp_y {
            self.skipped.clear();
            self.rows = Some((level, step - 1, warp_y));
        }
        match self.rows.replace((level, step, camera_y)) {
            Some((was, at, before)) if was == level && at + 1 == step => {
                let (old, new) = (u32::from(before), u32::from(camera_y));
                // Going down the drawer draws row (y + 232) / 8, going up row y / 8; only the
                // last boundary crossed gets its row.
                let missed = if new > old {
                    (old + 232) / 8 + 1..(new + 232) / 8
                } else {
                    new / 8 + 1..old / 8
                };
                self.skipped.extend(missed);
                let top = new / 8;
                self.skipped.retain(|&row| (top..top + 30).contains(&row));
            }
            _ => self.skipped.clear(),
        }
    }

    fn check(
        &mut self,
        world: &World,
        game: &Gameplay,
        step: usize,
        fields: &Fields,
    ) -> Result<Option<(usize, ScreenDiff)>, String> {
        let mut ours = self.draw(world, game)?;
        let helper = {
            let slot = &game.objects.slots[23];
            (slot.x, slot.y)
        };
        let earlier_helper = match self.helper.replace((step, helper)) {
            Some((at, position)) if at + 1 == step && position != helper => Some(position),
            _ => None,
        };
        let camera_y = game.camera.position.y;
        let shot = self.dir.join(format!("step-{step}.png"));
        if let Some(position) = earlier_helper
            && game.level() >= crate::game::level::FIRST_WARP_LEVEL
            && shot.is_file()
        {
            let theirs = screen::read_png(&shot)?;
            let mut earlier = game.clone();
            (earlier.objects.slots[23].x, earlier.objects.slots[23].y) = position;
            let redrawn = self.draw(world, &earlier)?;
            let misses =
                |pixels: &[u8]| screen::compare(pixels, &theirs, &self.palette, &[]).differing;
            if misses(&redrawn) < misses(&ours) {
                ours = redrawn;
            }
        }
        let verdict = match self.pending.take() {
            Some(mut pending) => {
                let next = (pending.step + 1 == step).then_some(ours.as_slice());
                if let Some(mut earlier) = pending.beam.take()
                    && next.is_some()
                {
                    earlier.tow.angle = game.tow.angle;
                    let redrawn = self.draw(world, &earlier)?;
                    let misses = |pixels: &[u8]| {
                        screen::compare(pixels, &pending.theirs, &self.palette, &pending.masked)
                            .unexplained
                    };
                    if misses(&redrawn) < misses(&pending.ours) {
                        pending.ours = redrawn;
                    }
                }
                Some(self.judge(pending, next)?)
            }
            None => None,
        };
        // The steps just before, without a gap.
        let before: Vec<Vec<u8>> = self
            .recent
            .iter()
            .rev()
            .zip(1..)
            .take_while(|((at, _), back)| at + back == step)
            .map(|((_, pixels), _)| pixels.clone())
            .collect();
        if shot.is_file() {
            let theirs = screen::read_png(&shot)?;
            let dump = std::fs::read_to_string(shot.with_extension("txt")).unwrap_or_default();
            let mut masked = screen::console_mask(&screen::sprite_memory(&dump), SPRITE_HEIGHT);
            // The playfield's first shown and last attribute columns (16 pixels): the original
            // streams the map's next column into them with seams and wrong attributes while
            // scrolling (`docs/deviations.md`, "Scrolling"; the left 8 pixels are blanked).
            let (rows, _) = masked.as_chunks_mut::<{ crate::game::SCREEN_WIDTH }>();
            for row in rows.iter_mut().take(PANEL_TOP) {
                row[8..24].fill(true);
                row[crate::game::SCREEN_WIDTH - 16..].fill(true);
            }
            // Past the map's right edge, where only a `warp` setup puts the camera (5.2), the
            // original draws whatever bytes follow each map row; the remake draws sky.
            let edge = u32::from(world.levels[game.level()].width) * 128;
            let inside = edge.saturating_sub(u32::from(game.camera.position.x)) as usize;
            if inside < crate::game::SCREEN_WIDTH {
                for row in rows.iter_mut().take(PANEL_TOP) {
                    row[inside..].fill(true);
                }
            }
            for (line, row) in rows.iter_mut().take(PANEL_TOP).enumerate() {
                if self
                    .skipped
                    .contains(&((u32::from(camera_y) + line as u32) / 8))
                {
                    row.fill(true);
                }
            }
            if self.skip_icon {
                for row in &mut rows[202..226] {
                    row[228..252].fill(true);
                }
            }
            // A full sprite table: the original's build stops at its 64th entry, so the objects
            // walked last are missing (`objects.md` 6.6a); ours of the objects are left out.
            let oam = screen::sprite_memory(&dump);
            let full = oam.len() == 256 && oam.as_chunks::<4>().0[5..].iter().all(|s| s[0] < 0xEF);
            // Likewise, the original hides a sprite partly above the top whole, where ours shows
            // its lines from 8 down (`docs/deviations.md`, "Sprites at the screen's edges").
            let edge = 8 * crate::game::SCREEN_WIDTH..16 * crate::game::SCREEN_WIDTH;
            let top_differs = ours[edge.clone()]
                .iter()
                .zip(&theirs[edge.clone()])
                .any(|(&colour, rgb)| self.palette[usize::from(colour & 0x3F)] != *rgb);
            if full || top_differs {
                let mut bare = game.clone();
                for slot in &mut bare.objects.slots[1..] {
                    slot.state = 0;
                }
                let without = self.draw(world, &bare)?;
                let span = if full { 0..ours.len() } else { edge };
                for i in span {
                    masked[i] |= ours[i] != without[i];
                }
            }
            if self.spilled {
                for line in 202..210 {
                    masked[line * crate::game::SCREEN_WIDTH + 8
                        ..line * crate::game::SCREEN_WIDTH + 136]
                        .fill(true);
                }
                let mut bare = game.clone();
                for slot in &mut bare.objects.slots[15..] {
                    slot.state = 0;
                }
                let without = self.draw(world, &bare)?;
                // The wrong tiles fill the objects' whole 8-px cells: the mask reaches 7 px
                // round our pixels.
                let (w, h) = (crate::game::SCREEN_WIDTH, PANEL_TOP);
                let near: Vec<usize> = (0..w * h).filter(|&i| ours[i] != without[i]).collect();
                for i in near {
                    let (x, y) = (i % w, i / w);
                    for yy in y.saturating_sub(7)..(y + 8).min(h) {
                        for xx in x.saturating_sub(7)..(x + 8).min(w) {
                            masked[yy * w + xx] = true;
                        }
                    }
                }
            }
            // A new object keeps its slot's earlier direction flags in the original, which
            // mirror it (`docs/deviations.md`, "Stale slot fields"): ours is also drawn with the
            // original's flags, and the closer picture judged.
            let mut flagged = game.clone();
            let mut stale = false;
            for slot in 3..crate::game::gameplay::objects::SLOTS {
                let object = &mut flagged.objects.slots[slot];
                if let Some(&flags) = fields.get(&format!("zd{slot}"))
                    && object.state != 0
                    && (u16::from(object.flags_x) << 8 | u16::from(object.flags_y)) != flags as u16
                {
                    (object.flags_x, object.flags_y) = ((flags >> 8) as u8, flags as u8);
                    stale = true;
                }
            }
            if stale {
                let redrawn = self.draw(world, &flagged)?;
                let misses = |pixels: &[u8]| {
                    screen::compare(pixels, &theirs, &self.palette, &masked).unexplained
                };
                if misses(&redrawn) < misses(&ours) {
                    ours = redrawn;
                }
            }
            self.pending = Some(Pending {
                step,
                theirs,
                masked,
                ours: ours.clone(),
                before,
                beam: (game.objects.slots[2].state == crate::game::gameplay::objects::state::BEAM)
                    .then(|| game.clone()),
            });
        }
        self.recent.push((step, ours));
        if self.recent.len() > PANEL_LAG {
            self.recent.remove(0);
        }
        Ok(verdict)
    }

    /// The verdict on the last picture.
    fn finish(&mut self) -> Result<Option<(usize, ScreenDiff)>, String> {
        match self.pending.take() {
            Some(pending) => Ok(Some(self.judge(pending, None)?)),
            None => Ok(None),
        }
    }

    fn judge(
        &mut self,
        pending: Pending,
        next: Option<&[u8]>,
    ) -> Result<(usize, ScreenDiff), String> {
        let Pending {
            step,
            theirs,
            masked,
            mut ours,
            before,
            ..
        } = pending;
        // The status panel: the best of the neighbouring steps' panels.
        let panel = PANEL_TOP * crate::game::SCREEN_WIDTH..ours.len();
        let misses = |pixels: &[u8]| {
            screen::compare(
                &pixels[panel.clone()],
                &theirs[panel.clone()],
                &self.palette,
                &masked[panel.clone()],
            )
            .unexplained
        };
        let best = before
            .iter()
            .map(Vec::as_slice)
            .chain(next)
            .filter(|candidate| misses(candidate) < misses(&ours))
            .min_by_key(|candidate| misses(candidate))
            .map(|candidate| candidate[panel.clone()].to_vec());
        if let Some(best) = best {
            ours[panel.clone()].copy_from_slice(&best);
        }
        let diff = screen::compare(&ours, &theirs, &self.palette, &masked);
        if diff.unexplained > 0
            && self.keep > 0
            && self
                .only
                .is_none_or(|(from, to)| (from..=to).contains(&step))
            && let Some(out) = &self.out
        {
            self.keep -= 1;
            let pixels = screen::side_by_side(&ours, &theirs, &self.palette, &masked);
            screen::write_png(
                &out.join(format!("step-{step}.png")),
                3 * crate::game::SCREEN_WIDTH,
                &pixels,
            )?;
        }
        Ok((step, diff))
    }
}

/// Counts a screen verdict into the report.
fn count_screen(report: &mut Report, verdict: Result<Option<(usize, ScreenDiff)>, String>) {
    match verdict {
        Ok(Some((step, diff))) => {
            report.screen_compared += 1;
            if diff.unexplained == 0 {
                report.screen_exact += 1;
            } else {
                report.screen_steps.push((step, diff));
            }
        }
        Ok(None) => {}
        Err(error) => eprintln!("screens: {error}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub step: usize,
    pub before: Fields,
    pub original: Fields,
    pub ours: Vec<(&'static str, u32)>,
}

/// Copies the original's state into the game (for lockstep comparisons and after cut passes).
fn adopt(game: &mut Gameplay, fields: &Fields) {
    adopt_pod(game, fields);
    let get = |name: &str| fields.get(name).copied().unwrap_or(0);
    game.tow.slot = get("t66") as usize;
    game.tow.mass = get("t67") as u8;
    game.tow.angle = (get("t69") << 8 | get("t68")) as u16;
    game.tow.speed = get("t6c") << 16 | get("t6b") << 8 | get("t6a");
    game.tow.sense = get("t6d") != 0;
    game.tow.spin = get("t6e") as u8;
    game.camera.position.x = get("cx") as u16;
    game.camera.position.y = get("cy") as u16;
    game.score = score_of(fields);
    let rng = get("rng");
    game.rng = Rng {
        out: (rng >> 16) as u8,
        high: (rng >> 8) as u8,
        low: rng as u8,
    };
}

/// Copies the original's pod fields (its flight, state, fuel and timers) into the game.
fn adopt_pod(game: &mut Gameplay, fields: &Fields) {
    let get = |name: &str| fields.get(name).copied().unwrap_or(0);
    let pod = &mut game.pod;
    pod.body.x = get("x") as u16;
    pod.body.x_fraction = get("xf") as u8;
    pod.body.y = get("y") as u16;
    pod.body.y_fraction = get("yf") as u8;
    pod.body.h_speed = (get("hw") << 8 | get("hf")) as u16;
    pod.body.h_left = get("hl") != 0;
    pod.body.v_speed = (get("vw") << 8 | get("vf")) as u16;
    pod.body.v_down = get("vd") != 0;
    pod.facing = get("face") as u8;
    pod.state = get("st") as u8;
    pod.fuel = get("fuel") as u16;
    pod.fuel_shown = get("fs") as u8;
    pod.move_angle = get("ang") as u8;
    pod.hit_pause = get("hp") as u8;
    pod.invulnerable = get("iv") as u8;
    pod.shield = get("sh") != 0;
    pod.accumulator_h = get("ach") as u8;
    pod.accumulator_v = get("acv") as u8;
    pod.weapon_energy = get("c9") as u8;
    pod.weapon_low = get("c8") as u8;
    pod.antigravity = get("ag") != 0;
    pod.surface = get("dd") as u8;
    pod.width = get("f1") as u8;
    pod.half_height = get("f2") as u8;
    pod.timer = get("ptm") as u8;
    pod.counter = get("pcn") as u8;
}

/// Copies the original's object slots into the game: 11-30 always (the demo's autopilot reads
/// 15-30, and the beam must find the items where the original has them), the shot slots 3-10 only
/// where ours differs in what the capture shows, so that an error in a shot is counted once.
/// Everything the capture holds is copied, the hidden bytes included, so that each step is judged
/// from the original's own state.
fn adopt_objects(game: &mut Gameplay, fields: &Fields, step: usize) {
    // The flags Y and frame bytes of every slot, whatever its state: the tow's lift sign reads
    // one of them (player.md 8.3 b).
    for slot in 1..crate::game::gameplay::objects::SLOTS {
        // A live shot is judged on its own below.
        let judged = crate::game::gameplay::objects::SHOT_SLOTS.contains(&slot)
            && fields.contains_key(&format!("os{slot}"));
        if judged {
            continue;
        }
        if let Some(&bytes) = fields.get(&format!("zy{slot}")) {
            let object = &mut game.objects.slots[slot];
            object.flags_y = (bytes >> 8) as u8;
            object.frame = bytes as u8;
        }
        if let Some(&index) = fields.get(&format!("zi{slot}")) {
            game.objects.slots[slot].index = index as u8;
        }
    }
    for slot in 3..crate::game::gameplay::objects::SLOTS {
        let get = |name: &str| fields.get(&format!("{name}{slot}")).copied();
        if crate::game::gameplay::objects::SHOT_SLOTS.contains(&slot) {
            let object = &game.objects.slots[slot];
            let hidden = (
                (u32::from(object.speed_x) << 16) | u32::from(object.speed_y),
                (u32::from(object.flags_x) << 8) | u32::from(object.flags_y),
                object.kind,
                object.work5,
            );
            let seen = (
                get("zv").unwrap_or(0),
                get("zd").unwrap_or(0),
                get("zk").unwrap_or(0) as u8,
                get("zw").unwrap_or(0) as u8,
            );
            let same = get("os") == Some(u32::from(object.state))
                && (get("os").is_none()
                    || (get("ox") == Some(u32::from(object.x))
                        && get("oy") == Some(u32::from(object.y))
                        && get("of") == Some(u32::from(object.frame))
                        && get("oi") == Some(u32::from(object.index))
                        && (object.state < 7 || hidden == seen)));
            // `SJ_HIDDEN` in the environment reports a shot whose hidden bytes differ (a debugging aid).
            if std::env::var_os("SJ_HIDDEN").is_some() && get("os").is_some() && !same {
                eprintln!("step {step} shot slot {slot}: ours {hidden:X?} original {seen:X?}");
            }
            if same {
                continue;
            }
        }
        let Some(state) = get("os") else {
            game.objects.slots[slot].state = 0;
            continue;
        };
        let object = &game.objects.slots[slot];
        if object.is_placed()
            && state < 0x80
            && let Some(entry) = game.objects.present.get_mut(usize::from(object.index))
        {
            // A placed object that is now an explosion or a thing of another state was
            // destroyed: it stays out of the placement scan's list.
            *entry = 0;
        }
        let object = &mut game.objects.slots[slot];
        object.state = state as u8;
        object.x = get("ox").unwrap_or(0) as u16;
        object.y = get("oy").unwrap_or(0) as u16;
        object.frame = get("of").unwrap_or(0) as u8;
        object.index = get("oi").unwrap_or(0) as u8;
        object.hit_points = get("oh").unwrap_or(0) as u8;
        object.x_fraction = get("qx").unwrap_or(0) as u8;
        object.y_fraction = get("qy").unwrap_or(0) as u8;
        if let Some(speeds) = get("zv") {
            object.speed_x = (speeds >> 16) as u16;
            object.speed_y = speeds as u16;
        }
        if let Some(flags) = get("zd") {
            object.flags_x = (flags >> 8) as u8;
            object.flags_y = flags as u8;
        }
        object.kind = get("zk").unwrap_or(0) as u8;
        object.work5 = get("zw").unwrap_or(0) as u8;
        object.work6 = get("zz").unwrap_or(0) as u8;
    }
}

/// The score a capture shows: its digits are written as hexadecimal digits.
fn score_of(fields: &Fields) -> Score {
    let digits = fields.get("score").copied().unwrap_or(0);
    Score(format!("{digits:X}").parse().unwrap_or_default())
}

/// The object fields of a capture's step, or of ours, as name → value (`o<letter><slot>`).
fn object_map_original(fields: &Fields) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();
    for (name, &value) in fields {
        let mut chars = name.chars();
        let (Some('o'), Some(letter)) = (chars.next(), chars.next()) else {
            continue;
        };
        let slot: usize = chars.as_str().parse().unwrap_or(0);
        // Hit points of the item slots are not defined (objects.md 10.1).
        if letter == 'h' && (11..=14).contains(&slot) {
            continue;
        }
        map.insert(name.clone(), value);
    }
    map
}

/// The attached slots' fields of a capture's step, or of ours (`a<letter><slot>`).
fn attached_original(fields: &Fields) -> BTreeMap<String, u32> {
    fields
        .iter()
        .filter(|(name, _)| {
            let b = name.as_bytes();
            b.len() == 3 && b[0] == b'a' && b"sxyfd".contains(&b[1]) && b[2].is_ascii_digit()
        })
        .map(|(name, &value)| (name.clone(), value))
        .collect()
}

fn attached_ours(game: &Gameplay) -> BTreeMap<String, u32> {
    game.attached_fields()
        .into_iter()
        .map(|(letter, slot, value)| (format!("a{letter}{slot}"), value))
        .collect()
}

fn object_map_ours(game: &Gameplay) -> BTreeMap<String, u32> {
    game.object_fields()
        .into_iter()
        .map(|(letter, slot, value)| (format!("o{letter}{slot}"), value))
        .collect()
}

/// `SJ_DUMP=FROM-TO` in the environment prints both sides' object tables for those steps
/// (a debugging aid).
fn dump_range() -> Option<(usize, usize)> {
    let text = std::env::var("SJ_DUMP").ok()?;
    let (from, to) = text.split_once('-')?;
    Some((from.parse().ok()?, to.parse().ok()?))
}

fn dump_objects(
    step: usize,
    theirs: &BTreeMap<String, u32>,
    ours: &BTreeMap<String, u32>,
    game: &Gameplay,
    fields: &Fields,
) {
    let mut slots: Vec<usize> = theirs
        .keys()
        .chain(ours.keys())
        .filter_map(|key| key[2..].parse().ok())
        .collect();
    slots.sort_unstable();
    slots.dedup();
    let show = |map: &BTreeMap<String, u32>, slot: usize| {
        "sxyfhi"
            .chars()
            .map(|letter| {
                map.get(&format!("o{letter}{slot}"))
                    .map_or("--".to_owned(), |v| format!("{v:X}"))
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut line = format!("step {step}:");
    for slot in slots {
        // The sub-pixel fractions (the capture's qx, qy), after a colon.
        let o = &game.objects.slots[slot];
        let frac_t = match (
            fields.get(&format!("qx{slot}")),
            fields.get(&format!("qy{slot}")),
        ) {
            (Some(x), Some(y)) => format!(":{x:02X}.{y:02X}"),
            _ => String::new(),
        };
        let frac_o = if o.state != 0 {
            format!(":{:02X}.{:02X}", o.x_fraction, o.y_fraction)
        } else {
            String::new()
        };
        let (t, o) = (show(theirs, slot) + &frac_t, show(ours, slot) + &frac_o);
        if t == o {
            line += &format!(" [{slot}: {t}]");
        } else {
            line += &format!(" [{slot}: original {t} | ours {o}]");
        }
    }
    eprintln!("{line}");
}

/// Plays `replay` in `src/game` and compares it with `capture`. With `lockstep` the
/// game takes the original's state after every step, so each step is judged on its own.
pub fn compare(world: &World, replay: &Replay, capture: &Capture, lockstep: bool) -> Report {
    compare_with_sound(world, replay, capture, lockstep, None, None, None)
}

/// The previous occupants' bytes in the free object slots (the capture's `l` fields): the
/// speeds, directions, work bytes, hit points and sub-pixel fractions a new object keeps where its
/// creator does not clear them (behaviour.md 2.5, Q17), and with `positions` the place and picture.
fn seed_leftovers(game: &mut Gameplay, fields: &Fields, positions: bool) {
    for slot in 3..crate::game::gameplay::objects::SLOTS {
        let leftover = |name: &str| fields.get(&format!("{name}{slot}")).copied();
        let object = &mut game.objects.slots[slot];
        if object.state != 0 || leftover("lh").is_none() {
            continue;
        }
        object.x_fraction = leftover("lx").unwrap_or(0) as u8;
        object.y_fraction = leftover("ly").unwrap_or(0) as u8;
        let speeds = leftover("lv").unwrap_or(0);
        (object.speed_x, object.speed_y) = ((speeds >> 16) as u16, speeds as u16);
        let flags = leftover("ld").unwrap_or(0);
        (object.flags_x, object.flags_y) = ((flags >> 8) as u8, flags as u8);
        object.kind = leftover("lk").unwrap_or(0) as u8;
        object.work5 = leftover("lw").unwrap_or(0) as u8;
        object.work6 = leftover("lz").unwrap_or(0) as u8;
        object.index = leftover("li").unwrap_or(0) as u8;
        object.hit_points = leftover("lh").unwrap_or(0) as u8;
        if positions {
            let position = leftover("lp").unwrap_or(0);
            (object.x, object.y) = ((position >> 16) as u16, position as u16);
            object.frame = leftover("lf").unwrap_or(0) as u8;
        }
    }
}

/// The replay's test setups before step `step` (1-based).
pub fn apply_setups(game: &mut Gameplay, replay: &Replay, step: usize) {
    if replay.keep_fuel.is_some_and(|from| step >= from) {
        game.pod.fuel = 0xE000;
    }
    if let Some(&fuel) = replay.fuel_pokes.get(&step) {
        game.pod.fuel = fuel;
    }
    if let Some(&[x, y, cx, cy]) = replay.warps.get(&step) {
        game.warp(x, y, cx, cy);
    }
    for &(slot, state, x, y, frame, index) in replay.objs.get(&step).into_iter().flatten() {
        game.set_object(slot, state, x, y, frame, index);
    }
    if let Some(&spin) = replay.spins.get(&step) {
        game.tow.spin = spin;
    }
    for &(index, kind, x, y) in replay.items.get(&step).into_iter().flatten() {
        game.set_item(index, kind, x, y);
    }
}

/// The game on the level a scene change led to (`Gameplay::change_level`, and the badge screen
/// after a warp level).
fn next_level(world: &World, game: &Gameplay, transition: Transition) -> Gameplay {
    match transition {
        Transition::WarpEnd => game.after_badge(world),
        _ => game.change_level(world),
    }
}

/// The game at the start of a replay: the level, the original's start state from the capture,
/// the original's rules and the first step's test setups.
pub fn start_game(
    world: &World,
    replay: &Replay,
    capture: &Capture,
    validation: Option<&ValidationInput>,
) -> Gameplay {
    let get = |name: &str| capture.init.get(name).copied().unwrap_or(0);
    let seed = Seed {
        rng: Rng {
            out: (get("rng") >> 16) as u8,
            high: (get("rng") >> 8) as u8,
            low: get("rng") as u8,
        },
        counter: get("ctr") >> 16 | (get("ctr") >> 8 & 0xFF) << 8 | (get("ctr") & 0xFF) << 16,
        score: score_of(&capture.init),
        saved_fuel_count: get("sf") as u8,
        ..Seed::default()
    };
    let level = replay.demo.map_or(replay.level, |n| [6, 4, 10][n - 1]);
    let seed = Seed {
        equipment: if replay.demo.is_some() { 0xC0 } else { 0 },
        weapon: replay.demo.map_or(replay.weapon, |n| [1, 8, 6][n - 1]),
        device: replay.device,
        bullet_upgrade: replay.bullet_upgrade,
        weapon_gauge: replay.weapon_gauge,
        lives: replay.lives.unwrap_or(seed.lives),
        planet_flags: replay.flags,
        // A demo starts with the object table's bytes the previous session left (the first,
        // the power-on RAM).
        leftovers: if replay.demo.is_some() {
            (0..crate::game::gameplay::objects::SLOTS)
                .map(|slot| {
                    let get = |name: &str| {
                        capture
                            .init
                            .get(&format!("{name}{slot}"))
                            .copied()
                            .unwrap_or(0)
                    };
                    (get("ld") as u8, get("lf") as u8)
                })
                .collect()
        } else {
            seed.leftovers
        },
        // A demo starts with the previous session's star job mode.
        star_mode: if replay.demo.is_some() {
            get("km") as u8
        } else {
            0
        },
        // A demo starts with the previous session's bytes (the capture's start values).
        pod_leftovers: if replay.demo.is_some() {
            PodLeftovers {
                move_angle: get("ang") as u8,
                invulnerable: get("iv") as u8,
                accumulator_h: get("ach") as u8,
                accumulator_v: get("acv") as u8,
                camera_carry: get("cc") as u8,
                tow_spin: get("t6e") as u8,
            }
        } else {
            PodLeftovers::default()
        },
        ..seed
    };
    let mut game = Gameplay::new(world, level, seed);
    if replay.demo.is_some() {
        game.start_demo(world, get("tm") as u8, Buttons(0), false);
    }
    if replay.demo.is_none() && replay.level > 12 {
        // A level entered by a transition keeps the pod's directions from the level before.
        game.pod.body.v_down = get("vd") != 0;
        game.pod.body.h_left = get("hl") != 0;
        game.pod.move_angle = get("ang") as u8;
        game.pod.timer = get("ptm") as u8;
        // And the F gauge's display copy, which only a launch empties (economy.md 3.1).
        game.pod.fuel_shown = get("fs") as u8;
    }
    if replay.level >= 29 {
        // A warp level reached by the forced changes of the capture script starts with the
        // guard and the items cursor the second level left.
        game.set_start_bytes(get("grd") as u8, get("ic") as usize);
    }
    // The comparison validates the original's rules, not the remake's deviations.
    game.set_original_rules(true);
    // The setups of the helper slots 23 and 24 clear their work bytes, which are also bytes
    // of the warp mesh's variant table (graphics.md 5.7.2).
    let mesh_holes: Vec<usize> = [(23, [66, 97]), (24, [67, 98])]
        .into_iter()
        .filter(|&(slot, _)| replay.objs.values().flatten().any(|o| o.0 == slot))
        .flat_map(|(_, bytes)| bytes)
        .collect();
    game.mesh_holes = mesh_holes.clone();
    if replay.demo.is_none() && replay.level < 13 {
        // Slot 1's flag bytes as the title's scroll helper left them (player.md 7.4).
        (game.objects.slots[1].flags_x, game.objects.slots[1].flags_y) = (1, 1);
    }
    game.set_validation_input(validation.cloned());
    if replay.demo.is_none() {
        game.pod.equipment.owned = replay.equipment;
    }
    game.tow.angle = (get("t69") << 8 | get("t68")) as u16;
    game.tow.speed = get("t6c") << 16 | get("t6b") << 8 | get("t6a");
    game.tow.sense = get("t6d") != 0;
    // The parallax stars as the session before left them: their mode, bytes and dead mask
    // (`kd`, bit 7 for star 0) outlast a level load (graphics.md 5.7.3). Older captures have
    // no mask: then only a demo is seeded, with every star dead.
    let dead_mask = capture.init.get("kd").copied();
    if replay.demo.is_some() || dead_mask.is_some() {
        if dead_mask.is_some() {
            game.seed_star_mode(get("km") as u8);
        }
        for i in 0..8 {
            let star = |name: &str| {
                capture
                    .init
                    .get(&format!("{name}{i}"))
                    .copied()
                    .unwrap_or(0)
            };
            game.seed_star(
                i,
                (star("kx") as u8, star("ky") as u8),
                (star("kf") as u8, star("kg") as u8),
                dead_mask.is_none_or(|mask| mask & (0x80 >> i) != 0),
            );
        }
    }
    if replay.demo.is_some() || replay.level >= 13 {
        seed_leftovers(&mut game, &capture.init, replay.demo.is_none());
    }
    // The first step's test setups are in place before the original's start state is read.
    apply_setups(&mut game, replay, 1);
    game
}

/// Frames of a scene change that service the request slots (the message screen's and the
/// loading frames; game-flow.md 1.1).
const SCENE_FRAMES: usize = 24;

/// The sound requests one pass makes, in the form the capture logs them. The model is the
/// game's side of the driver: the driver's step, then the pass with its slot service, then
/// the pass's own requests (game-flow.md 1.1).
pub struct SoundModel {
    pub sound: Sound,
    sink: Vec<crate::game::sound::RegisterWrite>,
}

impl SoundModel {
    pub fn new(data: &SoundData) -> SoundModel {
        let mut sound = Sound::new(data.clone());
        sound.record_starts(true);
        SoundModel {
            sound,
            sink: Vec::new(),
        }
    }

    /// One frame: the driver, the service at the start of the pass, then `requests`.
    pub fn pass(
        &mut self,
        parity: u8,
        counter: u32,
        requests: Vec<Request>,
        tokens: &mut Vec<String>,
    ) {
        self.sound.set_frame(counter as u8);
        self.sink.clear();
        self.sound.update(&mut self.sink);
        self.sound.service();
        let mut songs = Vec::new();
        for request in requests {
            let before = self.sound.driver().song();
            self.sound.apply(request);
            if let Request::Song { id, always } = request {
                // The "if different" entry calls the "always" one when it starts the song.
                songs.push(format!("m{parity}.{}.{id}", if always { 'A' } else { 'S' }));
                if !always && self.sound.driver().song() != before {
                    songs.push(format!("m{parity}.A.{id}"));
                }
            }
        }
        for (channel, id) in self.sound.take_starts() {
            let channel = Channel::ALL.iter().position(|&c| c == channel).unwrap_or(0);
            tokens.push(format!("e{parity}.{channel}.{id:02X}"));
        }
        tokens.extend(songs);
    }
}

pub fn compare_with_sound(
    world: &World,
    replay: &Replay,
    capture: &Capture,
    lockstep: bool,
    sound_data: Option<&SoundData>,
    validation: Option<&ValidationInput>,
    mut screens: Option<&mut ScreenCheck>,
) -> Report {
    let mut model = sound_data.map(SoundModel::new);
    if let Some(screens) = screens.as_deref_mut() {
        // The capture launches a new game (the standard pod, nothing fitted) and then writes the
        // replay's equipment into RAM.
        if replay.demo.is_none() {
            screens.pod_tiles = Some(0);
            screens.skip_icon = replay.weapon != 0;
        }
    }
    let mut game = start_game(world, replay, capture, validation);
    let mesh_holes = game.mesh_holes.clone();
    let mut report = Report::default();
    for (name, value) in game.snapshot() {
        let theirs = capture.init.get(name).copied().unwrap_or(0);
        // The controller bytes before the first pass are leftovers of the title.
        if theirs != value && !matches!(name, "ih" | "ip") {
            report.init.push((name, theirs, value));
        }
    }
    report.objects = OBJECT_FIELDS
        .iter()
        .map(|&(_, name)| (name, FieldReport::default()))
        .collect();
    // The start state of the objects.
    {
        let (theirs, ours) = (object_map_original(&capture.init), object_map_ours(&game));
        for key in theirs.keys().chain(ours.keys()) {
            if theirs.get(key) != ours.get(key) {
                report.init.push((
                    "objects",
                    theirs.get(key).copied().unwrap_or(0),
                    ours.get(key).copied().unwrap_or(0),
                ));
                eprintln!(
                    "start differs: {key} original {:?} ours {:?}",
                    theirs.get(key),
                    ours.get(key)
                );
            }
        }
    }
    report.fields = game
        .snapshot()
        .into_iter()
        .map(|(name, _)| (name, FieldReport::default()))
        .collect();
    let samples: BTreeMap<usize, &Sample> = capture.samples.iter().map(|s| (s.step, s)).collect();
    let last = capture
        .samples
        .last()
        .map_or(0, |s| s.step)
        .max(capture.left_at.unwrap_or(0))
        .max(capture.reloads.keys().copied().max().unwrap_or(0));
    let count = if replay.demo.is_some() {
        last
    } else {
        replay.steps.len().min(last)
    };
    report.steps = count;
    report.original_left = capture.left_at;
    let mut previous_fields = capture.init.clone();
    // The level in which the original's terrain list spilled (Q45, below), until it reloads.
    let mut spilled: Option<usize> = None;
    for index in 0..count {
        let buttons = replay.steps.get(index).copied().unwrap_or_default();
        let step = index + 1;
        if step > 1 {
            apply_setups(&mut game, replay, step);
        }
        if replay.demo.is_some()
            && let Some(sample) = samples.get(&step)
        {
            // The distance routine's scratch byte as the original's objects found it.
            for slot in 15..crate::game::gameplay::objects::SLOTS {
                game.objects.scratch_feed[slot] =
                    sample.fields.get(&format!("sr{slot}")).map(|&v| v as u8);
            }
        }
        let mut tokens = Vec::new();
        let counter = game.counter();
        game.pass(world, buttons);
        if let Some(model) = model.as_mut() {
            model.pass(0, counter, game.take_sound(), &mut tokens);
        }
        // The B pass follows unless the A pass asked for a scene change or left the frame
        // counter even (the gate state's first call, player.md 9.2.3).
        if game.transition().is_none() && !game.next_is_a_pass() {
            game.pass(world, buttons);
            if let Some(model) = model.as_mut() {
                model.pass(1, counter.wrapping_add(1), game.take_sound(), &mut tokens);
            }
        }
        if game.demo_over() {
            // The demo ends where a scene change would (code 0).
            report.our_transition.get_or_insert((step, 0));
            break;
        }
        if let Some(transition) = game.transition() {
            report
                .our_transition
                .get_or_insert((step, transition.code()));
            if let Some(fields) = capture.reloads.get(&step) {
                spilled = None;
                // The original reloaded and came back: so do we, then take the loading frames'
                // effect on the counters from it (docs/deviations.md).
                let outcome = game.resume(world);
                // The badge screen after a warp level counts the crystals, one effect on both
                // pulses each, 16 frames apart, and starts silence (scenes.md 11.2).
                let gems = if transition == Transition::WarpEnd {
                    game.warp_crystals
                } else {
                    0
                };
                let mut started = Vec::new();
                if let Elsewhere(next) = outcome {
                    game = next_level(world, &game, next);
                    game.mesh_holes = mesh_holes.clone();
                    if let Some(model) = model.as_mut() {
                        if transition == Transition::WarpEnd {
                            model.sound.apply(Request::Song {
                                id: 0,
                                always: false,
                            });
                            for gem in 0..gems {
                                let id = 0x1E + 2 * gem.min(9);
                                model.sound.apply(Request::a(Channel::Pulse1, id));
                                model.sound.apply(Request::b(Channel::Pulse2, id));
                                for _ in 0..16 {
                                    model.sink.clear();
                                    model.sound.update(&mut model.sink);
                                    model.sound.service();
                                    for (channel, id) in model.sound.take_starts() {
                                        let channel =
                                            Channel::ALL.iter().position(|&c| c == channel);
                                        started
                                            .push(format!("ex.{}.{id:02X}", channel.unwrap_or(0)));
                                    }
                                }
                            }
                        }
                        // The new level starts its own song (scenes.md 0.1).
                        model.sound.apply(Request::Song {
                            id: crate::game::scene::planet_song(game.level()),
                            always: false,
                        });
                    }
                }
                if let Some(model) = model.as_mut() {
                    // The scene change's frames (the message, the loading) service whatever
                    // is waiting in the slots, and the respawn asks for effect $32 and the
                    // lock; the original starts them there. Typing clicks and songs belong to
                    // the scene layer.
                    for request in game.take_sound() {
                        model.sound.apply(request);
                    }
                    // A lost Jetman is handled one frame after the request, and the level
                    // loads with the lock set (Mesen: one servicing, then the next pass).
                    let frames = if transition.code() == 4 {
                        model.sound.set_frame(counter.wrapping_add(2) as u8);
                        1
                    } else {
                        SCENE_FRAMES
                    };
                    for _ in 0..frames {
                        model.sink.clear();
                        model.sound.update(&mut model.sink);
                        model.sound.service();
                        for (channel, id) in model.sound.take_starts() {
                            let channel = Channel::ALL.iter().position(|&c| c == channel);
                            started.push(format!("ex.{}.{id:02X}", channel.unwrap_or(0)));
                        }
                    }
                    let theirs: Vec<String> = capture
                        .scene_sound
                        .get(&step)
                        .into_iter()
                        .flatten()
                        .filter(|t| !t.starts_with("mx") && !t.ends_with(".3.3A"))
                        .cloned()
                        .collect();
                    report.sound_compared += 1;
                    if theirs == started {
                        report.sound_exact += 1;
                    } else if report.sound_first.is_none() {
                        report.sound_first = Some((step, theirs, started));
                    }
                }
                if game.level() >= 29 {
                    // The warp level's items scan starts where the level before left its
                    // cursor, and the slots hold what the level before left in them.
                    game.set_start_bytes(0x40, fields.get("ic").copied().unwrap_or(0) as usize);
                    seed_leftovers(&mut game, fields, true);
                }
                let ctr = fields.get("ctr").copied().unwrap_or(0);
                game.set_counter((ctr >> 16 & 0xFF) | (ctr >> 8 & 0xFF) << 8 | (ctr & 0xFF) << 16);
                let rng = fields.get("rng").copied().unwrap_or(0);
                game.rng = Rng {
                    out: (rng >> 16) as u8,
                    high: (rng >> 8) as u8,
                    low: rng as u8,
                };
                let diffs = game
                    .snapshot()
                    .into_iter()
                    .filter_map(|(name, ours)| {
                        let theirs = fields.get(name).copied().unwrap_or(0);
                        (theirs != ours).then(|| (name.to_owned(), theirs, ours))
                    })
                    .collect();
                report.reloads.push((step, diffs));
                continue;
            }
            break;
        }
        let Some(&sample) = samples.get(&step) else {
            continue;
        };
        if sample.cut {
            report.cut += 1;
            adopt(&mut game, &sample.fields);
            if replay.demo.is_some() {
                adopt_objects(&mut game, &sample.fields, sample.step);
            }
            previous_fields = sample.fields.clone();
            continue;
        }
        report.compared += 1;
        if model.is_some() {
            report.sound_compared += 1;
            // SJ_SOUNDS=1 lists the steps whose requests differ, SJ_SOUNDS=all every step with any.
            let listing = std::env::var("SJ_SOUNDS").ok();
            let same = sample.sound == tokens;
            if listing.as_deref() == Some("all") && !(same && tokens.is_empty())
                || listing.is_some() && !same
            {
                eprintln!(
                    "step {}: original {:?} ours {:?}",
                    sample.step, sample.sound, tokens
                );
            }
            if same {
                report.sound_exact += 1;
            } else if report.sound_first.is_none() {
                report.sound_first = Some((sample.step, sample.sound.clone(), tokens.clone()));
            }
        }
        if replay.pod_sync {
            adopt_pod(&mut game, &sample.fields);
        }
        let mut exact = true;
        let ours_now = game.snapshot();
        if report.first_divergence.is_none()
            && ours_now.iter().any(|&(name, v)| {
                name != "rng" && name != "cc" && sample.fields.get(name) != Some(&v)
            })
        {
            report.first_divergence = Some(Divergence {
                step: sample.step,
                before: previous_fields.clone(),
                original: sample.fields.clone(),
                ours: ours_now.clone(),
            });
        }
        if std::env::var_os("SJ_DIFFS").is_some() {
            let diffs: Vec<String> = ours_now
                .iter()
                .filter(|&&(name, v)| {
                    name != "rng" && sample.fields.get(name).copied().unwrap_or(0) != v
                })
                .map(|&(name, v)| {
                    format!(
                        "{name} {:X}/{v:X}",
                        sample.fields.get(name).copied().unwrap_or(0)
                    )
                })
                .collect();
            if !diffs.is_empty() {
                eprintln!(
                    "step {} (input {:02X}): {}",
                    sample.step,
                    buttons.0,
                    diffs.join(" ")
                );
            }
        }
        for ((name, ours), (_, field)) in ours_now.into_iter().zip(&mut report.fields) {
            let theirs = sample.fields.get(name).copied().unwrap_or(0);
            field.total += 1;
            if theirs == ours {
                field.matched += 1;
            } else {
                exact = false;
                field.first.get_or_insert((sample.step, theirs, ours));
            }
        }
        report.exact += usize::from(exact);
        // The objects: per field, a step matches when every slot's value does.
        let (theirs, ours) = (object_map_original(&sample.fields), object_map_ours(&game));
        if let Some((from, to)) = dump_range()
            && (from..=to).contains(&sample.step)
        {
            dump_objects(sample.step, &theirs, &ours, &game, &sample.fields);
        }
        let mut objects_exact = true;
        for (&(letter, _), (_, field)) in OBJECT_FIELDS.iter().zip(&mut report.objects) {
            let of_letter = |key: &&String| key.chars().nth(1) == Some(letter);
            let mut differs = None;
            let mut values = 0;
            for key in theirs.keys().chain(ours.keys()).filter(of_letter) {
                values += 1;
                if theirs.get(key) != ours.get(key) && differs.is_none() {
                    differs = Some(key.clone());
                }
            }
            report.object_values += values;
            field.total += 1;
            match differs {
                None => field.matched += 1,
                Some(key) => {
                    objects_exact = false;
                    if field.first.is_none() {
                        field.first = Some((
                            sample.step,
                            theirs.get(&key).copied().unwrap_or(u32::MAX),
                            ours.get(&key).copied().unwrap_or(u32::MAX),
                        ));
                        field.at = key;
                    }
                }
            }
        }
        report.objects_exact += usize::from(objects_exact);
        report.all_exact += usize::from(exact && objects_exact);
        let (theirs, ours) = (attached_original(&sample.fields), attached_ours(&game));
        let differs = theirs
            .keys()
            .chain(ours.keys())
            .find(|key| theirs.get(*key) != ours.get(*key));
        match differs {
            None => report.attached_exact += 1,
            Some(key) if report.attached_first.is_none() => {
                report.attached_first = Some((
                    sample.step,
                    key.clone(),
                    theirs.get(key).copied().unwrap_or(u32::MAX),
                    ours.get(key).copied().unwrap_or(u32::MAX),
                ));
            }
            Some(_) => {}
        }
        // The original's picture of the step before a warp already shows the warp's camera, and
        // for three steps from the warp some of its attribute bytes are still the old picture's,
        // until the game's rotating attribute refresh has rewritten them (`levels.md`, Appendix A).
        // A warp before step 1 comes after the level's own draw and cannot be redrawn: the
        // picture is stale until the next warp.
        let warped = (sample.step.saturating_sub(2)..=sample.step + 1)
            .any(|step| step > 1 && replay.warps.contains_key(&step))
            || (replay.warps.contains_key(&1)
                && !replay
                    .warps
                    .keys()
                    .any(|&at| (2..=sample.step).contains(&at)));
        if let Some(screens) = screens.as_deref_mut() {
            // A warp before step 1 comes before the level's own draw.
            let warp_y = replay
                .warps
                .get(&sample.step)
                .filter(|_| sample.step > 1)
                .map(|warp| warp[3]);
            screens.track_rows(game.level(), sample.step, game.camera.position.y, warp_y);
        }
        // Quirk Q45 (`docs/deviations.md`): once the terrain test's list reaches $38 bytes the
        // original's spill sticks the F gauge and overwrites table A's tile ranges, which stay
        // wrong until the level loads again; the remake does neither, so from the first spill
        // to the next load the F gauge and those objects are masked.
        if game.pod.list_size >= 0x38 {
            spilled = Some(game.level());
        }
        if spilled.is_some_and(|level| level != game.level()) {
            spilled = None;
        }
        if let Some(screens) = screens.as_deref_mut() {
            screens.spilled = spilled.is_some();
        }
        if warped && screens.is_some() {
            report.screen_skipped += 1;
        } else if let Some(screens) = screens.as_deref_mut() {
            // `SJ_POD_BITS` prints the pod's palette bits and hit pause per step (a debugging aid).
            if std::env::var_os("SJ_POD_BITS").is_some() {
                eprintln!(
                    "bits {} {} {}",
                    sample.step, game.pod.palette_bits, game.pod.hit_pause
                );
            }
            let verdict = screens.check(world, &game, sample.step, &sample.fields);
            count_screen(&mut report, verdict);
        }
        previous_fields = sample.fields.clone();
        if lockstep {
            adopt(&mut game, &sample.fields);
            if replay.demo.is_some() {
                adopt_objects(&mut game, &sample.fields, sample.step);
            }
        }
    }
    if let Some(screens) = screens {
        let verdict = screens.finish();
        count_screen(&mut report, verdict);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_format() {
        let replay = parse_replay("# c\ndc 80\n2 -\n1 B L # thrust\n").unwrap();
        assert_eq!(replay.equipment, 0x80);
        assert_eq!(
            replay.steps,
            vec![Buttons(0), Buttons(0), Buttons(Buttons::B | Buttons::LEFT)]
        );
        assert!(parse_replay("1 Q").is_err());
    }

    #[test]
    fn capture_format() {
        let text = "init x=027B rng=78C903\ns 1 0 x=027B rng=88\ns 2 1 x=027C\nend\n";
        let capture = parse_capture(text).unwrap();
        assert_eq!(capture.init["x"], 0x27B);
        assert_eq!(capture.samples.len(), 2);
        assert!(capture.samples[1].cut);
        assert!(parse_capture("end\n").is_err());
    }
}
