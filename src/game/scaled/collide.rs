//! Collisions of the pod and its shots with objects, damage and destruction
//! (`docs/spec/behaviour.md` 3).

use super::clock::STEP;
use super::objdata::ObjectData;
use super::objects::{
    INVISIBLE, ITEM_SLOTS, ObjectTable, PLACED_SLOTS, SHOT_SLOTS, Score, penetration, relative,
    state,
};
use super::pod::Pod;
use super::rng::Rng;
use crate::game::sound::{Channel, Request};

/// Player states in which the checks run (behaviour.md 3): the pod flying, Jetman flying and
/// standing, and the final level.
fn checks_run(pod: &Pod) -> bool {
    matches!(pod.state, 1 | 2 | 3 | 6)
}

/// The collision checks of a tick: pod against objects (3.1), shots against bonus objects
/// (3.3), shots against placed objects (3.4). Each object hurts the pod at most once a step of
/// game time: `ready` holds, per slot, when it may next (`docs/scaled.md`).
#[allow(clippy::too_many_arguments)]
pub fn collide(
    table: &mut ObjectTable,
    pod: &mut Pod,
    data: &ObjectData,
    rng: &Rng,
    score: &mut Score,
    tag: &mut u8,
    ready: &mut [u64],
    now: u64,
) {
    if !checks_run(pod) {
        return;
    }
    // Spec gap: a touch that hurts the pod ends the step's checks; the shots are not tested
    // against the placed objects then (Mesen, level 2: a shot that overlapped a creature in
    // the step the pod was hit hit it one step later).
    if pod_against_objects(table, pod, data, score, ready, now) {
        return;
    }
    shots(table, pod, data, rng, score, tag);
}

/// Pod against the objects of slots 15-30 (behaviour.md 3.1).
fn pod_against_objects(
    table: &mut ObjectTable,
    pod: &mut Pod,
    data: &ObjectData,
    score: &mut Score,
    ready: &mut [u64],
    now: u64,
) -> bool {
    let pod_half = (i32::from(pod.width / 2), i32::from(pod.half_height / 2));
    for (slot, ready) in ready.iter_mut().enumerate().skip(15) {
        let o = table.slots[slot];
        if o.state < 6 || o.frame == INVISIBLE || now < *ready {
            continue;
        }
        let (w, h) = data.half_size(o.frame);
        let dx = relative(pod.body.x, o.x);
        let dy = relative(pod.body.y, o.y);
        let (Some(px), Some(py)) = (
            penetration(dx, i32::from(w), pod_half.0),
            penetration(dy, i32::from(h), pod_half.1),
        ) else {
            continue;
        };
        *ready = if now - *ready >= STEP {
            now + STEP
        } else {
            *ready + STEP
        };
        // The push: the object moves away by the smaller penetration; the pod's direction on
        // that axis is set away from the object.
        let vertical = py < px;
        {
            let object = &mut table.slots[slot];
            if vertical {
                object.y = if dy < 0 {
                    object.y.wrapping_add(py as u16)
                } else {
                    object.y.wrapping_sub(py as u16)
                };
                pod.body.v_down = dy >= 0;
            } else {
                object.x = if dx < 0 {
                    object.x.wrapping_add(px as u16)
                } else {
                    object.x.wrapping_sub(px as u16)
                };
                pod.body.h_left = dx < 0;
            }
        }
        let (pod_speed, object_speed) = if vertical {
            (pod.body.v_speed(), o.speed_y)
        } else {
            (pod.body.h_speed(), o.speed_x)
        };
        let pod_speed = if pod.invulnerable != 0 {
            // behaviour.md 3.1 and Q11: the remake stops the pod on the axis
            // (docs/deviations.md, "Push while protected").
            0
        } else if pod.equipment.device == 8 {
            pod_speed & 0xFF00
        } else {
            pod_speed.wrapping_add(object_speed / 2) & 0x03FF
        };
        if vertical {
            pod.body.set_v_speed(pod_speed);
        } else {
            pod.body.set_h_speed(pod_speed);
        }
        if pod.invulnerable != 0 {
            continue;
        }
        let flags = data.flags(o.state);
        if flags & 0x40 != 0 {
            table.destroy(data, slot, score);
        } else if flags & 1 == 0 {
            let object = &mut table.slots[slot];
            object.hit_points = object.hit_points.saturating_sub(1);
            if object.hit_points == 0 {
                table.destroy(data, slot, score);
            } else {
                // It survives: the bump sounds (behaviour.md 3.1).
                table.sfx.push(Request::a(Channel::Pulse1, 0x16));
            }
        } else {
            table.sfx.push(Request::a(Channel::Pulse1, 0x16));
        }
        pod.hurt_by_object(flags);
        return true;
    }
    false
}

/// Shots against bonus objects and placed objects (behaviour.md 3.3, 3.4).
fn shots(
    table: &mut ObjectTable,
    pod: &mut Pod,
    data: &ObjectData,
    rng: &Rng,
    score: &mut Score,
    tag: &mut u8,
) {
    let ph_below = i32::from(pod.half_height / 2);
    for shot_slot in SHOT_SLOTS {
        let shot = table.slots[shot_slot];
        if shot.state < 7 {
            continue;
        }
        // Bonus objects and fuel pods (item kind 11): a box of -6..=5 px around the target.
        let mut collected = false;
        for slot in ITEM_SLOTS {
            let target = table.slots[slot];
            let fuel_pod = target.state == state::ITEM && target.kind == 11;
            if !fuel_pod && !matches!(target.state, state::BONUS_STAR | state::BONUS_BOX) {
                continue;
            }
            let near = |a: u16, b: u16| (-6..=5).contains(&i32::from(relative(a, b)));
            if near(shot.x, target.x) && near(shot.y, target.y) {
                if fuel_pod {
                    // It explodes, is gone from the items list and fills the tank.
                    table.explode_in_place(slot, state::EXPLOSION_SMALL, 0);
                    // The fuel pod's explosion sounds $04 on the noise channel (Mesen).
                    table.sfx.push(Request::a(Channel::Noise, 0x04));
                    if let Some(entry) = table.items.get_mut(usize::from(target.index)) {
                        entry.kind = 0;
                    }
                    pod.fuel = 0xE000;
                    collected = true;
                    continue;
                }
                table.slots[slot].state = 0;
                pick_up(table, pod, data, score, tag, target);
                collected = true;
            }
        }
        if collected {
            continue;
        }
        // Placed objects: only the first overlapping one gets the hit.
        for slot in PLACED_SLOTS {
            let target = table.slots[slot];
            if target.state < 6 || target.frame == INVISIBLE {
                continue;
            }
            let (w, h) = data.half_size(target.frame);
            let dx = relative(shot.x, target.x);
            let dy = relative(shot.y, target.y);
            let ph = if dy < 0 { 2 } else { ph_below };
            let (Some(px), Some(py)) = (
                penetration(dx, i32::from(w), 2),
                penetration(dy, i32::from(h), ph),
            ) else {
                continue;
            };
            let deflected = hit_response(table, data, slot, shot_slot, (px, py), rng, score);
            if !deflected {
                if !matches!(shot.state, 0x12 | 0x14 | 0xBA) {
                    table.explode_shot(shot_slot);
                }
                return;
            }
            break;
        }
    }
}

/// The response of a placed object to a shot (behaviour.md 3.5). Returns whether the shot was
/// deflected.
#[allow(clippy::too_many_arguments)]
fn hit_response(
    table: &mut ObjectTable,
    data: &ObjectData,
    slot: usize,
    shot_slot: usize,
    (px, py): (i32, i32),
    rng: &Rng,
    score: &mut Score,
) -> bool {
    let target = table.slots[slot];
    let shot = table.slots[shot_slot];
    let flags = data.flags(target.state);
    let damage = shot.hit_points;
    let mut deflect = false;
    if flags & 0x80 != 0 {
        match flags & 15 {
            // Turrets: destroyed only while open.
            0 => {
                if matches!(target.work6, 2 | 3) {
                    table.destroy(data, slot, score);
                } else {
                    deflect = true;
                }
            }
            // Ground guns: only a shot coming from the front kills.
            1 => {
                if shot.left() == target.left() {
                    deflect = true;
                } else {
                    table.destroy(data, slot, score);
                }
            }
            2 | 3 => {
                let differ = if flags & 15 == 2 {
                    shot.down() != target.down()
                } else {
                    shot.left() != target.left()
                };
                if differ {
                    deflect = true;
                } else {
                    wound(table, data, slot, damage, score);
                }
            }
            // The egg gives an exit item at its position, then is destroyed.
            6 => {
                table.add_item(20, target.x, target.y);
                table.destroy(data, slot, score);
            }
            // The final level's objects (behaviour.md 3.5, 4.16): $B4-$B6 split into pieces,
            // the core ($B7) takes a hit only while open.
            4 => {
                if table.split_object(data, slot) {
                    table.destroy(data, slot, score);
                }
            }
            5 => {
                if target.frame == 0xB6 {
                    table.core_hit = true;
                    wound(table, data, slot, 1, score);
                } else {
                    deflect = true;
                }
            }
            _ => deflect = true,
        }
    } else {
        if flags & 8 != 0 {
            table.release_bonus(data, slot);
        }
        // Q16: the "no damage" flag protects against the shots too.
        if flags & 1 == 0 {
            let destroyed = wound(table, data, slot, damage, score);
            if !destroyed && flags & 4 != 0 {
                let target = &mut table.slots[slot];
                target.speed_x = 0x0600;
                target.speed_y = 0x0600;
                target.flags_x = (target.flags_x & !1) | (shot.flags_x & 1);
                target.flags_y = (target.flags_y & !1) | (shot.flags_y & 1);
            }
        }
    }
    if deflect {
        table.sfx.push(Request::b(Channel::Pulse1, 0x0E));
        let r2 = rng.low;
        let shot = &mut table.slots[shot_slot];
        // behaviour.md 3.5: R2 + 0 (horizontal flip) or R2 + 1 (vertical flip) is added to the
        // fraction byte of the speed on the other axis; the byte wraps.
        if px < py {
            shot.flags_x ^= 1;
            shot.speed_y = shot.speed_y & 0xFF00 | u16::from((shot.speed_y as u8).wrapping_add(r2));
        } else {
            shot.flags_y ^= 1;
            shot.speed_x = shot.speed_x & 0xFF00
                | u16::from((shot.speed_x as u8).wrapping_add(r2).wrapping_add(1));
        }
        // The star parent, the multi warhead and the companion bounce and fly on.
        if !matches!(shot.state, 0x12 | 0x14 | 0xBA) {
            shot.state = state::SPARK;
        }
    }
    deflect
}

/// Takes `damage` hit points off an object; at 0 it is destroyed. Returns whether it was.
fn wound(
    table: &mut ObjectTable,
    data: &ObjectData,
    slot: usize,
    damage: u8,
    score: &mut Score,
) -> bool {
    let target = &mut table.slots[slot];
    target.hit_points = target.hit_points.saturating_sub(damage);
    if target.hit_points == 0 {
        table.destroy(data, slot, score);
        true
    } else {
        false
    }
}

/// A star or a box is collected (behaviour.md 3.3, economy.md 2.5), by a shot or a touch: the
/// score, the weapon energy for a box, the pickup tag and the floating marker. The caller frees
/// the slot. A star collected by touch gets a marker its own tag frees at once (quirk Q22); the
/// remake shows the sign for both.
#[allow(clippy::too_many_arguments)]
pub(super) fn pick_up(
    table: &mut ObjectTable,
    pod: &mut Pod,
    data: &ObjectData,
    score: &mut Score,
    tag: &mut u8,
    target: super::objects::Object,
) {
    table.sfx.push(Request::a(Channel::Pulse1, 0x42));
    let star = target.state == state::BONUS_STAR;
    if star {
        score.add(50);
        *tag = 0x41;
    } else {
        score.add(25);
        pod.weapon_energy = pod.weapon_energy.saturating_add(0x1F).min(0xE0);
        *tag = 0x40;
    }
    let number = if star { 1 } else { 0 };
    table.make_marker(data, number, (target.x, target.y));
}

impl ObjectTable {
    /// The floating "bonus" marker of a collected bonus object (behaviour.md 3.3, 6.2): in the
    /// first free slot searched from 10 down to 3, invisible, rising at 1 px a step.
    pub fn make_marker(&mut self, data: &ObjectData, number: u8, at: (u16, u16)) {
        let slot = SHOT_SLOTS.rev().find(|&s| self.slots[s].state == 0);
        let Some(slot) = slot else { return };
        // Spec gap: the marker's position is not given; it starts at the bonus object.
        let marker = self.create(data, slot, state::MARKER, INVISIBLE, at.0, at.1);
        marker.hit_points = 0;
        marker.index = number;
        marker.speed_y = 0x0100;
    }
}
