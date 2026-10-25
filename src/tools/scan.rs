//! The release scan: finds runs of the ROM's ripped bytes inside another file (a release
//! binary, a committed data file).

use regex::Regex;
use std::collections::HashMap;

/// Shortest run of consecutive ROM bytes that counts as a hit.
pub const MIN_RUN: usize = 16;

/// Fewest distinct byte values in a window for it to be searched: a window of one or two
/// values (zero runs, fill) matches unrelated data by chance. Windows that count up or down
/// by a constant step (0x20, 0x21, 0x22 ...), or are three quarters one value, are skipped for the same reason.
const MIN_DISTINCT: usize = 4;

/// A clean-room category of `docs/spec/rom-map.md`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Ripped,
    Commit,
    /// A table of constants the specs give (not copyrightable), held in the remake's source.
    Constants,
    Code,
    Fill,
    Unknown,
}

impl Category {
    fn parse(text: &str) -> Option<Category> {
        let word = text.split(|c: char| !c.is_ascii_alphabetic()).next()?;
        Some(match word {
            "ripped" => Category::Ripped,
            "commit" => Category::Commit,
            "constants" => Category::Constants,
            "code" => Category::Code,
            "fill" => Category::Fill,
            "unknown" => Category::Unknown,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Category::Ripped => "ripped",
            Category::Commit => "commit",
            Category::Constants => "constants",
            Category::Code => "code",
            Category::Fill => "fill",
            Category::Unknown => "unknown",
        }
    }
}

/// One row of the ROM map: PRG offsets `start..=end`.
#[derive(Clone, Debug)]
pub struct Region {
    pub start: usize,
    pub end: usize,
    pub category: Category,
    pub label: String,
}

/// The categories whose bytes must not be in a release: everything but commit, constants and fill.
pub const NOT_COMMITTABLE: &[Category] = &[Category::Ripped, Category::Code, Category::Unknown];

/// Tables of the remake that match ROM bytes the map does not mark as constants, but come from
/// the specs: a hit inside one of them is allowed.
pub const ALLOWED: &[(&[u8], &str)] = &[(
    &crate::game::gameplay::final_level::HUES,
    "final level terrain hues (graphics.md 7.5)",
)];

/// PRG ranges (start, length) that a build was found to match by chance, not because it holds
/// them: a hit inside one is allowed. Only an offset is recorded, never the bytes.
pub const CHANCE: &[(usize, usize, &str)] = &[(
    0x29312,
    17,
    "chance match: two adjacent 16-byte constants of the Android arm64 library (layout \
     numbers of the printer messages and a deserializer's), low-entropy bytes",
)];

/// The `ALLOWED` table holding the bytes of `hit` in `data`, or the `CHANCE` range holding
/// `hit`, if any.
pub fn allowed(hit: &Hit, data: &[u8]) -> Option<&'static str> {
    let run = &data[hit.found_at..hit.found_at + hit.len];
    ALLOWED
        .iter()
        .find(|(table, _)| table.windows(run.len()).any(|w| w == run))
        .map(|&(_, what)| what)
        .or_else(|| {
            CHANCE
                .iter()
                .find(|&&(start, len, _)| {
                    hit.rom_offset >= start && hit.rom_offset + hit.len <= start + len
                })
                .map(|&(_, _, what)| what)
        })
}

fn hex(text: &str) -> Option<usize> {
    usize::from_str_radix(text.trim().strip_prefix("0x")?, 16).ok()
}

/// Reads the regions from the tables of `rom-map.md`. A row whose category is
/// "commit, except bytes A-B: ripped" and whose text says "N bytes each" becomes one ripped
/// region per record.
pub fn parse_rom_map(text: &str) -> Vec<Region> {
    let each = Regex::new(r"(\d+) bytes each").expect("regex");
    let except = Regex::new(r"except bytes (\d+)-(\d+): (\w+)").expect("regex");
    let mut regions = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("| 0x")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 5 {
            continue;
        }
        let Some(category) = Category::parse(cells[4]) else {
            continue;
        };
        let label: String = cells[3].chars().take(60).collect();
        for part in cells[1].split(',') {
            let (start, end) = match part.split_once('-') {
                Some((a, b)) => (hex(a), hex(b)),
                None => (hex(part), hex(part)),
            };
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            let mut push = |start, end, category| {
                regions.push(Region {
                    start,
                    end,
                    category,
                    label: label.clone(),
                });
            };
            push(start, end, category);
            if let (Some(e), Some(each)) = (except.captures(cells[4]), each.captures(cells[3]))
                && let (Ok(a), Ok(b), Ok(size), Some(inner)) = (
                    e[1].parse::<usize>(),
                    e[2].parse::<usize>(),
                    each[1].parse::<usize>(),
                    Category::parse(&e[3]),
                )
                && size > 0
            {
                let mut record = start;
                while record + size - 1 <= end {
                    push(record + a, record + b, inner);
                    record += size;
                }
            }
        }
    }
    regions
}

/// The category of each PRG byte (`len` bytes). Larger regions are painted first, so a
/// region nested in another one (a table inside code) wins; bytes in no region are unknown.
pub fn byte_categories(regions: &[Region], len: usize) -> Vec<Category> {
    let mut order: Vec<&Region> = regions.iter().collect();
    order.sort_by_key(|r| std::cmp::Reverse(r.end - r.start));
    let mut map = vec![Category::Unknown; len];
    for region in order {
        let end = region.end.min(len.saturating_sub(1));
        if region.start <= end {
            map[region.start..=end].fill(region.category);
        }
    }
    map
}

/// A run of ROM bytes found in the scanned data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// PRG offset of the first byte of the run.
    pub rom_offset: usize,
    /// Offset of the run in the scanned data.
    pub found_at: usize,
    pub len: usize,
}

const BASE: u64 = 0x100_0000_01B3;

/// Finds every maximal run of at least `MIN_RUN` consecutive bytes of `prg` in `data`, where
/// all the run's bytes have a category in `wanted` (`categories` is `prg`'s, one per byte).
pub fn scan(data: &[u8], prg: &[u8], categories: &[Category], wanted: &[Category]) -> Vec<Hit> {
    if data.len() < MIN_RUN || prg.len() < MIN_RUN {
        return Vec::new();
    }
    let top = (0..MIN_RUN - 1).fold(1u64, |p, _| p.wrapping_mul(BASE));
    let hash = |window: &[u8]| {
        window.iter().fold(0u64, |h, &b| {
            h.wrapping_mul(BASE).wrapping_add(u64::from(b) + 1)
        })
    };
    let searchable = |window: &[u8]| {
        let mut seen = [0usize; 256];
        window.iter().for_each(|&b| seen[usize::from(b)] += 1);
        let mostly_one_value = seen.iter().any(|&n| n >= MIN_RUN * 3 / 4);
        let arithmetic = window
            .windows(2)
            .all(|w| w[1].wrapping_sub(w[0]) == window[1].wrapping_sub(window[0]));
        !arithmetic && !mostly_one_value && seen.iter().filter(|&&n| n > 0).count() >= MIN_DISTINCT
    };
    // Hash of each searchable ROM window, to its offsets.
    let mut table: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut bad = categories[..MIN_RUN - 1]
        .iter()
        .filter(|c| !wanted.contains(c))
        .count();
    for start in 0..=prg.len() - MIN_RUN {
        let last = start + MIN_RUN - 1;
        if !wanted.contains(&categories[last]) {
            bad += 1;
        }
        if bad == 0 && searchable(&prg[start..=last]) {
            table
                .entry(hash(&prg[start..=last]))
                .or_default()
                .push(start);
        }
        if !wanted.contains(&categories[start]) {
            bad -= 1;
        }
    }
    // Roll over the data. Runs are tracked by diagonal (rom offset - data position).
    let mut open: HashMap<i64, Hit> = HashMap::new();
    let mut hits = Vec::new();
    let mut h = hash(&data[..MIN_RUN]);
    for pos in 0..=data.len() - MIN_RUN {
        if pos > 0 {
            let out = u64::from(data[pos - 1]) + 1;
            let inc = u64::from(data[pos + MIN_RUN - 1]) + 1;
            h = h
                .wrapping_sub(out.wrapping_mul(top))
                .wrapping_mul(BASE)
                .wrapping_add(inc);
        }
        let Some(offsets) = table.get(&h) else {
            continue;
        };
        let window = &data[pos..pos + MIN_RUN];
        for &offset in offsets.iter().filter(|&&o| prg[o..o + MIN_RUN] == *window) {
            let diagonal = offset as i64 - pos as i64;
            match open.get_mut(&diagonal) {
                Some(run) if run.found_at + run.len - MIN_RUN + 1 == pos => run.len += 1,
                _ => {
                    let new = Hit {
                        rom_offset: offset,
                        found_at: pos,
                        len: MIN_RUN,
                    };
                    if let Some(old) = open.insert(diagonal, new) {
                        hits.push(old);
                    }
                }
            }
        }
    }
    hits.extend(open.into_values());
    hits.sort_by_key(|h| (h.found_at, h.rom_offset));
    hits
}

/// The smallest region containing PRG offset `offset`.
pub fn region_at(regions: &[Region], offset: usize) -> Option<&Region> {
    regions
        .iter()
        .filter(|r| r.start <= offset && offset <= r.end)
        .min_by_key(|r| r.end - r.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "\
| Offset | CPU | What | Category | Status |
| 0x00000-0x0003F | $8000-$803F | program | code | [V] |
| 0x00010-0x0001F | $8010-$801F | text table | commit (text) | [C] |
| 0x00040-0x0007F | $8040-$807F | tiles | ripped | [V] |
| 0x00080-0x000BF | $8080-$80BF | 2 records of 32 bytes each | commit, except bytes 4-7: ripped | [V] |
| 0x000C0-0x000FF | $80C0-$80FF | $FF | fill | [V] |
";

    fn prg() -> Vec<u8> {
        (0..256u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect()
    }

    #[test]
    fn map_parsing_and_nesting() {
        let regions = parse_rom_map(MAP);
        let cats = byte_categories(&regions, 256);
        assert_eq!(cats[0x00], Category::Code);
        assert_eq!(cats[0x10], Category::Commit);
        assert_eq!(cats[0x50], Category::Ripped);
        assert_eq!(cats[0x83], Category::Commit);
        assert_eq!(cats[0x84], Category::Ripped);
        assert_eq!(cats[0xA7], Category::Ripped);
        assert_eq!(cats[0xE0], Category::Fill);
        assert_eq!(
            region_at(&regions, 0x12).unwrap().category,
            Category::Commit
        );
    }

    #[test]
    fn finds_embedded_runs_only() {
        let regions = parse_rom_map(MAP);
        let prg = prg();
        let cats = byte_categories(&regions, prg.len());
        let mut data = vec![0xEE; 40];
        data.extend_from_slice(&prg[0x44..0x44 + 30]); // ripped: found
        data.extend_from_slice(&[0xEE; 7]);
        data.extend_from_slice(&prg[0x14..0x14 + 12]); // commit: not
        data.extend_from_slice(&[0xEE; 9]);
        data.extend_from_slice(&prg[0x60..0x60 + 15]); // too short
        let hits = scan(&data, &prg, &cats, &[Category::Ripped]);
        assert_eq!(
            hits,
            vec![Hit {
                rom_offset: 0x44,
                found_at: 40,
                len: 30
            }]
        );
        // A run crossing from code into commit is not a hit for code-only scanning.
        let mut data = vec![0xEE; 5];
        data.extend_from_slice(&prg[0x08..0x08 + 20]);
        assert!(scan(&data, &prg, &cats, &[Category::Code]).is_empty());
        let both = [Category::Code, Category::Commit];
        assert_eq!(scan(&data, &prg, &cats, &both).len(), 1);
    }

    #[test]
    fn trivial_windows_are_skipped() {
        let mut prg = prg();
        prg[0x40..0x60].fill(0);
        let cats = byte_categories(&parse_rom_map(MAP), prg.len());
        let data = vec![0u8; 100];
        assert!(scan(&data, &prg, &cats, &[Category::Ripped]).is_empty());
    }

    #[test]
    fn allowed_tables() {
        let (table, _) = ALLOWED[0];
        let mut data = vec![0xEE; 8];
        data.extend_from_slice(table);
        let hit = |found_at, len| Hit {
            rom_offset: 0,
            found_at,
            len,
        };
        assert!(allowed(&hit(8, table.len()), &data).is_some());
        assert!(allowed(&hit(7, table.len()), &data).is_none());
    }

    #[test]
    fn chance_ranges() {
        let (start, len, _) = CHANCE[0];
        let data = vec![0xEE; 64];
        let hit = |rom_offset, len| Hit {
            rom_offset,
            found_at: 0,
            len,
        };
        assert!(allowed(&hit(start, len), &data).is_some());
        assert!(allowed(&hit(start + 1, len - 1), &data).is_some());
        assert!(allowed(&hit(start, len + 1), &data).is_none());
        assert!(allowed(&hit(start - 1, len), &data).is_none());
    }
}
