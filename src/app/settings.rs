//! The port's settings and their file, `settings.ini`: `key = value` lines under `[section]`
//! headers. Unknown keys and bad values are ignored, so an edited or older file still loads.

use crate::controls::{self, Control, Keys, Pad};
use solar_jetman::game::gameplay::Cheats;

/// Highest volume step; steps run from 0 to this.
pub const MAX_VOLUME: u8 = 10;
/// Highest fixed window scale in the menu; 0 means "fit the window".
pub const MAX_SCALE: u32 = 6;
/// Save-state slots.
pub const SLOTS: usize = 3;
/// The level select's last choice: the final level (0-12 are the planets).
pub const FINAL_LEVEL: u8 = 13;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub vsync: bool,
    /// Window size as a whole multiple of the screen; 0 leaves the window as the player sized it.
    pub scale: u32,
    pub fullscreen: bool,
    /// Hide the picture's edges: the 8 lines at the top and bottom that a television's
    /// overscan hides, and the 8-pixel column at the left that the game blanks.
    pub crop_overscan: bool,
    /// The view takes the display's shape, wider than the original's (`docs/widescreen.md`);
    /// off, the original's 256 pixels.
    pub native_aspect: bool,
    pub volume: u8,
    pub mute: bool,
    /// The cheats, all off by default.
    pub cheats: Cheats,
    /// The save-state slot (0-2) of the menu's Save and Load and the quick keys.
    pub slot: usize,
    /// The physics for the levels started from now on: Remastered (the Scaled engine at the
    /// display's rate, with the pod rest) or the Original's 30 Hz step.
    pub remastered: bool,
    pub keys: Keys,
    pub pad: Pad,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            vsync: true,
            scale: 3,
            fullscreen: false,
            crop_overscan: true,
            native_aspect: false,
            volume: MAX_VOLUME,
            mute: false,
            cheats: Cheats::default(),
            slot: 0,
            remastered: true,
            keys: Keys::default(),
            pad: Pad::default(),
        }
    }
}

fn flag(value: &str) -> Option<bool> {
    match value {
        "1" | "on" | "true" | "yes" => Some(true),
        "0" | "off" | "false" | "no" => Some(false),
        _ => None,
    }
}

impl Settings {
    pub fn parse(text: &str) -> Settings {
        let mut settings = Settings::default();
        let mut section = "";
        for line in text.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim().to_ascii_lowercase());
            let control = Control::ALL.into_iter().find(|c| c.ini_key() == key);
            match (section, control) {
                ("keyboard", Some(control)) => {
                    if let Some(key) = controls::parse_key(&value) {
                        settings.keys.0[control as usize] = key;
                    }
                    continue;
                }
                ("gamepad", Some(control)) => {
                    if let Some(button) = controls::parse_button(&value) {
                        settings.pad.0[control as usize] = button;
                    }
                    continue;
                }
                _ => {}
            }
            match key {
                "vsync" => settings.vsync = flag(&value).unwrap_or(settings.vsync),
                "fullscreen" => settings.fullscreen = flag(&value).unwrap_or(settings.fullscreen),
                "crop_overscan" => {
                    settings.crop_overscan = flag(&value).unwrap_or(settings.crop_overscan);
                }
                "aspect" => match value.as_str() {
                    "native" => settings.native_aspect = true,
                    "original" => settings.native_aspect = false,
                    _ => {}
                },
                "mute" => settings.mute = flag(&value).unwrap_or(settings.mute),
                "physics" => match value.as_str() {
                    "remastered" => settings.remastered = true,
                    "original" => settings.remastered = false,
                    _ => {}
                },
                "infinite_lives" => {
                    settings.cheats.infinite_lives =
                        flag(&value).unwrap_or(settings.cheats.infinite_lives);
                }
                "infinite_fuel" => {
                    settings.cheats.infinite_fuel =
                        flag(&value).unwrap_or(settings.cheats.infinite_fuel);
                }
                "full_gauge" => {
                    settings.cheats.full_gauge = flag(&value).unwrap_or(settings.cheats.full_gauge);
                }
                "slot" => {
                    if let Ok(n) = value.parse::<usize>() {
                        settings.slot = n.min(SLOTS - 1);
                    }
                }
                "scale" => {
                    if let Ok(n) = value.parse::<u32>() {
                        settings.scale = n.min(MAX_SCALE);
                    }
                }
                "volume" => {
                    if let Ok(n) = value.parse::<u8>() {
                        settings.volume = n.min(MAX_VOLUME);
                    }
                }
                _ => {}
            }
        }
        settings
    }

    pub fn to_ini(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        let mut text = format!(
            "[game]\nphysics = {}\n\n\
             [video]\nvsync = {}\nscale = {}\nfullscreen = {}\ncrop_overscan = {}\naspect = {}\n\n\
             [sound]\nvolume = {}\nmute = {}\n\n[cheats]\ninfinite_lives = {}\n\
             infinite_fuel = {}\nfull_gauge = {}\n\n\
             [saves]\nslot = {}\n",
            if self.remastered {
                "remastered"
            } else {
                "original"
            },
            on(self.vsync),
            self.scale,
            on(self.fullscreen),
            on(self.crop_overscan),
            if self.native_aspect {
                "native"
            } else {
                "original"
            },
            self.volume,
            on(self.mute),
            on(self.cheats.infinite_lives),
            on(self.cheats.infinite_fuel),
            on(self.cheats.full_gauge),
            self.slot,
        );
        text.push_str("\n[keyboard]\n");
        for control in Control::ALL {
            let key = controls::key_ini(self.keys.get(control));
            text.push_str(&format!("{} = {key}\n", control.ini_key()));
        }
        text.push_str("\n[gamepad]\n");
        for control in Control::ALL {
            let button = controls::button_ini(self.pad.get(control));
            text.push_str(&format!("{} = {button}\n", control.ini_key()));
        }
        text
    }

    /// Output gain for the audio player: 0 when muted.
    pub fn gain(&self) -> f32 {
        if self.mute {
            0.0
        } else {
            solar_jetman::game::sound::player::DEFAULT_GAIN * f32::from(self.volume)
                / f32::from(MAX_VOLUME)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut settings = Settings {
            vsync: false,
            scale: 5,
            fullscreen: true,
            crop_overscan: false,
            native_aspect: true,
            volume: 4,
            mute: true,
            cheats: Cheats {
                infinite_lives: true,
                infinite_fuel: false,
                full_gauge: true,
            },
            slot: 2,
            remastered: false,
            keys: Keys::default(),
            pad: Pad::default(),
        };
        settings
            .keys
            .bind(Control::A, sdl3::keyboard::Scancode::LShift);
        settings
            .pad
            .bind(Control::QuickSave, sdl3::gamepad::Button::LeftShoulder);
        settings.pad.0[Control::Menu as usize] = None;
        assert_eq!(Settings::parse(&settings.to_ini()), settings);
    }

    #[test]
    fn bad_input_keeps_defaults() {
        let text = "; comment\nvsync = maybe\nscale = 99\nvolume = loud\nfoo = 1\n[x]\nmute = YES";
        let settings = Settings::parse(text);
        assert!(settings.vsync);
        assert_eq!(settings.scale, MAX_SCALE);
        assert_eq!(settings.volume, MAX_VOLUME);
        assert!(settings.mute);
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn cheats_start_off_and_an_old_level_select_line_is_ignored() {
        assert_eq!(Settings::default().cheats, Cheats::default());
        assert_eq!(Settings::parse("level_select = 7"), Settings::default());
        assert!(!Settings::default().to_ini().contains("level_select"));
        assert_eq!(Settings::parse("slot = 9").slot, SLOTS - 1);
    }

    #[test]
    fn bindings_belong_to_their_section() {
        let settings = Settings::parse("a = z\n[keyboard]\na = q\nb = nonsense\n[gamepad]\na = y");
        assert_eq!(
            settings.keys.get(Control::A),
            Some(sdl3::keyboard::Scancode::Q)
        );
        assert_eq!(
            settings.keys.get(Control::B),
            Keys::default().get(Control::B)
        );
        assert_eq!(
            settings.pad.get(Control::A),
            Some(sdl3::gamepad::Button::North)
        );
    }

    #[test]
    fn gain_follows_volume_and_mute() {
        let mut settings = Settings::default();
        assert!(settings.gain() > 0.0);
        settings.volume = 0;
        assert_eq!(settings.gain(), 0.0);
        settings.volume = 5;
        settings.mute = true;
        assert_eq!(settings.gain(), 0.0);
    }
}
