//! What the keyboard's keys and a gamepad's buttons do: the NES controller's eight buttons and
//! the port's own five (menu, quick save and load, screenshot, quit). Every one can be rebound in the
//! port menu; the names shown are SDL's.

use sdl3::gamepad::Button;
use sdl3::keyboard::Scancode;
use solar_jetman::game::gameplay::input::Buttons;

/// A control that can be bound to a key and to a gamepad button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    A,
    B,
    Up,
    Down,
    Left,
    Right,
    Select,
    Start,
    Menu,
    QuickSave,
    QuickLoad,
    Screenshot,
    Quit,
}

const COUNT: usize = 13;

impl Control {
    pub const ALL: [Control; COUNT] = [
        Control::A,
        Control::B,
        Control::Up,
        Control::Down,
        Control::Left,
        Control::Right,
        Control::Select,
        Control::Start,
        Control::Menu,
        Control::QuickSave,
        Control::QuickLoad,
        Control::Screenshot,
        Control::Quit,
    ];

    /// The name in the port menu.
    pub fn label(self) -> &'static str {
        match self {
            Control::A => "A",
            Control::B => "B",
            Control::Up => "UP",
            Control::Down => "DOWN",
            Control::Left => "LEFT",
            Control::Right => "RIGHT",
            Control::Select => "SELECT",
            Control::Start => "START",
            Control::Menu => "PORT MENU",
            Control::QuickSave => "QUICK SAVE",
            Control::QuickLoad => "QUICK LOAD",
            Control::Screenshot => "SCREENSHOT",
            Control::Quit => "QUIT",
        }
    }

    /// The key in `settings.ini`.
    pub fn ini_key(self) -> &'static str {
        match self {
            Control::A => "a",
            Control::B => "b",
            Control::Up => "up",
            Control::Down => "down",
            Control::Left => "left",
            Control::Right => "right",
            Control::Select => "select",
            Control::Start => "start",
            Control::Menu => "menu",
            Control::QuickSave => "quick_save",
            Control::QuickLoad => "quick_load",
            Control::Screenshot => "screenshot",
            Control::Quit => "quit",
        }
    }

    /// The game's button, or 0 for the port's own controls.
    pub fn mask(self) -> u8 {
        match self {
            Control::A => Buttons::A,
            Control::B => Buttons::B,
            Control::Up => Buttons::UP,
            Control::Down => Buttons::DOWN,
            Control::Left => Buttons::LEFT,
            Control::Right => Buttons::RIGHT,
            Control::Select => Buttons::SELECT,
            Control::Start => Buttons::START,
            Control::Menu
            | Control::QuickSave
            | Control::QuickLoad
            | Control::Screenshot
            | Control::Quit => 0,
        }
    }
}

/// Where a binding lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Keyboard,
    Gamepad,
}

/// A key or button the port menu was waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    Key(Scancode),
    Button(Button),
}

/// One device's bindings, in the order of [`Control::ALL`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings<T>(pub [Option<T>; COUNT]);

impl<T: Copy + PartialEq> Bindings<T> {
    pub fn get(&self, control: Control) -> Option<T> {
        self.0[control as usize]
    }

    /// The control `input` is bound to.
    pub fn control(&self, input: T) -> Option<Control> {
        Control::ALL
            .into_iter()
            .find(|&control| self.get(control) == Some(input))
    }

    /// Binds `input` to `control`. A control that had `input` takes `control`'s old binding,
    /// so that no input does two things.
    pub fn bind(&mut self, control: Control, input: T) {
        if let Some(other) = self.control(input) {
            self.0[other as usize] = self.get(control);
        }
        self.0[control as usize] = Some(input);
    }
}

pub type Keys = Bindings<Scancode>;
pub type Pad = Bindings<Button>;

impl Default for Keys {
    /// Arrows, X for A (fire), Z for B (thrust), Backspace for Select, Enter for Start, and
    /// F1, F5, F9, F12 and Escape for the port. On Android, the system's back button is the
    /// port menu's key rather than F1.
    fn default() -> Self {
        Bindings([
            Some(Scancode::X),
            Some(Scancode::Z),
            Some(Scancode::Up),
            Some(Scancode::Down),
            Some(Scancode::Left),
            Some(Scancode::Right),
            Some(Scancode::Backspace),
            Some(Scancode::Return),
            Some(if cfg!(target_os = "android") {
                Scancode::AcBack
            } else {
                Scancode::F1
            }),
            Some(Scancode::F5),
            Some(Scancode::F9),
            Some(Scancode::F12),
            Some(Scancode::Escape),
        ])
    }
}

impl Default for Pad {
    /// SDL's A and B (the bottom and right face buttons), the d-pad, Back, Start and the guide
    /// button for the menu; the quick keys, screenshot and quit are unbound.
    fn default() -> Self {
        Bindings([
            Some(Button::South),
            Some(Button::East),
            Some(Button::DPadUp),
            Some(Button::DPadDown),
            Some(Button::DPadLeft),
            Some(Button::DPadRight),
            Some(Button::Back),
            Some(Button::Start),
            Some(Button::Guide),
            None,
            None,
            None,
            None,
        ])
    }
}

/// A key's name as SDL gives it ("Return", "Left Shift"), in capitals for the menu.
pub fn key_name(key: Option<Scancode>) -> String {
    match key.map(Scancode::name) {
        Some(name) if !name.is_empty() => name.to_ascii_uppercase(),
        Some(_) => "?".to_owned(),
        None => "NONE".to_owned(),
    }
}

/// A button's name from SDL's ("a", "dpup", "leftshoulder"), spelt out for the menu: "A",
/// "D-PAD UP", "LEFT SHOULDER".
pub fn button_name(button: Option<Button>) -> String {
    let Some(button) = button else {
        return "NONE".to_owned();
    };
    let name = button.string();
    for (prefix, words) in [
        ("dp", "D-PAD "),
        ("left", "LEFT "),
        ("right", "RIGHT "),
        ("misc", "MISC "),
        ("paddle", "PADDLE "),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return format!("{words}{}", rest.to_ascii_uppercase());
        }
    }
    name.to_ascii_uppercase()
}

/// The value written to `settings.ini` for a key: SDL's name, or `none`.
pub fn key_ini(key: Option<Scancode>) -> String {
    key.map_or_else(|| "none".to_owned(), |key| key.name().to_owned())
}

/// The value written to `settings.ini` for a button: SDL's name, or `none`.
pub fn button_ini(button: Option<Button>) -> String {
    button.map_or_else(|| "none".to_owned(), Button::string)
}

/// A key from `settings.ini`: `Some(None)` for `none`, `None` for a name SDL does not know.
pub fn parse_key(value: &str) -> Option<Option<Scancode>> {
    if value == "none" {
        Some(None)
    } else {
        Scancode::from_name(value).map(Some)
    }
}

/// A button from `settings.ini`, as [`parse_key`].
pub fn parse_button(value: &str) -> Option<Option<Button>> {
    if value == "none" {
        Some(None)
    } else {
        Button::from_string(value).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_a_used_input_swaps_the_two() {
        let mut keys = Keys::default();
        keys.bind(Control::A, Scancode::Z);
        assert_eq!(keys.get(Control::A), Some(Scancode::Z));
        assert_eq!(keys.get(Control::B), Some(Scancode::X));
        let mut pad = Pad::default();
        pad.bind(Control::Start, Button::LeftShoulder);
        pad.bind(Control::QuickSave, Button::LeftShoulder);
        assert_eq!(pad.get(Control::Start), None);
        assert_eq!(pad.control(Button::LeftShoulder), Some(Control::QuickSave));
    }

    #[test]
    fn names_are_sdls_in_capitals() {
        assert_eq!(key_name(Some(Scancode::Return)), "RETURN");
        assert_eq!(key_name(None), "NONE");
        assert_eq!(button_name(Some(Button::South)), "A");
        assert_eq!(button_name(Some(Button::DPadUp)), "D-PAD UP");
        assert_eq!(button_name(Some(Button::LeftShoulder)), "LEFT SHOULDER");
        assert_eq!(button_name(Some(Button::Guide)), "GUIDE");
    }

    #[test]
    fn ini_names_read_back() {
        for key in Keys::default().0 {
            assert_eq!(parse_key(&key_ini(key).to_ascii_lowercase()), Some(key));
        }
        for button in Pad::default().0 {
            assert_eq!(parse_button(&button_ini(button)), Some(button));
        }
        assert_eq!(parse_key("left shift"), Some(Some(Scancode::LShift)));
        assert_eq!(parse_key("no such key"), None);
    }
}
