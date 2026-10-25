//! The port menu (F1): the model, with no window. It owns the cursor and changes
//! [`Settings`]; the program saves them and applies them. `ui` draws it.

use crate::controls::{self, Control, Device, Keys, Pad, Pressed};
use crate::settings::{FINAL_LEVEL, MAX_SCALE, MAX_VOLUME, SLOTS, Settings};
use std::time::Duration;

/// How long the menu waits for the key or button to bind.
pub const LISTEN_TIME: Duration = Duration::from_secs(5);

/// What a control asks of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Physics,
    Video,
    Sound,
    Controls,
    Cheats,
    SaveStates,
    Reset,
    Quit,
    Vsync,
    Scale,
    Fullscreen,
    Overscan,
    Aspect,
    Volume,
    Mute,
    Keyboard,
    Gamepad,
    Bind(Control),
    Defaults,
    InfiniteLives,
    InfiniteFuel,
    FullGauge,
    LevelSelect,
    Slot,
    Save,
    Load,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Video,
    Sound,
    Controls,
    Keyboard,
    Gamepad,
    Cheats,
    SaveStates,
}

/// The rows of the keyboard and gamepad pages.
const BINDING_ROWS: &[Row] = &[
    Row::Bind(Control::A),
    Row::Bind(Control::B),
    Row::Bind(Control::Up),
    Row::Bind(Control::Down),
    Row::Bind(Control::Left),
    Row::Bind(Control::Right),
    Row::Bind(Control::Select),
    Row::Bind(Control::Start),
    Row::Bind(Control::Menu),
    Row::Bind(Control::QuickSave),
    Row::Bind(Control::QuickLoad),
    Row::Bind(Control::Screenshot),
    Row::Bind(Control::Quit),
    Row::Defaults,
    Row::Back,
];

impl Page {
    /// The rows the cursor visits.
    fn rows(self) -> &'static [Row] {
        match self {
            Page::Main => &[
                Row::Physics,
                Row::Video,
                Row::Sound,
                Row::Controls,
                Row::Cheats,
                Row::SaveStates,
                Row::Reset,
                Row::Quit,
            ],
            Page::Video => &[
                Row::Vsync,
                Row::Scale,
                Row::Fullscreen,
                Row::Overscan,
                Row::Aspect,
                Row::Back,
            ],
            Page::Sound => &[Row::Volume, Row::Mute, Row::Back],
            Page::Controls => &[Row::Keyboard, Row::Gamepad, Row::Back],
            Page::Keyboard | Page::Gamepad => BINDING_ROWS,
            Page::Cheats => &[
                Row::InfiniteLives,
                Row::InfiniteFuel,
                Row::FullGauge,
                Row::LevelSelect,
                Row::Back,
            ],
            Page::SaveStates => &[Row::Slot, Row::Save, Row::Load, Row::Back],
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Main => "SOLAR JETMAN: HUNT FOR THE GOLDEN WARPSHIP: REMASTERED",
            Page::Video => "VIDEO",
            Page::Sound => "SOUND",
            Page::Controls => "CONTROLS",
            Page::Keyboard => "KEYBOARD",
            Page::Gamepad => "GAMEPAD",
            Page::Cheats => "CHEATS",
            Page::SaveStates => "SAVE STATES",
        }
    }

    /// The device a bindings page rebinds.
    fn device(self) -> Option<Device> {
        match self {
            Page::Keyboard => Some(Device::Keyboard),
            Page::Gamepad => Some(Device::Gamepad),
            _ => None,
        }
    }
}

/// What the program should do besides saving the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Save the state in the slot.
    Save(usize),
    /// Load the state in the slot.
    Load(usize),
    /// Start the game at this level now: a planet (0-12) or the final level (13). A one-shot
    /// action, not a setting.
    StartLevel(u8),
    /// Reset the game, as the console's reset button: back to the title.
    Reset,
}

/// What a key press did to the program.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub settings_changed: bool,
    pub quit: bool,
    pub action: Option<Action>,
}

/// One drawn row.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    pub label: String,
    pub value: String,
    pub focused: bool,
    /// Not a choice: a heading or a note.
    pub dim: bool,
}

#[derive(Debug)]
pub struct Menu {
    open: bool,
    focus: usize,
    page: Page,
    /// The pages above this one, each with the row to return to.
    trail: Vec<(Page, usize)>,
    /// The last result of a save or load, shown on that page.
    notice: String,
    /// The level the level select would start (0-12 are the planets, 13 the final level).
    /// Not a setting: the menu forgets it when the program ends.
    level: u8,
    /// The control waiting for a new key or button, and the time left to press one.
    listen: Option<(Control, Duration)>,
}

impl Default for Menu {
    fn default() -> Self {
        Menu {
            open: false,
            focus: 0,
            page: Page::Main,
            trail: Vec::new(),
            notice: String::new(),
            level: 0,
            listen: None,
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// The level select's choices in order: the planets 0-12, the final level.
fn cycle_level(now: u8, step: i32) -> u8 {
    let next = (i32::from(now) + step).rem_euclid(i32::from(FINAL_LEVEL) + 1);
    u8::try_from(next).unwrap_or(0)
}

impl Menu {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.page = Page::Main;
        self.focus = 0;
        self.trail.clear();
        self.listen = None;
    }

    pub fn page_title(&self) -> &'static str {
        self.page.title()
    }

    /// Shows the result of a save or load on the save-states page.
    pub fn set_notice(&mut self, text: &str) {
        self.notice = text.to_owned();
    }

    /// The device whose next key or button the menu wants for a binding.
    pub fn listening(&self) -> Option<Device> {
        self.listen.and(self.page.device())
    }

    /// Counts down the wait for a binding; when it runs out the binding stays as it was.
    pub fn tick(&mut self, elapsed: Duration) {
        if let Some((control, left)) = self.listen {
            self.listen = left
                .checked_sub(elapsed)
                .filter(|left| !left.is_zero())
                .map(|left| (control, left));
        }
    }

    /// Binds the key or button the menu was waiting for.
    pub fn bind(&mut self, pressed: Pressed, settings: &mut Settings) -> Outcome {
        let Some((control, _)) = self.listen.take() else {
            return Outcome::default();
        };
        let before = settings.clone();
        match (self.page.device(), pressed) {
            (Some(Device::Keyboard), Pressed::Key(key)) => settings.keys.bind(control, key),
            (Some(Device::Gamepad), Pressed::Button(button)) => settings.pad.bind(control, button),
            _ => {}
        }
        Outcome {
            settings_changed: *settings != before,
            ..Outcome::default()
        }
    }

    fn enter(&mut self, page: Page) {
        self.trail.push((self.page, self.focus));
        self.page = page;
        self.focus = 0;
        self.notice.clear();
    }

    /// Back to the page above; from the main page, closes the menu.
    fn leave(&mut self) {
        match self.trail.pop() {
            Some((page, focus)) => {
                self.page = page;
                self.focus = focus;
            }
            None => self.open = false,
        }
    }

    /// Handles one control. Ignored while the menu waits for a binding.
    pub fn handle(&mut self, nav: Nav, settings: &mut Settings) -> Outcome {
        let mut out = Outcome::default();
        if self.listen.is_some() {
            return out;
        }
        let rows = self.page.rows();
        let row = rows[self.focus];
        match nav {
            Nav::Up => self.focus = (self.focus + rows.len() - 1) % rows.len(),
            Nav::Down => self.focus = (self.focus + 1) % rows.len(),
            Nav::Back => self.leave(),
            Nav::Left | Nav::Right | Nav::Accept => {
                let step: i32 = if nav == Nav::Left { -1 } else { 1 };
                let accept = nav == Nav::Accept;
                let before = settings.clone();
                match row {
                    Row::Physics => settings.remastered = !settings.remastered,
                    Row::Vsync => settings.vsync = !settings.vsync,
                    Row::Fullscreen => settings.fullscreen = !settings.fullscreen,
                    Row::Overscan => settings.crop_overscan = !settings.crop_overscan,
                    Row::Aspect => settings.native_aspect = !settings.native_aspect,
                    Row::Mute => settings.mute = !settings.mute,
                    Row::Scale => {
                        let n = i32::try_from(MAX_SCALE).unwrap_or(0) + 1;
                        let now = i32::try_from(settings.scale).unwrap_or(0);
                        settings.scale = u32::try_from((now + step).rem_euclid(n)).unwrap_or(0);
                    }
                    Row::Volume if !accept => {
                        let next = i32::from(settings.volume) + step;
                        settings.volume = next.clamp(0, i32::from(MAX_VOLUME)) as u8;
                    }
                    Row::Bind(control) if accept => self.listen = Some((control, LISTEN_TIME)),
                    Row::Defaults if accept => match self.page.device() {
                        Some(Device::Keyboard) => settings.keys = Keys::default(),
                        Some(Device::Gamepad) => settings.pad = Pad::default(),
                        None => {}
                    },
                    Row::InfiniteLives => {
                        settings.cheats.infinite_lives = !settings.cheats.infinite_lives;
                    }
                    Row::InfiniteFuel => {
                        settings.cheats.infinite_fuel = !settings.cheats.infinite_fuel;
                    }
                    Row::FullGauge => settings.cheats.full_gauge = !settings.cheats.full_gauge,
                    Row::LevelSelect if accept => out.action = Some(Action::StartLevel(self.level)),
                    Row::LevelSelect => self.level = cycle_level(self.level, step),
                    Row::Slot => {
                        let n = i32::try_from(SLOTS).unwrap_or(1);
                        let now = i32::try_from(settings.slot).unwrap_or(0);
                        settings.slot = usize::try_from((now + step).rem_euclid(n)).unwrap_or(0);
                        self.notice.clear();
                    }
                    Row::Save if accept => {
                        out.action = Some(Action::Save(settings.slot));
                    }
                    Row::Load if accept => {
                        out.action = Some(Action::Load(settings.slot));
                    }
                    Row::Video if accept => self.enter(Page::Video),
                    Row::Sound if accept => self.enter(Page::Sound),
                    Row::Controls if accept => self.enter(Page::Controls),
                    Row::Keyboard if accept => self.enter(Page::Keyboard),
                    Row::Gamepad if accept => self.enter(Page::Gamepad),
                    Row::Cheats if accept => self.enter(Page::Cheats),
                    Row::SaveStates if accept => self.enter(Page::SaveStates),
                    Row::Back if accept => self.leave(),
                    Row::Reset if accept => out.action = Some(Action::Reset),
                    Row::Quit if accept => out.quit = true,
                    _ => {}
                }
                out.settings_changed = *settings != before;
            }
        }
        out
    }

    /// A binding's value: its key or button, or the countdown while the menu waits for one.
    fn binding(&self, control: Control, settings: &Settings) -> String {
        if let Some((waiting, left)) = self.listen
            && waiting == control
        {
            let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
            return match self.page.device() {
                Some(Device::Gamepad) => format!("PRESS A BUTTON {seconds}"),
                _ => format!("PRESS A KEY {seconds}"),
            };
        }
        match self.page.device() {
            Some(Device::Gamepad) => controls::button_name(settings.pad.get(control)),
            _ => controls::key_name(settings.keys.get(control)),
        }
    }

    /// The rows of the current page. `filled` tells which save slots hold a state.
    pub fn lines(&self, settings: &Settings, filled: [bool; SLOTS]) -> Vec<Line> {
        let text_line = |label: &str, value: &str, dim: bool| Line {
            label: label.into(),
            value: value.into(),
            focused: false,
            dim,
        };
        let mut lines: Vec<Line> = self
            .page
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let (label, value) = match row {
                    Row::Physics => (
                        "PHYSICS",
                        if settings.remastered {
                            "REMASTERED"
                        } else {
                            "ORIGINAL"
                        }
                        .into(),
                    ),
                    Row::Vsync => ("VSYNC", on_off(settings.vsync).into()),
                    Row::Scale => (
                        "SCALE",
                        if settings.scale == 0 {
                            "WINDOW".to_owned()
                        } else {
                            format!("{}X", settings.scale)
                        },
                    ),
                    Row::Fullscreen => ("FULLSCREEN", on_off(settings.fullscreen).into()),
                    Row::Overscan => (
                        "OVERSCAN",
                        if settings.crop_overscan {
                            "CROPPED"
                        } else {
                            "FULL"
                        }
                        .into(),
                    ),
                    Row::Aspect => (
                        "ASPECT",
                        if settings.native_aspect {
                            "NATIVE"
                        } else {
                            "ORIGINAL"
                        }
                        .into(),
                    ),
                    Row::Volume => ("VOLUME", format!("{}/{}", settings.volume, MAX_VOLUME)),
                    Row::Mute => ("MUTE", on_off(settings.mute).into()),
                    Row::Video => ("VIDEO", ">".into()),
                    Row::Sound => ("SOUND", ">".into()),
                    Row::Controls => ("CONTROLS", ">".into()),
                    Row::Keyboard => ("KEYBOARD", ">".into()),
                    Row::Gamepad => ("GAMEPAD", ">".into()),
                    Row::Cheats => ("CHEATS", ">".into()),
                    Row::SaveStates => ("SAVE STATES", ">".into()),
                    Row::Reset => ("RESET", String::new()),
                    Row::Quit => ("QUIT", String::new()),
                    Row::Bind(control) => (control.label(), self.binding(*control, settings)),
                    Row::Defaults => ("RESET TO DEFAULTS", String::new()),
                    Row::InfiniteLives => (
                        "INFINITE LIVES",
                        on_off(settings.cheats.infinite_lives).into(),
                    ),
                    Row::InfiniteFuel => (
                        "INFINITE FUEL",
                        on_off(settings.cheats.infinite_fuel).into(),
                    ),
                    Row::FullGauge => (
                        "FULL WEAPON GAUGE",
                        on_off(settings.cheats.full_gauge).into(),
                    ),
                    Row::LevelSelect => (
                        "START AT LEVEL",
                        match self.level {
                            FINAL_LEVEL => "FINAL LEVEL".to_owned(),
                            n => format!("PLANET {n}"),
                        },
                    ),
                    Row::Slot => (
                        "SLOT",
                        format!(
                            "{} {}",
                            settings.slot + 1,
                            if filled[settings.slot.min(SLOTS - 1)] {
                                "(USED)"
                            } else {
                                "(EMPTY)"
                            }
                        ),
                    ),
                    Row::Save => ("SAVE", String::new()),
                    Row::Load => ("LOAD", String::new()),
                    Row::Back => ("BACK", String::new()),
                };
                Line {
                    label: label.into(),
                    value,
                    focused: index == self.focus,
                    dim: false,
                }
            })
            .collect();
        match self.page {
            Page::Gamepad => {
                lines.push(text_line("", "", true));
                lines.push(text_line("SELECT + START ALSO OPENS THE MENU", "", true));
            }
            Page::Cheats => {
                lines.push(text_line("", "", true));
                lines.push(text_line("LEVEL SELECT STARTS THE GAME", "", true));
                lines.push(text_line("AGAIN ON THAT LEVEL", "", true));
            }
            Page::SaveStates => {
                let save = controls::key_name(settings.keys.get(Control::QuickSave));
                let load = controls::key_name(settings.keys.get(Control::QuickLoad));
                lines.push(text_line("", "", true));
                lines.push(text_line(
                    &format!("{save} SAVES AND {load} LOADS"),
                    "",
                    true,
                ));
                lines.push(text_line("THE SELECTED SLOT", "", true));
                if !self.notice.is_empty() {
                    lines.push(text_line("", "", true));
                    lines.push(text_line(&self.notice, "", false));
                }
            }
            _ => {}
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl3::gamepad::Button;
    use sdl3::keyboard::Scancode;

    fn open() -> (Menu, Settings) {
        let mut menu = Menu::default();
        menu.toggle();
        (menu, Settings::default())
    }

    fn goto(menu: &mut Menu, settings: &mut Settings, row: Row) {
        while menu.page.rows()[menu.focus] != row {
            menu.handle(Nav::Down, settings);
        }
    }

    /// Opens the sub-page reached from the current page's `row`.
    fn enter(menu: &mut Menu, settings: &mut Settings, row: Row) {
        goto(menu, settings, row);
        menu.handle(Nav::Accept, settings);
    }

    #[test]
    fn vsync_toggles_and_reports_a_change() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Video);
        goto(&mut menu, &mut settings, Row::Vsync);
        let out = menu.handle(Nav::Right, &mut settings);
        assert!(out.settings_changed);
        assert!(!settings.vsync);
        menu.handle(Nav::Accept, &mut settings);
        assert!(settings.vsync);
    }

    #[test]
    fn physics_switches_between_remastered_and_original() {
        let (mut menu, mut settings) = open();
        assert_eq!(menu.page.rows()[menu.focus], Row::Physics);
        assert!(settings.remastered);
        assert_eq!(menu.lines(&settings, [false; SLOTS])[0].value, "REMASTERED");
        assert!(menu.handle(Nav::Accept, &mut settings).settings_changed);
        assert!(!settings.remastered);
        assert_eq!(menu.lines(&settings, [false; SLOTS])[0].value, "ORIGINAL");
        menu.handle(Nav::Left, &mut settings);
        assert!(settings.remastered);
    }

    #[test]
    fn cursor_wraps() {
        let (mut menu, mut settings) = open();
        menu.handle(Nav::Up, &mut settings);
        assert_eq!(menu.page.rows()[menu.focus], Row::Quit);
        menu.handle(Nav::Down, &mut settings);
        assert_eq!(menu.page.rows()[menu.focus], Row::Physics);
    }

    #[test]
    fn volume_and_scale_stay_in_range() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Sound);
        goto(&mut menu, &mut settings, Row::Volume);
        for _ in 0..20 {
            menu.handle(Nav::Left, &mut settings);
        }
        assert_eq!(settings.volume, 0);
        assert!(!menu.handle(Nav::Left, &mut settings).settings_changed);
        menu.handle(Nav::Back, &mut settings);
        enter(&mut menu, &mut settings, Row::Video);
        goto(&mut menu, &mut settings, Row::Scale);
        for _ in 0..=MAX_SCALE {
            menu.handle(Nav::Right, &mut settings);
        }
        assert_eq!(settings.scale, Settings::default().scale);
        menu.handle(Nav::Left, &mut settings);
        menu.handle(Nav::Left, &mut settings);
        menu.handle(Nav::Left, &mut settings);
        menu.handle(Nav::Left, &mut settings);
        assert_eq!(settings.scale, MAX_SCALE);
    }

    #[test]
    fn sub_pages_return_to_their_row_and_back_closes_and_quit() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Controls);
        enter(&mut menu, &mut settings, Row::Gamepad);
        assert_eq!(menu.page, Page::Gamepad);
        menu.handle(Nav::Back, &mut settings);
        assert_eq!(menu.page.rows()[menu.focus], Row::Gamepad);
        goto(&mut menu, &mut settings, Row::Back);
        menu.handle(Nav::Accept, &mut settings);
        assert_eq!(menu.page, Page::Main);
        assert_eq!(menu.page.rows()[menu.focus], Row::Controls);
        menu.handle(Nav::Back, &mut settings);
        assert!(!menu.is_open());
        menu.toggle();
        assert_eq!(menu.page.rows()[menu.focus], Row::Physics);
        goto(&mut menu, &mut settings, Row::Reset);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::Reset)
        );
        goto(&mut menu, &mut settings, Row::Quit);
        assert!(menu.handle(Nav::Accept, &mut settings).quit);
    }

    #[test]
    fn a_binding_waits_for_its_device_then_swaps() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Controls);
        enter(&mut menu, &mut settings, Row::Keyboard);
        assert_eq!(menu.listening(), None);
        goto(&mut menu, &mut settings, Row::Bind(Control::A));
        menu.handle(Nav::Accept, &mut settings);
        assert_eq!(menu.listening(), Some(Device::Keyboard));
        assert_eq!(
            menu.lines(&settings, [false; SLOTS])[0].value,
            "PRESS A KEY 5"
        );
        // Menu controls do nothing meanwhile.
        assert_eq!(menu.handle(Nav::Back, &mut settings), Outcome::default());
        menu.tick(Duration::from_millis(1500));
        assert_eq!(
            menu.lines(&settings, [false; SLOTS])[0].value,
            "PRESS A KEY 4"
        );
        let out = menu.bind(Pressed::Key(Scancode::Z), &mut settings);
        assert!(out.settings_changed);
        assert_eq!(settings.keys.get(Control::A), Some(Scancode::Z));
        assert_eq!(settings.keys.get(Control::B), Some(Scancode::X));
        assert_eq!(menu.listening(), None);
        assert_eq!(menu.lines(&settings, [false; SLOTS])[0].value, "Z");
    }

    #[test]
    fn a_binding_times_out_unchanged() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Controls);
        enter(&mut menu, &mut settings, Row::Gamepad);
        goto(&mut menu, &mut settings, Row::Bind(Control::Start));
        menu.handle(Nav::Accept, &mut settings);
        assert_eq!(menu.listening(), Some(Device::Gamepad));
        menu.tick(LISTEN_TIME);
        assert_eq!(menu.listening(), None);
        let out = menu.bind(Pressed::Button(Button::North), &mut settings);
        assert_eq!(out, Outcome::default());
        assert_eq!(settings.pad, Pad::default());
    }

    #[test]
    fn reset_restores_only_its_own_device() {
        let (mut menu, mut settings) = open();
        settings.keys.bind(Control::Up, Scancode::W);
        settings.pad.bind(Control::Up, Button::North);
        enter(&mut menu, &mut settings, Row::Controls);
        enter(&mut menu, &mut settings, Row::Gamepad);
        goto(&mut menu, &mut settings, Row::Defaults);
        assert!(menu.handle(Nav::Accept, &mut settings).settings_changed);
        assert_eq!(settings.pad, Pad::default());
        assert_eq!(settings.keys.get(Control::Up), Some(Scancode::W));
    }

    #[test]
    fn cheats_toggle_and_are_off_by_default() {
        let (mut menu, mut settings) = open();
        assert_eq!(
            settings.cheats,
            solar_jetman::game::gameplay::Cheats::default()
        );
        enter(&mut menu, &mut settings, Row::Cheats);
        for row in [Row::InfiniteLives, Row::InfiniteFuel, Row::FullGauge] {
            goto(&mut menu, &mut settings, row);
            assert!(menu.handle(Nav::Accept, &mut settings).settings_changed);
        }
        let on = solar_jetman::game::gameplay::Cheats {
            infinite_lives: true,
            infinite_fuel: true,
            full_gauge: true,
        };
        assert_eq!(settings.cheats, on);
        goto(&mut menu, &mut settings, Row::FullGauge);
        menu.handle(Nav::Left, &mut settings);
        assert!(!settings.cheats.full_gauge);
    }

    #[test]
    fn level_select_cycles_planets_and_final_and_starts_on_accept() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::Cheats);
        goto(&mut menu, &mut settings, Row::LevelSelect);
        let before = settings.clone();
        let out = menu.handle(Nav::Right, &mut settings);
        assert_eq!(out.action, None);
        for _ in 0..11 {
            menu.handle(Nav::Right, &mut settings);
        }
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::StartLevel(12))
        );
        menu.handle(Nav::Right, &mut settings);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::StartLevel(FINAL_LEVEL))
        );
        menu.handle(Nav::Right, &mut settings);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::StartLevel(0))
        );
        menu.handle(Nav::Left, &mut settings);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::StartLevel(FINAL_LEVEL))
        );
        assert_eq!(settings, before, "the choice is not a setting");
    }

    #[test]
    fn save_states_page_picks_a_slot_and_asks_for_save_and_load() {
        let (mut menu, mut settings) = open();
        enter(&mut menu, &mut settings, Row::SaveStates);
        menu.handle(Nav::Right, &mut settings);
        assert_eq!(settings.slot, 1);
        menu.handle(Nav::Left, &mut settings);
        menu.handle(Nav::Left, &mut settings);
        assert_eq!(settings.slot, SLOTS - 1);
        goto(&mut menu, &mut settings, Row::Save);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::Save(SLOTS - 1))
        );
        goto(&mut menu, &mut settings, Row::Load);
        assert_eq!(
            menu.handle(Nav::Accept, &mut settings).action,
            Some(Action::Load(SLOTS - 1))
        );
        let mut filled = [false; SLOTS];
        filled[SLOTS - 1] = true;
        assert!(menu.lines(&settings, filled)[0].value.contains("USED"));
        goto(&mut menu, &mut settings, Row::Back);
        menu.handle(Nav::Accept, &mut settings);
        assert!(menu.is_open() && menu.page == Page::Main);
    }
}
