//! Listening to the songs and effects from the viewer: the next or previous song, the next or
//! previous effect (played on the channels the game uses), and mute.

use super::data::SONG_COUNT;
use super::{EFFECTS, EffectUse, Sound, song_description};

/// What the viewer has picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Jukebox {
    song: usize,
    /// Index into [`EFFECTS`], or `None` before the first pick.
    effect: Option<usize>,
    muted: bool,
}

impl Jukebox {
    pub fn song(&self) -> usize {
        self.song
    }

    pub fn effect(&self) -> Option<&'static EffectUse> {
        self.effect.map(|index| &EFFECTS[index])
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Moves to another song (wrapping around) and starts it.
    pub fn change_song(&mut self, sound: &mut Sound, delta: isize) {
        self.song = (self.song as isize + delta).rem_euclid(SONG_COUNT as isize) as usize;
        sound.play_song(self.song);
    }

    /// Moves to another effect (wrapping around; the first pick of +1 is the first effect) and
    /// requests it as the game does.
    pub fn change_effect(&mut self, sound: &mut Sound, delta: isize) {
        let count = EFFECTS.len() as isize;
        let index = match self.effect {
            None if delta > 0 => 0,
            None => count - 1,
            Some(index) => (index as isize + delta).rem_euclid(count),
        } as usize;
        self.effect = Some(index);
        sound.request_effect_as_the_game_does(EFFECTS[index].id);
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        self.muted
    }

    /// A line for the window title.
    pub fn describe(&self) -> String {
        let mut text = format!("song {} ({})", self.song, song_description(self.song));
        if let Some(effect) = self.effect() {
            text.push_str(&format!(
                ", effect ${:02X} ({})",
                effect.id, effect.description
            ));
        }
        if self.muted {
            text.push_str(", muted");
        }
        text
    }
}
