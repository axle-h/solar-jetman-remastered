//! The planet surface's sides in a wider frame (`docs/widescreen.md`, "Surface pictures"): cells
//! beside the picture made from the picture's own, copied or mirrored by a short recipe of ours
//! per picture. A cell keeps its tile and palette number, so the sides take each planet's
//! colours with the picture.

use super::ppu::{COLUMNS, ROWS, SideCell, SideCells};

/// Columns either side: a 16:9 frame shows 85½ pixels beside the picture.
pub const SIDE_COLUMNS: usize = 11;
/// The first row drawn from pattern table 0 (the split at line 126, with the scroll of 2).
const LOWER_FIRST_ROW: usize = 16;

/// One step of a recipe. Columns count from the picture's left edge: −11 to −1 on the left,
/// 32 to 42 on the right. Later steps draw over earlier ones.
enum Step {
    /// Every cell of `columns` × `rows` (first and count) takes the cell that is most common
    /// in its row of the picture: the sky, the ground, a band of the horizon.
    Common { columns: (i8, u8), rows: (u8, u8) },
    /// The picture's `size` cells from `from` to `to`; mirrored left to right when `mirror`.
    Copy {
        to: (i8, u8),
        from: (u8, u8),
        size: (u8, u8),
        mirror: bool,
    },
}

use Step::{Common, Copy};

const fn copy(to: (i8, u8), from: (u8, u8), size: (u8, u8)) -> Step {
    Copy {
        to,
        from,
        size,
        mirror: false,
    }
}

/// Picture 1 (planets 4 and 7): the ice floes. The bands of sky and distant ridges carry on.
/// The pair of floes at the picture's right comes again on the left, unmirrored to keep the
/// light from the upper left; the small floe of the pair, alone and further down the open ice
/// (rows 17 on are one plain fill), on the right.
const ICE_FLOES: &[Step] = &[
    Common {
        columns: (-11, 11),
        rows: (0, 30),
    },
    Common {
        columns: (32, 11),
        rows: (0, 30),
    },
    // The distant range carries on to the left with columns 0-7 again: column 8 is much like
    // column 0, so column 7 sits beside column 0 as it sits beside column 8.
    copy((-8, 10), (0, 10), (8, 6)),
    copy((-11, 10), (5, 10), (3, 6)),
    // The picture's right end again beyond it, as the original has it: the distant mountain
    // and ridges, the pair of floes in front, their drips; but not the big centre floe's
    // edge, in columns 21 and 22 from rows 15 and 16.
    copy((32, 10), (21, 10), (1, 5)),
    copy((33, 10), (22, 10), (1, 6)),
    copy((34, 10), (23, 10), (9, 14)),
    // Without the tiny shard of columns 0-1 and the top of the big left floe (column 4) in
    // row 15.
    Common {
        columns: (-8, 2),
        rows: (15, 1),
    },
    Common {
        columns: (-4, 1),
        rows: (15, 1),
    },
    // The small floe on the left, two rows lower, without the drips of its neighbour (column
    // 28, row 18).
    copy((-7, 20), (29, 18), (3, 5)),
    copy((-8, 21), (28, 19), (1, 4)),
    // In the picture, open ice where the pair stood.
    Common {
        columns: (25, 7),
        rows: (13, 3),
    },
    Common {
        columns: (24, 8),
        rows: (16, 8),
    },
];

/// The recipe of surface picture `picture`, if it has one yet.
fn recipe(picture: usize) -> Option<&'static [Step]> {
    match picture {
        1 => Some(ICE_FLOES),
        _ => None,
    }
}

/// The sides of surface picture `picture`, from its nametable `page` (30 rows of 32 tiles) and
/// its 64 attribute bytes, as loaded (before the menu box).
pub fn side_cells(picture: usize, page: &[Vec<u8>], attributes: &[u8]) -> Option<SideCells> {
    build(recipe(picture)?, page, attributes)
}

fn build(steps: &[Step], page: &[Vec<u8>], attributes: &[u8]) -> Option<SideCells> {
    let cell = |column: usize, row: usize, flip_x: bool| {
        let attribute = attributes.get((row / 4) * 8 + column / 4).copied()?;
        let shift = ((row / 2) & 1) * 4 + ((column / 2) & 1) * 2;
        Some(SideCell {
            tile: *page.get(row)?.get(column)?,
            palette: (attribute >> shift) & 3,
            table: u8::from(row < LOWER_FIRST_ROW),
            flip_x,
        })
    };
    let mut sides = SideCells::new(SIDE_COLUMNS);
    for step in steps {
        match *step {
            Common { columns, rows } => {
                for row in usize::from(rows.0)..usize::from(rows.0 + rows.1).min(ROWS) {
                    let common = most_common(page.get(row)?);
                    for column in columns.0..columns.0 + columns.1 as i8 {
                        sides.set(i32::from(column), row, cell(common, row, false));
                    }
                }
            }
            Copy {
                to,
                from,
                size,
                mirror,
            } => {
                for dy in 0..usize::from(size.1) {
                    for dx in 0..usize::from(size.0) {
                        let source = if mirror {
                            usize::from(from.0) + usize::from(size.0) - 1 - dx
                        } else {
                            usize::from(from.0) + dx
                        };
                        let column = i32::from(to.0) + dx as i32;
                        let row = usize::from(to.1) + dy;
                        let source_row = usize::from(from.1) + dy;
                        // A cell keeps its pattern table's half of the screen.
                        debug_assert_eq!(row < LOWER_FIRST_ROW, source_row < LOWER_FIRST_ROW);
                        sides.set(column, row, cell(source, source_row, mirror));
                    }
                }
            }
        }
    }
    Some(sides)
}

/// The column of the row's most common tile (the leftmost of a tie), column 0 left out: the
/// original never shows it.
fn most_common(row: &[u8]) -> usize {
    let count = |tile: u8| row[1..].iter().filter(|&&t| t == tile).count();
    (1..row.len().min(COLUMNS))
        .max_by_key(|&c| (count(row[c]), std::cmp::Reverse(c)))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_common_tile_of_a_row() {
        let mut row = vec![7u8; 32];
        row[0] = 9;
        row[3] = 9;
        row[5] = 4;
        assert_eq!(row[most_common(&row)], 7);
        assert_eq!(most_common(&row), 1);
    }

    #[test]
    fn copies_keep_tiles_palettes_and_tables() {
        let page: Vec<Vec<u8>> = (0..30)
            .map(|r| (0..32).map(|c| (r * 32 + c) as u8).collect())
            .collect();
        // Palette 2 everywhere.
        let attributes = vec![0xAA; 64];
        let sides = side_cells(1, &page, &attributes).unwrap();
        // The picture's right end again on the right: side (34, 13) is the picture's (23, 13),
        // table 1.
        let right = sides.cell(34, 13).unwrap();
        assert_eq!(
            (right.tile, right.palette, right.table),
            (page[13][23], 2, 1)
        );
        assert!(!right.flip_x);
        // The small floe on the left, moved two rows down: (−7, 20) is (29, 18), table 0.
        let left = sides.cell(-7, 20).unwrap();
        assert_eq!((left.tile, left.table), (page[18][29], 0));
        // The distant range carries on to the left with columns 0-7: (−1, 12) is (7, 12).
        assert_eq!(sides.cell(-1, 12).map(|c| c.tile), Some(page[12][7]));
        // Every side cell is set, none beyond them.
        assert!((0..30).all(|r| (-11..0).chain(32..43).all(|c| sides.cell(c, r).is_some())));
        assert!(sides.cell(-12, 0).is_none() && sides.cell(43, 0).is_none());
        // In the picture the pair of floes gives way to each row's common cell, beside the
        // distant mountain (column 24 kept in rows 13-15) and below it; the rest is the page.
        let common = |row: usize| page[row][most_common(&page[row])];
        assert_eq!(sides.cell(25, 13).map(|c| c.tile), Some(common(13)));
        assert_eq!(sides.cell(24, 16).map(|c| c.tile), Some(common(16)));
        assert_eq!(sides.cell(31, 23).map(|c| c.tile), Some(common(23)));
        assert!(sides.cell(24, 15).is_none() && sides.cell(23, 20).is_none());
        assert!(sides.cell(25, 12).is_none() && sides.cell(25, 24).is_none());
        assert!(side_cells(3, &page, &attributes).is_none());
    }

    #[test]
    fn no_recipe_replaces_the_cells_of_the_menu_box() {
        let page = vec![vec![0u8; 32]; 30];
        for picture in 0..7 {
            let Some(sides) = side_cells(picture, &page, &[0; 64]) else {
                continue;
            };
            for row in 4..16 {
                for column in 10..22 {
                    assert!(sides.cell(column, row).is_none(), "picture {picture}");
                }
            }
        }
    }

    #[test]
    fn a_mirrored_copy_reverses_the_columns_and_flips_each_cell() {
        let page: Vec<Vec<u8>> = (0..30)
            .map(|r| (0..32).map(|c| (r * 32 + c) as u8).collect())
            .collect();
        let step = Copy {
            to: (-3, 20),
            from: (1, 20),
            size: (3, 1),
            mirror: true,
        };
        let sides = build(&[step], &page, &[0; 64]).unwrap();
        for (column, source) in [(-3, 3), (-2, 2), (-1, 1)] {
            let cell = sides.cell(column, 20).unwrap();
            assert_eq!((cell.tile, cell.flip_x), (page[20][source], true));
        }
        assert!(sides.cell(-4, 20).is_none() && sides.cell(-1, 19).is_none());
    }
}
