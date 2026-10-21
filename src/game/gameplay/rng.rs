//! The random number generator (`docs/spec/game-flow.md` 5).

/// Output byte and the 16-bit shift register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Rng {
    /// RAM $5A, the output byte.
    pub out: u8,
    /// RAM $5B, the register's high byte.
    pub high: u8,
    /// RAM $5C, the register's low byte.
    pub low: u8,
}

impl Rng {
    /// One step. `inputs` are, in order, the controller bytes $18, $19, $1A, $1B and the
    /// counters $5D, $5E, $5F, $60 as they are at that moment.
    pub fn step(&mut self, inputs: [u8; 8]) {
        let feedback = ((self.out >> 3) ^ (self.out >> 6)) & 1;
        let mut carry = u16::from(self.high >> 7);
        let register = (u16::from(self.high) << 8 | u16::from(self.low)) << 1 | u16::from(feedback);
        self.high = (register >> 8) as u8;
        self.low = register as u8;
        let mut sum = u16::from(self.out.wrapping_add(1));
        for value in inputs.into_iter().chain([13]) {
            sum += u16::from(value) + carry;
            carry = sum >> 8;
            sum &= 0xFF;
        }
        self.out = sum as u8;
    }
}

impl Rng {
    /// Two right rotations of the output byte for one impact spark (player.md 5.5), the carry
    /// entering the first being `carry`. Returns the spark's directions: left is the bit the
    /// second rotation shifts out, down is bit 0 of the result. Spec gap: player.md 5.5 only
    /// says the direction bits come from the rotations; this assignment fits all 33 sparks of
    /// four replays.
    pub fn rotate_for_directions(&mut self, mut carry: u8) -> (bool, bool) {
        for _ in 0..2 {
            let out = self.out & 1;
            self.out = self.out >> 1 | carry << 7;
            carry = out;
        }
        (carry == 1, self.out & 1 == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two steps seen in the original at the start of level 0 (inputs: none, counter 0, 1,
    /// rotating job 1, 2).
    #[test]
    fn matches_the_original_at_level_start() {
        let mut rng = Rng {
            out: 0x78,
            high: 0xC9,
            low: 0x03,
        };
        rng.step([0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(
            rng,
            Rng {
                out: 0x88,
                high: 0x92,
                low: 0x06
            }
        );
        rng.step([0, 0, 0, 0, 1, 0, 0, 2]);
        assert_eq!(
            rng,
            Rng {
                out: 0x9A,
                high: 0x24,
                low: 0x0D
            }
        );
    }
}
