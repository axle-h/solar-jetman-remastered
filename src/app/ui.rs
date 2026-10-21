//! Drawing the port menu and the message screens with the port's own font.

use crate::font::{PITCH_X, PITCH_Y, Rgb, Surface, text_width};
use crate::menu::Menu;
use crate::settings::{SLOTS, Settings};

const WHITE: Rgb = [236, 236, 236];
const GREY: Rgb = [120, 120, 130];
const GOLD: Rgb = [248, 200, 64];
const PANEL: Rgb = [16, 20, 48];
const MARGIN: usize = 16;
/// Where values start.
const VALUE_X: usize = 132;

/// Breaks `text` into lines of at most `columns` characters, at spaces.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word;
            while word.chars().count() > columns {
                let cut = word
                    .char_indices()
                    .nth(columns)
                    .map_or(word.len(), |(i, _)| i);
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                lines.push(word[..cut].to_owned());
                word = &word[cut..];
            }
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > columns {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

/// A full-screen message: a title and wrapped text.
pub fn draw_message(surface: &mut Surface, title: &str, body: &str) {
    surface.fill_rect(0, 0, surface.width, surface.height, PANEL);
    let columns = (surface.width - 2 * MARGIN) / PITCH_X;
    surface.text(MARGIN, MARGIN, title, GOLD);
    surface.fill_rect(MARGIN, MARGIN + 11, text_width(title), 1, GOLD);
    for (index, line) in wrap(body, columns).iter().enumerate() {
        surface.text(MARGIN, MARGIN + 24 + index * PITCH_Y, line, WHITE);
    }
}

/// The menu over the (dimmed) game.
pub fn draw_menu(surface: &mut Surface, menu: &Menu, settings: &Settings, filled: [bool; SLOTS]) {
    surface.dim();
    let title = wrap(menu.page_title(), (surface.width - 2 * MARGIN) / PITCH_X);
    for (index, line) in title.iter().enumerate() {
        surface.text(MARGIN, MARGIN + index * PITCH_Y, line, GOLD);
    }
    let top = MARGIN + (title.len() - 1) * PITCH_Y;
    surface.fill_rect(MARGIN, top + 11, surface.width - 2 * MARGIN, 1, GOLD);
    for (index, line) in menu.lines(settings, filled).iter().enumerate() {
        let y = top + 20 + index * PITCH_Y;
        let colour = if line.focused {
            WHITE
        } else if line.dim {
            GREY
        } else {
            [190, 190, 200]
        };
        if line.focused {
            surface.text(MARGIN - PITCH_X, y, ">", GOLD);
        }
        surface.text(MARGIN, y, &line.label, colour);
        surface.text(VALUE_X, y, &line.value, colour);
    }
    // In the controller's terms: the keys and buttons behind them are on the Controls pages.
    let hint = [
        "UP/DOWN: CHOOSE  LEFT/RIGHT: CHANGE",
        "A OR START: SELECT  B: BACK",
    ];
    for (index, text) in hint.iter().enumerate() {
        let y = surface.height - MARGIN - PITCH_Y * (hint.len() - index);
        surface.text(MARGIN, y, text, GREY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_respects_width_and_newlines() {
        let lines = wrap("one two three\nfour", 7);
        assert_eq!(lines, ["one two", "three", "four"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert!(wrap("a b c d e f g h", 5).iter().all(|l| l.len() <= 5));
    }
}
