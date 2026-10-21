//! The pilot's map of a level: the terrain's surfaces drawn into a grid of 8-pixel cells, the
//! open space the pod can reach from where it starts, how far each cell is from the rock, and
//! distance fields that lead to a goal along the open space.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::game::level::{BLOCK_SIZE, CHUNK_BLOCKS, Level, Line, World};

/// Cell size in pixels.
pub const CELL: i32 = 8;
/// Clearance is counted up to this many cells.
const MAX_CLEARANCE: u8 = 12;
/// The pod never flies above this y (levels.md 6.5).
const CEILING_Y: i32 = 16;
/// How far from a position (cells) a field looks for a cell it reaches: the pod may be in a
/// narrow place the field leaves out.
const REACH: i32 = 4;
/// A distance field's value for a cell it does not reach.
pub const UNREACHED: u32 = u32::MAX;

/// The grid of one level.
#[derive(Debug, Clone)]
pub struct NavMap {
    pub width: i32,
    pub height: i32,
    /// Per cell: 0 for rock, a surface or a place the pod cannot reach; else the distance in
    /// cells to the nearest such cell (1 next to it), up to `MAX_CLEARANCE`.
    clearance: Vec<u8>,
}

/// Distances to one goal, in cost units (10 per straight cell), for every reachable cell.
#[derive(Debug, Clone)]
pub struct Field {
    pub goal: (i32, i32),
    cost: Vec<u32>,
}

/// The eight neighbours with their step costs.
const NEIGHBOURS: [(i32, i32, u32); 8] = [
    (1, 0, 10),
    (-1, 0, 10),
    (0, 1, 10),
    (0, -1, 10),
    (1, 1, 14),
    (1, -1, 14),
    (-1, 1, 14),
    (-1, -1, 14),
];

impl NavMap {
    /// The grid of `level`, open where the pod can reach from `start` (pixels), with the box
    /// `closed` (x0, y0, x1, y1 in pixels) shut.
    pub fn new(
        world: &World,
        level: &Level,
        start: (u16, u16),
        closed: (i32, i32, i32, i32),
    ) -> NavMap {
        let blocks_x = i32::from(level.width) * CHUNK_BLOCKS as i32;
        let blocks_y = level.map_rows() as i32 * CHUNK_BLOCKS as i32;
        let cells_per_block = BLOCK_SIZE as i32 / CELL;
        let (width, height) = (blocks_x * cells_per_block, blocks_y * cells_per_block);
        let mut solid = vec![false; (width * height) as usize];
        let mut mark = |x: i32, y: i32| {
            let (cx, cy) = (x.div_euclid(CELL), y.div_euclid(CELL));
            if (0..width).contains(&cx) && (0..height).contains(&cy) {
                solid[(cy * width + cx) as usize] = true;
            }
        };
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let (ox, oy) = (bx * BLOCK_SIZE as i32, by * BLOCK_SIZE as i32);
                for line in block_lines(world, level, bx, by) {
                    draw(line, ox, oy, &mut mark);
                }
            }
        }
        for y in (closed.1..=closed.3).step_by(CELL as usize / 2) {
            for x in (closed.0..=closed.2).step_by(CELL as usize / 2) {
                mark(x, y);
            }
        }
        let ceiling_rows = (CEILING_Y + CELL - 1) / CELL;
        for cell in solid.iter_mut().take((ceiling_rows * width) as usize) {
            *cell = true;
        }
        // The open space: four-connected from the start, so that a diagonal pair of surface
        // cells still closes the rock off.
        let mut open = vec![false; solid.len()];
        let mut queue = VecDeque::new();
        let first = nearest_open(&solid, width, height, start);
        if let Some(first) = first {
            open[first] = true;
            queue.push_back(first);
        }
        while let Some(at) = queue.pop_front() {
            let (x, y) = (at as i32 % width, at as i32 / width);
            for (dx, dy, _) in &NEIGHBOURS[..4] {
                let (nx, ny) = (x + dx, y + dy);
                if (0..width).contains(&nx) && (0..height).contains(&ny) {
                    let next = (ny * width + nx) as usize;
                    if !open[next] && !solid[next] {
                        open[next] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        // Clearance: a breadth-first walk out of everything that is not open.
        let mut clearance = vec![0u8; solid.len()];
        let mut queue = VecDeque::new();
        for (at, &is_open) in open.iter().enumerate() {
            if is_open {
                let (x, y) = (at as i32 % width, at as i32 / width);
                let edge = NEIGHBOURS.iter().any(|&(dx, dy, _)| {
                    let (nx, ny) = (x + dx, y + dy);
                    !(0..width).contains(&nx)
                        || !(0..height).contains(&ny)
                        || !open[(ny * width + nx) as usize]
                });
                if edge {
                    clearance[at] = 1;
                    queue.push_back(at);
                }
            }
        }
        while let Some(at) = queue.pop_front() {
            let c = clearance[at];
            if c >= MAX_CLEARANCE {
                continue;
            }
            let (x, y) = (at as i32 % width, at as i32 / width);
            for (dx, dy, _) in NEIGHBOURS {
                let (nx, ny) = (x + dx, y + dy);
                if (0..width).contains(&nx) && (0..height).contains(&ny) {
                    let next = (ny * width + nx) as usize;
                    if open[next] && clearance[next] == 0 {
                        clearance[next] = c + 1;
                        queue.push_back(next);
                    }
                }
            }
        }
        // What the walk stopped short of is farther than it counts.
        for (c, &is_open) in clearance.iter_mut().zip(&open) {
            if is_open && *c == 0 {
                *c = MAX_CLEARANCE;
            }
        }
        NavMap {
            width,
            height,
            clearance,
        }
    }

    /// The cell of a position in pixels.
    pub fn cell(x: i32, y: i32) -> (i32, i32) {
        (x.div_euclid(CELL), y.div_euclid(CELL))
    }

    /// The centre of a cell in pixels.
    pub fn centre((cx, cy): (i32, i32)) -> (f32, f32) {
        ((cx * CELL + CELL / 2) as f32, (cy * CELL + CELL / 2) as f32)
    }

    fn index(&self, (cx, cy): (i32, i32)) -> Option<usize> {
        ((0..self.width).contains(&cx) && (0..self.height).contains(&cy))
            .then(|| (cy * self.width + cx) as usize)
    }

    /// The clearance of a cell (0: not open).
    pub fn clearance(&self, cell: (i32, i32)) -> u8 {
        self.index(cell).map_or(0, |i| self.clearance[i])
    }

    /// The clearance at a position in pixels.
    pub fn clearance_at(&self, x: f32, y: f32) -> u8 {
        self.clearance(NavMap::cell(x as i32, y as i32))
    }

    /// The open cell nearest to `cell` (within a few cells) with at least `min` clearance.
    pub fn nearest(&self, cell: (i32, i32), min: u8) -> Option<(i32, i32)> {
        for radius in 0..16i32 {
            let mut best = None;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let at = (cell.0 + dx, cell.1 + dy);
                    let c = self.clearance(at);
                    if c >= min && best.is_none_or(|(_, b)| c > b) {
                        best = Some((at, c));
                    }
                }
            }
            if let Some((at, _)) = best {
                return Some(at);
            }
        }
        None
    }

    /// The cost of entering a cell: narrow places cost more, and cells below `min` clearance
    /// are closed.
    fn weight(&self, clearance: u8, min: u8) -> Option<u32> {
        if clearance < min {
            return None;
        }
        Some(match clearance - min {
            0 => 6,
            1 => 3,
            2 => 2,
            _ => 1,
        })
    }

    /// The distances from every cell to `goal` (pixels), through cells of at least `min`
    /// clearance. None if the goal is nowhere near the open space.
    pub fn field(&self, goal: (i32, i32), min: u8) -> Option<Field> {
        let target = self.nearest(NavMap::cell(goal.0, goal.1), min)?;
        let mut cost = vec![UNREACHED; self.clearance.len()];
        let mut heap = BinaryHeap::new();
        let start = self.index(target)?;
        cost[start] = 0;
        heap.push(Reverse((0u32, start)));
        while let Some(Reverse((d, at))) = heap.pop() {
            if d > cost[at] {
                continue;
            }
            let (x, y) = (at as i32 % self.width, at as i32 / self.width);
            for (dx, dy, step) in NEIGHBOURS {
                let Some(next) = self.index((x + dx, y + dy)) else {
                    continue;
                };
                let Some(weight) = self.weight(self.clearance[next], min) else {
                    continue;
                };
                if dx != 0 && dy != 0 {
                    // No corner cutting past a closed cell.
                    let side = |cx, cy| self.clearance((cx, cy)) >= min;
                    if !side(x + dx, y) || !side(x, y + dy) {
                        continue;
                    }
                }
                let total = d + step * weight;
                if total < cost[next] {
                    cost[next] = total;
                    heap.push(Reverse((total, next)));
                }
            }
        }
        Some(Field { goal, cost })
    }

    /// Whether the straight line between two points (pixels) stays in cells of at least `min`
    /// clearance.
    pub fn line_clear(&self, from: (f32, f32), to: (f32, f32), min: u8) -> bool {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = (dx.abs().max(dy.abs()) / (CELL as f32 / 2.0))
            .ceil()
            .max(1.0) as i32;
        (0..=steps).all(|i| {
            let t = i as f32 / steps as f32;
            self.clearance_at(from.0 + dx * t, from.1 + dy * t) >= min
        })
    }
}

impl Field {
    /// The cost from a cell to the goal.
    pub fn cost(&self, map: &NavMap, cell: (i32, i32)) -> u32 {
        map.index(cell).map_or(UNREACHED, |i| self.cost[i])
    }

    /// The cost from a position (pixels), from the best cell near it.
    pub fn cost_at(&self, map: &NavMap, x: f32, y: f32) -> u32 {
        let (cx, cy) = NavMap::cell(x as i32, y as i32);
        let mut best = UNREACHED;
        for dy in -REACH..=REACH {
            for dx in -REACH..=REACH {
                best = best.min(self.cost(map, (cx + dx, cy + dy)));
            }
        }
        best
    }

    /// The cells from `from` (pixels) toward the goal, downhill, at most `count` of them.
    pub fn path(&self, map: &NavMap, from: (f32, f32), count: usize) -> Vec<(i32, i32)> {
        let (cx, cy) = NavMap::cell(from.0 as i32, from.1 as i32);
        // Start from the best cell near the pod: it may be in a narrow cell the field closed.
        let mut at = None;
        let mut best = UNREACHED;
        for dy in -REACH..=REACH {
            for dx in -REACH..=REACH {
                let c = self
                    .cost(map, (cx + dx, cy + dy))
                    .saturating_add(10 * (dx.abs() + dy.abs()) as u32);
                if c < best {
                    best = c;
                    at = Some((cx + dx, cy + dy));
                }
            }
        }
        let Some(mut at) = at else {
            return Vec::new();
        };
        let mut path = vec![at];
        while path.len() < count {
            let here = self.cost(map, at);
            if here == 0 {
                break;
            }
            let next = NEIGHBOURS
                .iter()
                .map(|&(dx, dy, _)| (at.0 + dx, at.1 + dy))
                .min_by_key(|&cell| self.cost(map, cell));
            match next {
                Some(next) if self.cost(map, next) < here => {
                    at = next;
                    path.push(at);
                }
                _ => break,
            }
        }
        path
    }
}

/// The surfaces of block (`bx`, `by`) (levels.md 6.1-6.3). Block 0, numbers past the class
/// map and blocks outside the map have none.
fn block_lines<'a>(
    world: &'a World,
    level: &Level,
    bx: i32,
    by: i32,
) -> impl Iterator<Item = (u8, Line)> + 'a {
    let per = CHUNK_BLOCKS as i32;
    let chunk = world
        .map(level)
        .chunks
        .get((bx / per) as usize, (by / per) as usize);
    let block = chunk.and_then(|chunk| {
        world
            .blocks
            .chunks
            .get(usize::from(chunk))?
            .get(((by % per) * per + bx % per) as usize)
            .copied()
    });
    let class = block.filter(|&b| b != 0).and_then(|block| {
        world.collision.block_classes[usize::from(level.block_set)]
            .get(usize::from(block))
            .copied()
    });
    class
        .and_then(|class| world.collision.classes.get(usize::from(class)))
        .into_iter()
        .flat_map(|class| class.segments.iter().map(|s| (s.direction, s.line)))
}

/// Marks every pixel of a surface, block-local coordinates moved by (`ox`, `oy`).
fn draw(line: (u8, Line), ox: i32, oy: i32, mark: &mut impl FnMut(i32, i32)) {
    let (direction, line) = line;
    match line {
        Line::Flat { x0, width, y } => {
            for x in 0..=i32::from(width as u8) {
                mark(ox + i32::from(x0) + x, oy + i32::from(y));
            }
        }
        Line::Wall { x, y0, height } => {
            for y in 0..=i32::from(height as u8) {
                mark(ox + i32::from(x), oy + i32::from(y0) + y);
            }
        }
        Line::Floor {
            y_low,
            x0,
            width,
            y_high,
        } => {
            let (y_low, y_high) = (i32::from(y_low), i32::from(y_high));
            slope(
                i32::from(x0),
                i32::from(width as u8),
                ox,
                oy,
                mark,
                |t| match direction {
                    1 => y_high + t / 2,
                    2 => y_low - t / 2,
                    3 => y_high + t,
                    4 => y_low - t,
                    5 => y_high + 2 * t,
                    _ => y_low - 2 * t,
                },
            );
        }
        Line::Ceiling {
            y_top,
            x0,
            width,
            y_bottom,
        } => {
            let (y_top, y_bottom) = (i32::from(y_top), i32::from(y_bottom));
            slope(
                i32::from(x0),
                i32::from(width as u8),
                ox,
                oy,
                mark,
                |t| match direction {
                    10 => y_bottom - t,
                    11 => y_top + t,
                    12 => y_bottom - 2 * t,
                    13 => y_top + 2 * t,
                    14 => y_bottom - t / 2,
                    _ => y_top + t / 2,
                },
            );
        }
    }
}

/// A sloped surface, column by column, with the pixels between one column's height and the
/// next filled so that steep lines have no gaps.
fn slope(
    x0: i32,
    width: i32,
    ox: i32,
    oy: i32,
    mark: &mut impl FnMut(i32, i32),
    height: impl Fn(i32) -> i32,
) {
    let mut last = height(0);
    for t in 0..=width {
        let y = height(t);
        for between in last.min(y)..=last.max(y) {
            mark(ox + x0 + t, oy + between);
        }
        last = y;
    }
}

/// The open cell nearest to a position: the start may sit on a surface cell.
fn nearest_open(solid: &[bool], width: i32, height: i32, at: (u16, u16)) -> Option<usize> {
    let (cx, cy) = NavMap::cell(i32::from(at.0), i32::from(at.1));
    for radius in 0..8 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (x, y) = (cx + dx, cy + dy);
                if (0..width).contains(&x) && (0..height).contains(&y) {
                    let i = (y * width + x) as usize;
                    if !solid[i] {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}
