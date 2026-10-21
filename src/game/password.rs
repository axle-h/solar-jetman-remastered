//! Passwords: twelve letters that hold 48 bits of the game's state (`docs/spec/password.md`).

/// The sixteen letters, one per 4-bit value (section 1).
pub const ALPHABET: &[u8; 16] = b"BDGHKLMNPQRTVWXZ";

/// The part of the game a password keeps (sections 3 and 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Saved {
    /// Level index RAM $3B, 0-63.
    pub level: u8,
    /// Jetman lives, 0-15.
    pub lives: u8,
    /// The score as six decimal digits, most significant first.
    pub score: [u8; 6],
    /// The pod's equipment byte RAM $DC: only pod type (bits 0-1), boosters (bit 6) and probe
    /// shields (bit 7) are kept.
    pub equipment: u8,
    /// The mapping device and the super mapping device (RAM $060C, $060D).
    pub mapping_device: bool,
    pub super_mapping_device: bool,
}

impl Saved {
    /// The state of a new game: level 0, four lives, score 1000.
    pub const NEW_GAME: Saved = Saved {
        level: 0,
        lives: 4,
        score: [0, 0, 1, 0, 0, 0],
        equipment: 0,
        mapping_device: false,
        super_mapping_device: false,
    };

    /// The score as a number.
    pub fn score_value(&self) -> u32 {
        self.score.iter().fold(0, |n, &d| n * 10 + u32::from(d))
    }

    /// Sets the score digits from a number (at most 999999).
    pub fn with_score(mut self, mut score: u32) -> Saved {
        score = score.min(999_999);
        for digit in self.score.iter_mut().rev() {
            *digit = (score % 10) as u8;
            score /= 10;
        }
        self
    }

    /// The six bytes P0..P5 (sections 2-4).
    fn bytes(&self) -> [u8; 6] {
        let [s1, s2, s3, s4, s5, s6] = self.score.map(|d| d & 15);
        // S1..S6 are the score's digits, most significant first.
        let pod_field = (self.equipment & 3) << 2
            | u8::from(self.equipment & 0x80 != 0) << 1
            | u8::from(self.equipment & 0x40 != 0);
        let p0 = self.lives.min(15) << 4 | s3;
        let p1 = s6 << 4 | s2;
        let p2 = self.level << 2
            | u8::from(self.mapping_device) << 1
            | u8::from(self.super_mapping_device);
        let p4 = s4 << 4 | pod_field;
        let p5 = s5 << 4 | s1;
        [p0, p1, p2, checksum(p0, p1, p2, p4, p5), p4, p5]
    }

    /// The twelve letters.
    pub fn encode(&self) -> [u8; 12] {
        let bytes = self.bytes();
        let mut letters = [0u8; 12];
        for (i, &byte) in bytes.iter().enumerate() {
            letters[i] = ALPHABET[usize::from(byte >> 4)];
            letters[6 + i] = ALPHABET[usize::from(byte & 15)];
        }
        letters
    }

    /// Reads twelve letters, `None` if the password is invalid (section 6). The level index is
    /// returned as stored; the remake accepts only levels 0-12 ([`Saved::is_playable`]).
    pub fn decode(letters: &[u8; 12]) -> Option<Saved> {
        let mut nibbles = [0u8; 12];
        for (nibble, letter) in nibbles.iter_mut().zip(letters) {
            *nibble = ALPHABET.iter().position(|a| a == letter)? as u8;
        }
        let mut bytes = [0u8; 6];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = nibbles[i] << 4 | nibbles[6 + i];
        }
        let [p0, p1, p2, p3, p4, p5] = bytes;
        if p3 != checksum(p0, p1, p2, p4, p5) {
            return None;
        }
        // The digits S1..S6; all must be decimal.
        let digits = [p5 & 15, p1 & 15, p0 & 15, p4 >> 4, p5 >> 4, p1 >> 4];
        if digits.iter().any(|&d| d > 9) {
            return None;
        }
        let pod_field = p4 & 15;
        if pod_field >> 2 == 3 {
            return None;
        }
        Some(Saved {
            level: p2 >> 2,
            lives: p0 >> 4,
            score: digits,
            equipment: (pod_field >> 2) | (pod_field & 2) << 6 | (pod_field & 1) << 6,
            mapping_device: p2 & 2 != 0,
            super_mapping_device: p2 & 1 != 0,
        })
    }

    /// Whether the remake starts a game from this state: the level must be a planet
    /// (`docs/deviations.md`, "Passwords").
    pub fn is_playable(&self) -> bool {
        self.level <= 12
    }
}

/// The checksum byte P3 (section 4).
fn checksum(p0: u8, p1: u8, p2: u8, p4: u8, p5: u8) -> u8 {
    let t = u16::from(p0 ^ p1) + u16::from(p2);
    let carry = (t >> 8) as u8;
    ((t as u8) ^ p4).wrapping_add(p5).wrapping_add(carry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letters(text: &str) -> [u8; 12] {
        text.as_bytes().try_into().unwrap()
    }

    #[test]
    fn new_game() {
        assert_eq!(&Saved::NEW_GAME.encode(), b"KBBKBBDBBDBB");
    }

    #[test]
    fn worked_examples() {
        let level_5 = Saved {
            level: 5,
            lives: 7,
            equipment: 0x41,
            mapping_device: true,
            ..Saved::NEW_GAME
        }
        .with_score(12345);
        assert_eq!(&level_5.encode(), b"NLDKHKGDMVLB");
        assert_eq!(Saved::decode(&letters("NLDKHKGDMVLB")), Some(level_5));

        let level_3 = Saved {
            level: 3,
            lives: 12,
            equipment: 2,
            super_mapping_device: true,
            ..Saved::NEW_GAME
        }
        .with_score(999_999);
        assert_eq!(&level_3.encode(), b"VQBLQQQQWXPQ");
        assert_eq!(Saved::decode(&letters("VQBLQQQQWXPQ")), Some(level_3));

        // The first addition carries.
        let level_12 = Saved {
            level: 12,
            lives: 15,
            ..Saved::NEW_GAME
        }
        .with_score(0);
        assert_eq!(&level_12.encode(), b"ZBHGBBBBBDBB");
        assert_eq!(Saved::decode(&letters("ZBHGBBBBBDBB")), Some(level_12));
    }

    #[test]
    fn invalid_passwords() {
        // A wrong checksum, a letter outside the alphabet, a score digit above 9, pod type 3.
        assert_eq!(Saved::decode(&letters("VQBLQQQQWXPB")), None);
        assert_eq!(Saved::decode(&letters("AQBLQQQQWXPQ")), None);
        // A score digit of 10 with a matching checksum: S1 = 10 is P5's low nibble.
        let mut bytes = Saved::NEW_GAME.bytes();
        bytes[5] |= 0x0A;
        bytes[3] = checksum(bytes[0], bytes[1], bytes[2], bytes[4], bytes[5]);
        let mut text = [0u8; 12];
        for (i, &byte) in bytes.iter().enumerate() {
            text[i] = ALPHABET[usize::from(byte >> 4)];
            text[6 + i] = ALPHABET[usize::from(byte & 15)];
        }
        assert_eq!(Saved::decode(&text), None);
        let pod_3 = Saved {
            equipment: 3,
            ..Saved::NEW_GAME
        };
        assert_eq!(Saved::decode(&pod_3.encode()), None);
    }

    #[test]
    fn levels_above_twelve_are_not_playable() {
        let saved = Saved {
            level: 20,
            ..Saved::NEW_GAME
        };
        assert_eq!(Saved::decode(&saved.encode()), Some(saved));
        assert!(!saved.is_playable());
        assert!(Saved::NEW_GAME.is_playable());
    }
}
