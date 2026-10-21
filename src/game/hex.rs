//! Byte data in RON files as rows of hexadecimal, e.g. `["0F 21 31 30", "0F 16 27 37"]`, so a
//! nametable or a map reads as a grid of numbers rather than a column of a thousand numbers.
//! [`HexRows`] is a byte string cut into rows of a fixed width; [`Grid`] is a two-dimensional
//! grid whose width is that of its rows.

use serde::de::{Deserializer, Error as _, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Write as _;
use std::ops::Deref;

/// Bytes written as rows of `W` hexadecimal bytes. Reading accepts any row length.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HexRows<const W: usize>(pub Vec<u8>);

impl<const W: usize> Deref for HexRows<W> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl<const W: usize> From<Vec<u8>> for HexRows<W> {
    fn from(bytes: Vec<u8>) -> Self {
        HexRows(bytes)
    }
}

impl<const W: usize> Serialize for HexRows<W> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let rows = self.0.chunks(W.max(1));
        let mut seq = serializer.serialize_seq(Some(rows.len()))?;
        for row in rows {
            let mut text = String::with_capacity(row.len() * 3);
            for (i, byte) in row.iter().enumerate() {
                if i > 0 {
                    text.push(' ');
                }
                write!(text, "{byte:02X}").expect("writing to a String");
            }
            seq.serialize_element(&text)?;
        }
        seq.end()
    }
}

impl<'de, const W: usize> Deserialize<'de> for HexRows<W> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowsVisitor;
        impl<'de> Visitor<'de> for RowsVisitor {
            type Value = Vec<u8>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of strings of hexadecimal bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
                let mut bytes = Vec::new();
                while let Some(row) = seq.next_element::<String>()? {
                    for word in row.split_whitespace() {
                        let byte = u8::from_str_radix(word, 16).map_err(|_| {
                            A::Error::custom(format!("`{word}` is not a hexadecimal byte"))
                        })?;
                        bytes.push(byte);
                    }
                }
                Ok(bytes)
            }
        }
        deserializer.deserialize_seq(RowsVisitor).map(HexRows)
    }
}

/// How the cells of a [`Grid`] are written.
pub trait CellStyle {
    /// Writes one row of cells.
    fn write_row(row: &[u8]) -> String;
    /// Reads one row back.
    fn read_row(text: &str) -> Result<Vec<u8>, String>;
}

/// Cells written as two-digit hexadecimal bytes separated by spaces: `"00 1F 2A"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hex;

/// Cells 0-9 written as single digits with no separator: `"0023"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Digits;

impl CellStyle for Hex {
    fn write_row(row: &[u8]) -> String {
        let mut text = String::with_capacity(row.len() * 3);
        for (i, byte) in row.iter().enumerate() {
            if i > 0 {
                text.push(' ');
            }
            write!(text, "{byte:02X}").expect("writing to a String");
        }
        text
    }

    fn read_row(text: &str) -> Result<Vec<u8>, String> {
        text.split_whitespace()
            .map(|word| {
                u8::from_str_radix(word, 16)
                    .map_err(|_| format!("`{word}` is not a hexadecimal byte"))
            })
            .collect()
    }
}

impl CellStyle for Digits {
    fn write_row(row: &[u8]) -> String {
        row.iter()
            .map(|&cell| char::from_digit(u32::from(cell), 10).unwrap_or('?'))
            .collect()
    }

    fn read_row(text: &str) -> Result<Vec<u8>, String> {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| {
                c.to_digit(10)
                    .map(|digit| digit as u8)
                    .ok_or_else(|| format!("`{c}` is not a digit"))
            })
            .collect()
    }
}

/// A grid of byte cells, row by row, written as one string per row. Every row has the same
/// width; reading checks that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid<S> {
    width: usize,
    cells: Vec<u8>,
    style: std::marker::PhantomData<S>,
}

/// A grid of hexadecimal bytes.
pub type HexGrid = Grid<Hex>;
/// A grid of single digits.
pub type DigitGrid = Grid<Digits>;

impl<S> Grid<S> {
    /// A grid from its cells, row by row. Panics if `cells` is not a whole number of rows.
    pub fn new(width: usize, cells: Vec<u8>) -> Self {
        assert!(
            width > 0 && cells.len().is_multiple_of(width),
            "{} cells do not make rows of {width}",
            cells.len()
        );
        Grid {
            width,
            cells,
            style: std::marker::PhantomData,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.cells.len() / self.width
    }

    /// The cell at column `x`, row `y`, or `None` outside the grid.
    pub fn get(&self, x: usize, y: usize) -> Option<u8> {
        if x < self.width {
            self.cells.get(y * self.width + x).copied()
        } else {
            None
        }
    }

    /// All cells, row by row.
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }
}

impl<S: CellStyle> Serialize for Grid<S> {
    fn serialize<Z: Serializer>(&self, serializer: Z) -> Result<Z::Ok, Z::Error> {
        let rows = self.cells.chunks(self.width.max(1));
        let mut seq = serializer.serialize_seq(Some(rows.len()))?;
        for row in rows {
            seq.serialize_element(&S::write_row(row))?;
        }
        seq.end()
    }
}

impl<'de, S: CellStyle> Deserialize<'de> for Grid<S> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rows = Vec::<String>::deserialize(deserializer)?;
        let mut width = None;
        let mut cells = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let row = S::read_row(row).map_err(D::Error::custom)?;
            let expected = *width.get_or_insert(row.len());
            if row.len() != expected || expected == 0 {
                return Err(D::Error::custom(format!(
                    "grid row {index} has {} cells, expected {expected} (and at least one)",
                    row.len()
                )));
            }
            cells.extend(row);
        }
        let Some(width) = width else {
            return Err(D::Error::custom("a grid needs at least one row"));
        };
        Ok(Grid::new(width, cells))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_rows_and_reads_back() {
        let data = HexRows::<4>((0..10).map(|i| i * 17).collect());
        let text = ron::to_string(&data).unwrap();
        assert_eq!(text, r#"["00 11 22 33","44 55 66 77","88 99"]"#);
        let back: HexRows<4> = ron::from_str(&text).unwrap();
        assert_eq!(back, data);
        let other: HexRows<2> = ron::from_str(r#"["0f  A0", "1"]"#).unwrap();
        assert_eq!(other.0, vec![0x0F, 0xA0, 0x01]);
        assert!(ron::from_str::<HexRows<2>>(r#"["0G"]"#).is_err());
    }

    #[test]
    fn grids_round_trip() {
        let grid = HexGrid::new(3, vec![0, 0x1F, 0xA0, 1, 2, 3]);
        let text = ron::to_string(&grid).unwrap();
        assert_eq!(text, r#"["00 1F A0","01 02 03"]"#);
        let back: HexGrid = ron::from_str(&text).unwrap();
        assert_eq!(back, grid);
        assert_eq!((back.width(), back.height()), (3, 2));
        assert_eq!(back.get(1, 1), Some(2));
        assert_eq!(back.get(3, 0), None);
        assert_eq!(back.get(0, 2), None);

        let digits = DigitGrid::new(4, vec![0, 2, 3, 3, 1, 0, 0, 9]);
        let text = ron::to_string(&digits).unwrap();
        assert_eq!(text, r#"["0233","1009"]"#);
        assert_eq!(ron::from_str::<DigitGrid>(&text).unwrap(), digits);
    }

    #[test]
    fn ragged_or_bad_grids_are_rejected() {
        assert!(ron::from_str::<HexGrid>(r#"["00 01","02"]"#).is_err());
        assert!(ron::from_str::<HexGrid>(r#"[]"#).is_err());
        assert!(ron::from_str::<DigitGrid>(r#"["0a"]"#).is_err());
    }
}
