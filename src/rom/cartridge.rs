//! Checking that a `.nes` file is the USA release (`docs/spec/rom-map.md`, section 1).

use std::fmt;

/// SHA-1 of the USA release's PRG ROM, without the 16-byte header.
pub const PRG_SHA1: &str = "caa4d1ab710bd766f8505ef24f5702dac6e988af";

/// Size of the PRG ROM: eight 32 KB banks.
pub const PRG_SIZE: usize = 8 * BANK_SIZE;

/// Size of one switchable bank, mapped at CPU $8000-$FFFF.
pub const BANK_SIZE: usize = 0x8000;

const HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 512;
const MAGIC: [u8; 4] = *b"NES\x1A";
const AXROM: u16 = 7;

/// Why a file is not a ROM we can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomError {
    /// Shorter than the 16-byte header.
    TooShort { len: usize },
    /// The file does not start with `NES` and $1A.
    NotNesFile,
    /// The header names another mapper than 7 (AxROM).
    WrongMapper { mapper: u16 },
    /// The header gives another PRG ROM size than 256 KB.
    WrongPrgSize { bytes: u64 },
    /// The header declares CHR ROM; the original has CHR RAM only.
    HasChrRom { bytes: u64 },
    /// The file ends before the PRG ROM the header announces.
    Truncated { needed: usize, len: usize },
    /// The PRG ROM is not the USA release (another version, a hack or a bad dump).
    WrongChecksum { sha1: String },
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::TooShort { len } => {
                write!(f, "file too short for a NES header ({len} bytes)")
            }
            RomError::NotNesFile => write!(f, "not a .nes file (no iNES/NES 2.0 header)"),
            RomError::WrongMapper { mapper } => write!(
                f,
                "wrong cartridge type: mapper {mapper}, expected {AXROM} (AxROM)"
            ),
            RomError::WrongPrgSize { bytes } => write!(
                f,
                "wrong PRG ROM size: {} KB, expected {} KB",
                bytes / 1024,
                PRG_SIZE / 1024
            ),
            RomError::HasChrRom { bytes } => write!(
                f,
                "the header declares {} KB of CHR ROM; the game uses CHR RAM",
                bytes / 1024
            ),
            RomError::Truncated { needed, len } => write!(
                f,
                "file truncated: {len} bytes, the header needs at least {needed}"
            ),
            RomError::WrongChecksum { sha1 } => write!(
                f,
                "not the USA release of Solar Jetman: PRG ROM SHA-1 is {sha1}, expected {PRG_SHA1}"
            ),
        }
    }
}

impl std::error::Error for RomError {}

/// The fields of an iNES or NES 2.0 header that the check needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NesHeader {
    pub nes2: bool,
    pub mapper: u16,
    pub prg_rom_bytes: u64,
    pub chr_rom_bytes: u64,
    pub has_trainer: bool,
}

impl NesHeader {
    /// Reads the header at the start of `file` (iNES or NES 2.0; nesdev.org "NES 2.0").
    pub fn parse(file: &[u8]) -> Result<NesHeader, RomError> {
        let Some(header) = file.get(..HEADER_SIZE) else {
            return Err(RomError::TooShort { len: file.len() });
        };
        if header[..4] != MAGIC {
            return Err(RomError::NotNesFile);
        }
        let nes2 = header[7] & 0x0C == 0x08;
        // Old dumping tools wrote text into bytes 7-15 of iNES headers; byte 7 is only
        // trusted when it is NES 2.0 or the tail of the header is clean.
        let byte7_valid = nes2 || header[12..16].iter().all(|&b| b == 0);
        let mut mapper = u16::from(header[6] >> 4);
        if byte7_valid {
            mapper |= u16::from(header[7] & 0xF0);
        }
        let (prg_rom_bytes, chr_rom_bytes) = if nes2 {
            mapper |= u16::from(header[8] & 0x0F) << 8;
            (
                nes2_rom_size(header[4], header[9] & 0x0F, 16 * 1024),
                nes2_rom_size(header[5], header[9] >> 4, 8 * 1024),
            )
        } else {
            (
                u64::from(header[4]) * 16 * 1024,
                u64::from(header[5]) * 8 * 1024,
            )
        };
        Ok(NesHeader {
            nes2,
            mapper,
            prg_rom_bytes,
            chr_rom_bytes,
            has_trainer: header[6] & 0x04 != 0,
        })
    }
}

/// A NES 2.0 ROM size: a 12-bit count of `unit`s, or, when the high nibble is $F, the
/// exponent-multiplier form `2^E × (2M + 1)` bytes.
fn nes2_rom_size(low: u8, high_nibble: u8, unit: u64) -> u64 {
    if high_nibble == 0x0F {
        let exponent = u32::from(low >> 2);
        let multiplier = u64::from(low & 3) * 2 + 1;
        1u64.checked_shl(exponent)
            .map_or(u64::MAX, |power| power.saturating_mul(multiplier))
    } else {
        (u64::from(high_nibble) << 8 | u64::from(low)) * unit
    }
}

/// The PRG ROM of a checked USA release.
#[derive(Clone)]
pub struct Rom {
    prg: Vec<u8>,
}

impl fmt::Debug for Rom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rom")
            .field("prg_len", &self.prg.len())
            .finish()
    }
}

impl Rom {
    /// Checks the bytes of a `.nes` file: the header must describe mapper 7 with 256 KB of PRG
    /// ROM and no CHR ROM, and the PRG ROM must hash to [`PRG_SHA1`].
    pub fn from_nes_file(file: &[u8]) -> Result<Rom, RomError> {
        let header = NesHeader::parse(file)?;
        if header.mapper != AXROM {
            return Err(RomError::WrongMapper {
                mapper: header.mapper,
            });
        }
        if header.prg_rom_bytes != PRG_SIZE as u64 {
            return Err(RomError::WrongPrgSize {
                bytes: header.prg_rom_bytes,
            });
        }
        if header.chr_rom_bytes != 0 {
            return Err(RomError::HasChrRom {
                bytes: header.chr_rom_bytes,
            });
        }
        let start = HEADER_SIZE + if header.has_trainer { TRAINER_SIZE } else { 0 };
        let needed = start + PRG_SIZE;
        let Some(prg) = file.get(start..needed) else {
            return Err(RomError::Truncated {
                needed,
                len: file.len(),
            });
        };
        let sha1 = sha1_hex(prg);
        if sha1 != PRG_SHA1 {
            return Err(RomError::WrongChecksum { sha1 });
        }
        Ok(Rom { prg: prg.to_vec() })
    }

    /// A ROM from PRG bytes that were not checked: for tests on synthetic data only.
    #[cfg(test)]
    pub(crate) fn unchecked(prg: Vec<u8>) -> Rom {
        assert_eq!(prg.len(), PRG_SIZE);
        Rom { prg }
    }

    /// The whole 256 KB PRG ROM.
    pub fn prg(&self) -> &[u8] {
        &self.prg
    }

    /// `len` bytes at PRG ROM offset `offset`, or `None` past the end.
    pub fn bytes(&self, offset: usize, len: usize) -> Option<&[u8]> {
        self.prg.get(offset..offset.checked_add(len)?)
    }

    /// The byte at PRG ROM offset `offset`, or `None` past the end.
    pub fn byte(&self, offset: usize) -> Option<u8> {
        self.prg.get(offset).copied()
    }
}

/// The PRG ROM offset of CPU address `address` ($8000-$FFFF) with `bank` mapped.
pub fn bank_offset(bank: u8, address: u16) -> usize {
    usize::from(bank & 7) * BANK_SIZE + usize::from(address.wrapping_sub(0x8000) & 0x7FFF)
}

/// Lower-case hexadecimal SHA-1 of `bytes`.
pub fn sha1_hex(bytes: &[u8]) -> String {
    sha1_smol::Sha1::from(bytes).digest().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(bytes: [u8; 16]) -> Vec<u8> {
        bytes.to_vec()
    }

    /// An NES 2.0 header for the USA release: mapper 7, 16 × 16 KB PRG, 8 KB CHR RAM.
    fn usa_like_header() -> [u8; 16] {
        let mut h = [0u8; 16];
        h[..4].copy_from_slice(b"NES\x1A");
        h[4] = 16;
        h[6] = 0x70;
        h[7] = 0x08;
        h[11] = 0x07;
        h
    }

    #[test]
    fn parses_nes2_header() {
        let parsed = NesHeader::parse(&header(usa_like_header())).unwrap();
        assert_eq!(
            parsed,
            NesHeader {
                nes2: true,
                mapper: 7,
                prg_rom_bytes: 256 * 1024,
                chr_rom_bytes: 0,
                has_trainer: false,
            }
        );
    }

    #[test]
    fn parses_ines_header_and_ignores_dirty_byte7() {
        let mut h = usa_like_header();
        h[7] = 0x40; // would make mapper $47
        h[12..16].copy_from_slice(b"Dude");
        let parsed = NesHeader::parse(&h).unwrap();
        assert!(!parsed.nes2);
        assert_eq!(parsed.mapper, 7);
        h[12..16].fill(0);
        assert_eq!(NesHeader::parse(&h).unwrap().mapper, 0x47);
    }

    #[test]
    fn nes2_sizes_and_mapper_high_bits() {
        let mut h = usa_like_header();
        h[8] = 0x01; // mapper bits 8-11
        h[9] = 0x21; // PRG MSB 1, CHR MSB 2
        h[5] = 3;
        let parsed = NesHeader::parse(&h).unwrap();
        assert_eq!(parsed.mapper, 0x107);
        assert_eq!(parsed.prg_rom_bytes, 0x110 * 16 * 1024);
        assert_eq!(parsed.chr_rom_bytes, 0x203 * 8 * 1024);
        // Exponent-multiplier form: 2^5 × 3 = 96 bytes.
        h[9] = 0x0F;
        h[4] = (5 << 2) | 1;
        assert_eq!(NesHeader::parse(&h).unwrap().prg_rom_bytes, 96);
    }

    #[test]
    fn rejects_bad_headers() {
        assert_eq!(
            Rom::from_nes_file(b"NES").unwrap_err(),
            RomError::TooShort { len: 3 }
        );
        assert_eq!(
            Rom::from_nes_file(&[0u8; 16]).unwrap_err(),
            RomError::NotNesFile
        );
        let mut h = usa_like_header();
        h[6] = 0x20;
        assert_eq!(
            Rom::from_nes_file(&h).unwrap_err(),
            RomError::WrongMapper { mapper: 2 }
        );
        let mut h = usa_like_header();
        h[4] = 8;
        assert_eq!(
            Rom::from_nes_file(&h).unwrap_err(),
            RomError::WrongPrgSize { bytes: 128 * 1024 }
        );
        let mut h = usa_like_header();
        h[5] = 1;
        assert_eq!(
            Rom::from_nes_file(&h).unwrap_err(),
            RomError::HasChrRom { bytes: 8 * 1024 }
        );
        assert_eq!(
            Rom::from_nes_file(&usa_like_header()).unwrap_err(),
            RomError::Truncated {
                needed: 16 + PRG_SIZE,
                len: 16
            }
        );
    }

    #[test]
    fn trainer_moves_prg_and_checksum_is_checked() {
        let mut file = usa_like_header().to_vec();
        file[6] |= 0x04;
        file.resize(16 + 512 + PRG_SIZE, 0);
        file[16 + 512] = 0x5A;
        let mut prg = vec![0u8; PRG_SIZE];
        prg[0] = 0x5A;
        let expected = sha1_hex(&prg);
        assert_eq!(
            Rom::from_nes_file(&file).unwrap_err(),
            RomError::WrongChecksum { sha1: expected }
        );
    }

    #[test]
    fn sha1_known_value() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn bank_offsets() {
        assert_eq!(bank_offset(0, 0x8000), 0);
        assert_eq!(bank_offset(2, 0xA2DE), 0x122DE);
        assert_eq!(bank_offset(7, 0xFFFF), 0x3FFFF);
    }

    #[test]
    fn errors_read_well() {
        let text = RomError::WrongMapper { mapper: 2 }.to_string();
        assert!(text.contains("mapper 2"), "{text}");
    }
}
