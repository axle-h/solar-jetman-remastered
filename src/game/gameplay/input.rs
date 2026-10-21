//! The controller bytes (`docs/spec/game-flow.md` 3.1).

/// Buttons held, as the original's controller byte: A bit 7, B bit 6, Select bit 5, Start bit 4,
/// Up bit 3, Down bit 2, Left bit 1, Right bit 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const A: u8 = 0x80;
    pub const B: u8 = 0x40;
    pub const SELECT: u8 = 0x20;
    pub const START: u8 = 0x10;
    pub const UP: u8 = 0x08;
    pub const DOWN: u8 = 0x04;
    pub const LEFT: u8 = 0x02;
    pub const RIGHT: u8 = 0x01;

    pub fn has(self, mask: u8) -> bool {
        self.0 & mask != 0
    }

    /// The buttons newly pressed since `before`: held now and not then.
    pub fn pressed_since(self, before: Buttons) -> Buttons {
        Buttons(self.0 & !before.0)
    }
}
