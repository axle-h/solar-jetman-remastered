//! The releases of the game we accept, and where each holds what the USA release holds. The
//! catalogue, the extraction and the sound import name every location as the USA release's
//! PRG ROM offset or bank address (`docs/spec/rom-map.md`); another release keeps the same data
//! in stretches moved by a fixed number of bytes, listed here.
//!
//! The Europe (PAL) release was matched against the USA release byte for byte with our own
//! tools. Banks 2, 3 and 5-7 hold their data where the USA release does; in banks 0, 1 and 4
//! the program changed size, so the data after it moved. Every stretch below holds bytes equal
//! to the USA release's, except that pointer tables hold pointers moved with their targets
//! (`tests/release.rs` checks each location the import reads against both ROMs).

use crate::rom::cartridge::BANK_SIZE;

/// SHA-1 of the USA release's PRG ROM, without the 16-byte header.
pub const PRG_SHA1: &str = "caa4d1ab710bd766f8505ef24f5702dac6e988af";

/// SHA-1 of the Europe release's PRG ROM, without the 16-byte header.
pub const EUROPE_PRG_SHA1: &str = "9fe6fed4e8dec3df34c69c6d4b2e528a288246d2";

/// A release of the game, told apart by the SHA-1 of its PRG ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Release {
    /// *Solar Jetman: Hunt for the Golden Warpship* (USA), the release the spec describes.
    Usa,
    /// The Europe (PAL) release.
    Europe,
}

/// USA PRG ROM offsets `from..to` are at `offset + by` in another release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub from: usize,
    pub to: usize,
    pub by: isize,
}

const fn moved(from: usize, to: usize, by: isize) -> Moved {
    Moved { from, to, by }
}

/// Where the Europe release moved the data the import reads, in USA PRG ROM offsets. Offsets
/// outside these stretches are the same in both releases.
#[rustfmt::skip]
const EUROPE: &[Moved] = &[
    // Bank 0: the title colours inside the program ($8182-$8385).
    moved(0x00182, 0x00386, -8),
    // Bank 0: the flight's window colours ($844C-$89EA).
    moved(0x0044C, 0x009EB, 168),
    // Bank 0: gauge rows, star masks, warp mesh tables ($95FF-$AC29).
    moved(0x015FF, 0x02C2A, 180),
    // Bank 0: the analysis screen's colour pairs ($D380-$E1C7).
    moved(0x05380, 0x061C8, 1510),
    // Bank 0: palette map pointers and maps, RAM effect template, level and preset colours
    // ($E1D5-$F71D).
    moved(0x061D5, 0x0771E, 1547),
    // Bank 0: the palette preset tables ($F71E-$F78D). The Europe release has two presets
    // more, after the USA release's 28, so each table grows by two entries.
    moved(0x0771E, 0x07756, 1562),
    moved(0x07756, 0x07772, 1566),
    moved(0x07772, 0x0778E, 1568),
    // Bank 0: the title logo fade-in colours and the title screen tables ($F78E-$F84C).
    moved(0x0778E, 0x0784D, 1570),
    // Bank 1: the flight's message, sprite layouts and their tables, sprite set lists and
    // records, sprite tile graphics ($9E36-$FF48).
    moved(0x09E36, 0x0FF49, -4),
    // Bank 4: note periods, sound pointer tables, music and effect data ($A5A9-$FE54).
    moved(0x225A9, 0x27E55, 7),
];

impl Release {
    /// Every release we accept.
    pub const ALL: [Release; 2] = [Release::Usa, Release::Europe];

    /// The release whose PRG ROM has this SHA-1 (lower-case hexadecimal), if any.
    pub fn from_prg_sha1(sha1: &str) -> Option<Release> {
        Release::ALL
            .into_iter()
            .find(|release| release.prg_sha1() == sha1)
    }

    pub fn prg_sha1(self) -> &'static str {
        match self {
            Release::Usa => PRG_SHA1,
            Release::Europe => EUROPE_PRG_SHA1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Release::Usa => "USA",
            Release::Europe => "Europe",
        }
    }

    /// The stretches this release moved, in USA offsets.
    pub fn moves(self) -> &'static [Moved] {
        match self {
            Release::Usa => &[],
            Release::Europe => EUROPE,
        }
    }

    /// This release's PRG ROM offset of what the USA release holds at offset `usa`.
    pub fn locate(self, usa: usize) -> usize {
        self.moves()
            .iter()
            .find(|m| (m.from..m.to).contains(&usa))
            .map_or(usa, |m| usa.wrapping_add_signed(m.by))
    }

    /// [`Release::locate`] for the exclusive end of a region.
    pub fn locate_end(self, usa_end: usize) -> usize {
        self.locate(usa_end - 1) + 1
    }

    /// The reverse of [`Release::locate`]: the USA offset of what this release holds at
    /// `offset`, or `None` where the USA release holds nothing of it (bytes the move took away).
    pub fn usa_offset(self, offset: usize) -> Option<usize> {
        let moves = self.moves();
        if let Some(m) = moves.iter().find(|m| {
            let to = m.from.wrapping_add_signed(m.by);
            (to..to + (m.to - m.from)).contains(&offset)
        }) {
            return Some(offset.wrapping_add_signed(-m.by));
        }
        (!moves.iter().any(|m| (m.from..m.to).contains(&offset))).then_some(offset)
    }

    /// [`Release::locate`] for a CPU address with `bank` mapped. Moves never leave their bank.
    pub fn locate_address(self, bank: u8, address: u16) -> u16 {
        to_address(self.locate(crate::rom::cartridge::bank_offset(bank, address)))
    }

    /// [`Release::usa_offset`] for a CPU address with `bank` mapped.
    pub fn usa_address(self, bank: u8, address: u16) -> Option<u16> {
        self.usa_offset(crate::rom::cartridge::bank_offset(bank, address))
            .map(to_address)
    }
}

/// The CPU address of a PRG ROM offset, with its bank mapped.
fn to_address(offset: usize) -> u16 {
    0x8000 + (offset % BANK_SIZE) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_by_checksum() {
        for release in Release::ALL {
            assert_eq!(Release::from_prg_sha1(release.prg_sha1()), Some(release));
        }
        assert_eq!(Release::from_prg_sha1("00"), None);
    }

    #[test]
    fn moves_stay_in_their_bank_and_do_not_overlap() {
        for release in Release::ALL {
            let moves = release.moves();
            for (i, m) in moves.iter().enumerate() {
                assert!(m.from < m.to);
                let to = m.from.wrapping_add_signed(m.by);
                assert_eq!(m.from / BANK_SIZE, (m.to - 1) / BANK_SIZE, "{m:?}");
                assert_eq!(to / BANK_SIZE, m.from / BANK_SIZE, "{m:?}");
                assert_eq!((to + m.to - m.from - 1) / BANK_SIZE, m.from / BANK_SIZE);
                for other in &moves[i + 1..] {
                    assert!(m.to <= other.from, "{m:?} and {other:?} are not in order");
                    let other_to = other.from.wrapping_add_signed(other.by);
                    assert!(
                        to + (m.to - m.from) <= other_to,
                        "{m:?} and {other:?} overlap"
                    );
                }
            }
        }
    }

    #[test]
    fn locating_and_back() {
        let europe = Release::Europe;
        assert_eq!(Release::Usa.locate(0x0E045), 0x0E045);
        assert_eq!(europe.locate(0x0E045), 0x0E041);
        assert_eq!(europe.locate(0x12B99), 0x12B99);
        assert_eq!(europe.locate_end(0x27E4C), 0x27E53);
        assert_eq!(europe.locate_address(4, 0xA907), 0xA90E);
        assert_eq!(europe.usa_address(1, 0xCB41), Some(0xCB45));
        assert_eq!(europe.usa_offset(0x0E041), Some(0x0E045));
        assert_eq!(europe.usa_offset(0x12B99), Some(0x12B99));
        // The USA release's bytes here went 4 bytes down; what is here now is not theirs.
        assert_eq!(europe.usa_offset(0x0FF46), None);
        for m in europe.moves() {
            for usa in [m.from, m.to - 1] {
                assert_eq!(europe.usa_offset(europe.locate(usa)), Some(usa));
            }
        }
    }
}
