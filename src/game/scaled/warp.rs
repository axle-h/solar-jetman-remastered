//! Warp levels 29-41: the countdown digit and the two mesh helpers (`docs/spec/levels.md` 6.7,
//! `docs/spec/objects.md` 10.6, `docs/spec/graphics.md` 5.7.2, `docs/spec/behaviour.md` 6.3).
//! The routine runs on every video frame, in both passes.

use super::Scaled;
use super::objects::{Object, state};
use crate::game::graphics::LevelGraphics;
use crate::game::level::{FIRST_WARP_LEVEL, Level, World};

/// The warp set's first tile, the mesh that drifts (graphics.md 5.7.2).
const MESH_TILE: u8 = 0x81;

/// The two helper slots: slot 23's position drives the mesh tile.
pub const HELPER_SLOTS: [usize; 2] = [23, 24];
/// Where a homing helper starts (objects.md 10.6).
const HELPER_START: (u16, u16) = (0, 16);
/// Acceleration and top speed of the homing helpers, in 1/256 px per frame: slot 23 and slot
/// 24 (objects.md 10.6).
const HOME_SLOT_23: (u16, u16) = (2, 0x50);
const HOME_SLOT_24: (u16, u16) = (3, 0x40);

impl Scaled {
    /// The flag byte of the warp level (objects.md 10.6): bit 7 moves slot 23 as a homing
    /// helper, bit 6 slot 24 too. The remake gives levels 37 and 39 the behaviour of 33 and 35
    /// (Q38).
    fn warp_flags(&self, world: &World) -> u8 {
        world.objects.warp_mesh[self.level - FIRST_WARP_LEVEL] & 0x80
    }

    /// The helpers' start: the homing ones begin at (0, 16) at rest, heading right and up.
    pub(super) fn start_warp_level(&mut self, world: &World) {
        if self.warp_flags(world) & 0x80 != 0 {
            for slot in HELPER_SLOTS {
                self.objects.slots[slot] = Object {
                    x: HELPER_START.0,
                    y: HELPER_START.1,
                    ..Object::default()
                };
            }
        }
    }

    /// One call of the warp-level routine.
    pub(super) fn warp_routine(&mut self, world: &World, level: &Level) {
        if !level.is_warp_level() {
            return;
        }
        // The countdown's object is made in the first pass (behaviour.md 6.3): the slot takes
        // the state and a zero counter, and the object's own step writes the position, frame
        // and flags X; everything else stays what the last occupant left (the remake starts
        // from a cleared slot).
        if self.objects.slots[3].state != state::COUNTDOWN {
            let slot = &mut self.objects.slots[3];
            *slot = Object::default();
            slot.state = state::COUNTDOWN;
            slot.speed_x = 0;
        }
        let flags = self.warp_flags(world);
        let (px, py) = (self.pod.body.x, self.pod.body.y);
        if flags & 0x80 != 0 {
            let helper = &mut self.objects.slots[HELPER_SLOTS[0]];
            helper.home_x(px, HOME_SLOT_23.0, HOME_SLOT_23.1);
            helper.home_y(py, HOME_SLOT_23.0, HOME_SLOT_23.1);
        }
        if flags & 0x40 != 0 {
            let helper = &mut self.objects.slots[HELPER_SLOTS[1]];
            helper.home_x(px, HOME_SLOT_24.0, HOME_SLOT_24.1);
            helper.home_y(py, HOME_SLOT_24.0, HOME_SLOT_24.1);
        }
        // The tables set the helpers' low bytes, except on the levels whose helper homes by
        // itself with the remake's rules (and with the original's on 37 and 39, whose lookup
        // is wrong: see `table_index`).
        if flags & 0x80 == 0 || flags & 0x40 != 0 {
            let rows = &world.objects;
            let source =
                |n: usize| rows.warp_mesh[13 * n + usize::from(level.number) - FIRST_WARP_LEVEL];
            let values = [
                self.mesh_source(source(1)),
                self.mesh_source(source(2)),
                self.mesh_source(source(3)),
                self.mesh_source(source(4)),
            ];
            let set = |o: &mut Object, x: u8, y: u8| {
                o.x = (o.x & 0xFF00) | u16::from(x);
                o.y = (o.y & 0xFF00) | u16::from(y);
            };
            set(
                &mut self.objects.slots[HELPER_SLOTS[0]],
                values[0],
                values[1],
            );
            set(
                &mut self.objects.slots[HELPER_SLOTS[1]],
                values[2],
                values[3],
            );
        }
    }

    /// The value a source byte names (graphics.md 5.7.2): a quantity in the low bits, a shift in
    /// the high ones; kept to 8 bits.
    fn mesh_source(&self, source: u8) -> u8 {
        let value = match source & 7 {
            0 => 5,
            1 => self.camera.position.x as u8,
            2 => self.camera.position.y as u8,
            3 => self.pod.body.x as u8,
            4 => self.pod.body.y as u8,
            5 => self.counter as u8,
            6 => self.pod.body.h_speed() as u8,
            // Graphics.md 5.7.2: the pod's vertical speed fraction byte.
            _ => self.pod.body.v_speed() as u8,
        };
        match source >> 4 {
            1 | 3 => value << 2,
            2 => value << 1,
            8..=0xB => value >> 1,
            4..=7 | 0xC..=0xF => value >> 2,
            _ => value,
        }
    }

    /// The mesh tile's offsets (graphics.md 5.7.2): the low three bits of slot 23's x and y.
    pub fn mesh_offsets(&self) -> (u8, u8) {
        let helper = &self.objects.slots[HELPER_SLOTS[0]];
        ((helper.x & 7) as u8, (helper.y & 7) as u8)
    }

    /// The level's graphics as the game shows them now: on a warp level tile $81 of the
    /// background set is the mesh, its rows and columns rotated by the offsets (graphics.md
    /// 5.7.2), and in the gate state the background palettes 2 and 3 are darkened once for
    /// every fade so far (player.md 9.2.3 step 6).
    pub(super) fn changed_graphics(&self, graphics: &LevelGraphics) -> LevelGraphics {
        let mut changed = graphics.clone();
        if self.level >= FIRST_WARP_LEVEL {
            let (ox, oy) = self.mesh_offsets();
            let base = graphics.background.tile(MESH_TILE);
            // Variant `ox` of the tile: each plane's rows rotated right by `ox` bits, as the
            // table of eight variants holds them (16 bytes each, plane 0 rows then plane 1).
            let mut variant = [0u8; 16];
            for (i, byte) in variant.iter_mut().enumerate() {
                let (plane, row) = (i / 8, i % 8);
                let bits = (0..8).fold(0u8, |bits, x| {
                    bits | ((base[row * 8 + x] >> plane) & 1) << (7 - x)
                });
                *byte = bits.rotate_right(u32::from(ox));
            }
            let mut pixels = [0u8; 64];
            for y in 0..8usize {
                let row = (y + usize::from(oy)) & 7;
                for x in 0..8usize {
                    let bit = |plane: usize| (variant[plane * 8 + row] >> (7 - x)) & 1;
                    pixels[y * 8 + x] = bit(0) | bit(1) << 1;
                }
            }
            changed.background.set_tile(MESH_TILE, pixels);
        }
        if let Some(colours) = self.terrain.shown {
            changed.palette.0[9..12].copy_from_slice(&colours);
        }
        for _ in 0..self.gate_fades() {
            for entry in &mut changed.palette.0[8..16] {
                *entry = if *entry >> 4 == 0 {
                    0x0F
                } else {
                    *entry - 0x10
                };
            }
        }
        changed
    }

    /// The helpers' position low bytes, for the replay comparison.
    pub fn helper_bytes(&self) -> [u8; 4] {
        let (a, b) = (
            &self.objects.slots[HELPER_SLOTS[0]],
            &self.objects.slots[HELPER_SLOTS[1]],
        );
        [a.x as u8, a.y as u8, b.x as u8, b.y as u8]
    }
}
