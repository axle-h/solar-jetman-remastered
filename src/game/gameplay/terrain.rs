//! The 3 × 3 block window of terrain surfaces around the player (`docs/spec/levels.md` 6.4,
//! step 1): segments collected per direction, in window coordinates.

use crate::game::level::{BLOCK_SIZE, CHUNK_BLOCKS, Level, Line, World};

/// A surface in window coordinates (pixels from the top-left corner of the 3 × 3 blocks),
/// with the fields the spec names for its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Surface {
    /// Directions 0 and 9: y from x0 to x0 + width.
    Flat { x0: i32, width: i32, y: i32 },
    /// Directions 7 and 8: x from y0 to y0 + height.
    Wall { x: i32, y0: i32, height: i32 },
    /// Directions 1-6: `y_low` is the first stored byte, `y_high` the last.
    Floor {
        y_low: i32,
        x0: i32,
        width: i32,
        y_high: i32,
    },
    /// Directions 10-15.
    Ceiling {
        y_top: i32,
        x0: i32,
        width: i32,
        y_bottom: i32,
    },
}

impl Surface {
    /// Height of a sloped surface's line at window column `column` for `direction`
    /// (levels.md 6.2); `t` is the column's distance from x0.
    pub fn line_height(&self, direction: u8, column: i32) -> i32 {
        match *self {
            Surface::Floor {
                y_low, x0, y_high, ..
            } => {
                let t = column - x0;
                match direction {
                    1 => y_high + t / 2,
                    2 => y_low - t / 2,
                    3 => y_high + t,
                    4 => y_low - t,
                    5 => y_high + 2 * t,
                    _ => y_low - 2 * t,
                }
            }
            Surface::Ceiling {
                y_top,
                x0,
                y_bottom,
                ..
            } => {
                let t = column - x0;
                match direction {
                    10 => y_bottom - t,
                    11 => y_top + t,
                    12 => y_bottom - 2 * t,
                    13 => y_top + 2 * t,
                    14 => y_bottom - t / 2,
                    _ => y_top + t / 2,
                }
            }
            _ => 0,
        }
    }
}

/// The surfaces of the window, one list per direction (0-15), in collection order.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Window {
    pub lists: [Vec<Surface>; 16],
}

/// A segment coordinate moved by the block's offset, as an unsigned byte (levels.md 6.4: a
/// coordinate below 0 wraps to 240-255).
fn wrap(coordinate: i8, offset: i32) -> i32 {
    i32::from((coordinate as u8).wrapping_add(offset as u8))
}

/// Whether the chunk RAM $D4 holds is 0, the open sky (levels.md 6.5, graphics.md 5.7.3), for a
/// player at (`x`, `y`): the chunk of the window's last collected block, its bottom-right one
/// (graphics.md 5.7.3).
pub fn window_in_sky(world: &World, level: &Level, x: u16, y: u16) -> bool {
    // The walk starts at the block of x - 16 (of x itself below 16) and ends two blocks on.
    let (bx, by) = (
        ((i32::from(x) - 16).max(0) >> 4) + 2,
        ((i32::from(y) - 16).max(0) >> 4) + 2,
    );
    let per = CHUNK_BLOCKS as i32;
    world
        .map(level)
        .chunks
        .get((bx / per) as usize, (by / per) as usize)
        == Some(0)
}

impl Window {
    /// Collects the window for a player at world position (`x`, `y`). Blocks beyond the map
    /// are empty (spec gap: the original reads other map data there; play never reaches it).
    pub fn collect(world: &World, level: &Level, x: u16, y: u16) -> Window {
        let mut window = Window::default();
        let map = world.map(level);
        let origin_x = (i32::from(x) - 16) >> 4;
        let origin_y = (i32::from(y) - 16) >> 4;
        let classes = &world.collision.block_classes[usize::from(level.block_set)];
        for row in 0..3 {
            for column in 0..3 {
                let (bx, by) = (origin_x + column, origin_y + row);
                if bx < 0 || by < 0 {
                    continue;
                }
                let blocks_per_chunk = CHUNK_BLOCKS as i32;
                let chunk = map.chunks.get(
                    (bx / blocks_per_chunk) as usize,
                    (by / blocks_per_chunk) as usize,
                );
                let Some(chunk) = chunk else { continue };
                let index =
                    ((by % blocks_per_chunk) * blocks_per_chunk + bx % blocks_per_chunk) as usize;
                let Some(&block) = world
                    .blocks
                    .chunks
                    .get(usize::from(chunk))
                    .and_then(|chunk| chunk.get(index))
                else {
                    continue;
                };
                // Block 0 and block numbers from 170 on never collide.
                if block == 0 {
                    continue;
                }
                let Some(&class) = classes.get(usize::from(block)) else {
                    continue;
                };
                let (ox, oy) = (column * BLOCK_SIZE as i32, row * BLOCK_SIZE as i32);
                for segment in &world.collision.classes[usize::from(class)].segments {
                    let surface = match segment.line {
                        Line::Flat { x0, width, y } => Surface::Flat {
                            x0: wrap(x0, ox),
                            width: i32::from(width as u8),
                            y: wrap(y, oy),
                        },
                        Line::Wall { x, y0, height } => Surface::Wall {
                            x: wrap(x, ox),
                            y0: wrap(y0, oy),
                            height: i32::from(height as u8),
                        },
                        Line::Floor {
                            y_low,
                            x0,
                            width,
                            y_high,
                        } => Surface::Floor {
                            y_low: wrap(y_low, oy),
                            x0: wrap(x0, ox),
                            width: i32::from(width as u8),
                            y_high: wrap(y_high, oy),
                        },
                        Line::Ceiling {
                            y_top,
                            x0,
                            width,
                            y_bottom,
                        } => Surface::Ceiling {
                            y_top: wrap(y_top, oy),
                            x0: wrap(x0, ox),
                            width: i32::from(width as u8),
                            y_bottom: wrap(y_bottom, oy),
                        },
                    };
                    window.lists[usize::from(segment.direction)].push(surface);
                }
            }
        }
        window
    }

    /// The size of the collision lists in bytes, RAM $B8 (graphics.md 5.7.3): 16 plus 3 per
    /// segment in directions 0, 7, 8, 9 and 4 per segment in the others.
    pub fn list_size(&self) -> u16 {
        let bytes = |direction: usize| {
            if matches!(direction, 0 | 7 | 8 | 9) {
                3
            } else {
                4
            }
        };
        16 + self
            .lists
            .iter()
            .enumerate()
            .map(|(direction, list)| (list.len() * bytes(direction)) as u16)
            .sum::<u16>()
    }

    /// Whether the window holds any flat floor or flat ceiling (direction 0 or 9).
    pub fn has_flat(&self) -> bool {
        !self.lists[0].is_empty() || !self.lists[9].is_empty()
    }
}
