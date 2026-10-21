//! The object data (the pack's `game/objects.ron`, `docs/spec/objects.md` 3, 4.1, 8 and
//! `docs/spec/behaviour.md` 3.6): placement lists, per-type initial frame and state, hit points
//! and flags per state, and collision sizes per frame.

use crate::game::hex::HexRows;
use crate::game::level::Placement;
use serde::{Deserialize, Serialize};

/// File name inside the pack's `game/` folder.
pub const OBJECTS_FILE: &str = "objects.ron";

/// Number of object types (0-$61).
pub const TYPE_COUNT: usize = 0x62;
/// Entries of the hit-point table (objects.md 2.4).
pub const HIT_POINT_ENTRIES: usize = 93;
/// Flag entries for states $00-$24 and $80-$BA (behaviour.md 3.6).
pub const FLAG_ENTRIES_LOW: usize = 37;
pub const FLAG_ENTRIES_HIGH: usize = 59;
/// Frames of table A with a size (objects.md 8).
pub const SIZED_FRAMES: usize = 193;

/// `objects.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectData {
    /// The placement list of every level, in list order (objects.md 3.2); `kind` is the object
    /// type.
    pub placements: Vec<Vec<Placement>>,
    /// Per list entry, the second byte (dy) of its offset pair when it is a member of a group
    /// (objects.md 3.2); the placement scan loads it into the thrust's carry-in byte
    /// (player.md 4.6, rule 3).
    #[serde(default)]
    pub group_dy: Vec<Vec<Option<u8>>>,
    /// Initial frame and initial state of each type 0-$61 (objects.md 4.1).
    pub types: Vec<(u8, u8)>,
    /// Initial hit points per state (objects.md 2.4): index = state for states below $80,
    /// state - $5E for states from $80.
    pub hit_points: HexRows<16>,
    /// Flag bytes of states $00-$24 and of states $80-$BA (behaviour.md 3.6).
    pub flags_low: HexRows<16>,
    pub flags_high: HexRows<16>,
    /// Half-width and half-height of each table-A frame (objects.md 8).
    pub half_width: HexRows<16>,
    pub half_height: HexRows<16>,
    /// The attract demo's waypoint lists, one after the other: (x, y, radius) in pixels
    /// (game-flow.md 7.1, "Demo autopilot").
    pub demo_waypoints: Vec<(u16, u16, u8)>,
    /// The warp levels' mesh helpers (objects.md 10.6, graphics.md 5.7.2): five rows of 13
    /// bytes, one entry per warp level 29-41: the flags, then the x and y source of slot 23
    /// and of slot 24.
    #[serde(default)]
    pub warp_mesh: HexRows<13>,
}

/// The length of the validation input's `warp_shimmer`: three more tables of 13 and a byte
/// index of 0-255.
pub const SHIMMER_BYTES: usize = 3 * 13 + 256;

/// Number of waypoints of the three demo levels (6, 4, 10) and where each list starts.
pub const DEMO_WAYPOINTS: usize = 88;
/// Warp levels 29-41.
pub const WARP_LEVELS: usize = 13;
pub const DEMO_LIST_STARTS: [usize; 3] = [0, 27, 52];

impl ObjectData {
    pub fn parse(text: &str) -> Result<ObjectData, String> {
        let data: ObjectData = ron::from_str(text).map_err(|error| error.to_string())?;
        data.check()?;
        Ok(data)
    }

    fn check(&self) -> Result<(), String> {
        let sizes = [
            ("types", self.types.len(), TYPE_COUNT),
            ("hit points", self.hit_points.len(), HIT_POINT_ENTRIES),
            ("low flags", self.flags_low.len(), FLAG_ENTRIES_LOW),
            ("high flags", self.flags_high.len(), FLAG_ENTRIES_HIGH),
            ("half-widths", self.half_width.len(), SIZED_FRAMES),
            ("half-heights", self.half_height.len(), SIZED_FRAMES),
            ("demo waypoints", self.demo_waypoints.len(), DEMO_WAYPOINTS),
            ("warp mesh bytes", self.warp_mesh.len(), 5 * WARP_LEVELS),
            (
                "placement lists",
                self.placements.len(),
                crate::game::level::LEVEL_COUNT,
            ),
        ];
        for (what, have, want) in sizes {
            if have != want {
                return Err(format!("{have} {what}, expected {want}"));
            }
        }
        Ok(())
    }

    /// Initial frame and state of an object type (objects.md 4.1).
    pub fn type_start(&self, kind: u8) -> (u8, u8) {
        self.types.get(usize::from(kind)).copied().unwrap_or((0, 0))
    }

    /// Initial hit points of a state (objects.md 2.4); states past the table have none.
    pub fn initial_hit_points(&self, state: u8) -> u8 {
        let index = if state < 0x80 {
            usize::from(state)
        } else {
            usize::from(state) - 0x5E
        };
        self.hit_points.get(index).copied().unwrap_or(0)
    }

    /// The flag byte of a state (behaviour.md 3.6). State $25 reads the entry of state $80, as
    /// the original's table overlaps.
    pub fn flags(&self, state: u8) -> u8 {
        let index = usize::from(state);
        match state {
            0..=0x24 => self.flags_low.get(index),
            0x25 => self.flags_high.first(),
            0x80..=0xBA => self.flags_high.get(index - 0x80),
            _ => None,
        }
        .copied()
        .unwrap_or(0)
    }

    /// Half-width and half-height of a table-A frame (objects.md 8); 0 beyond the table.
    pub fn half_size(&self, frame: u8) -> (u8, u8) {
        let index = usize::from(frame);
        (
            self.half_width.get(index).copied().unwrap_or(0),
            self.half_height.get(index).copied().unwrap_or(0),
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small data set for tests: types 1 and 2, the turret and the mine.
    pub fn synthetic() -> ObjectData {
        let mut types = vec![(0, 0); TYPE_COUNT];
        types[1] = (0x1B, 0x86);
        types[0x49] = (0xFF, 0xAE);
        let mut hit_points = vec![0u8; HIT_POINT_ENTRIES];
        hit_points[0x86 - 0x5E] = 3;
        let mut flags_high = vec![0u8; FLAG_ENTRIES_HIGH];
        flags_high[6] = 0x80;
        ObjectData {
            placements: vec![Vec::new(); crate::game::level::LEVEL_COUNT],
            group_dy: Vec::new(),
            types,
            hit_points: HexRows(hit_points),
            flags_low: HexRows(vec![0; FLAG_ENTRIES_LOW]),
            flags_high: HexRows(flags_high),
            half_width: HexRows(vec![4; SIZED_FRAMES]),
            half_height: HexRows(vec![4; SIZED_FRAMES]),
            demo_waypoints: vec![(0, 0, 0); DEMO_WAYPOINTS],
            warp_mesh: HexRows(vec![0; 5 * WARP_LEVELS]),
        }
    }

    #[test]
    fn tables_are_indexed_by_state() {
        let data = synthetic();
        assert_eq!(data.initial_hit_points(0x86), 3);
        assert_eq!(data.flags(0x86), 0x80);
        assert_eq!(data.flags(0x25), data.flags(0x80));
        assert_eq!(data.half_size(0x1B), (4, 4));
        assert_eq!(data.type_start(1), (0x1B, 0x86));
    }
}
