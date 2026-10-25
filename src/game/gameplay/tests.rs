//! Object behaviour and collisions on a small synthetic data set (`docs/spec/behaviour.md`,
//! `docs/spec/objects.md`). The real data are checked by the replays (`sj-tools replay-compare`).

use super::collide::collide;
use super::input::Buttons;
use super::objdata::{
    FLAG_ENTRIES_HIGH, FLAG_ENTRIES_LOW, HIT_POINT_ENTRIES, ObjectData, SIZED_FRAMES, TYPE_COUNT,
};
use super::objects::{Env, ITEM_SLOTS, ObjectTable, Score, state};
use super::pod::Pod;
use super::rng::Rng;
use super::{Gameplay, Seed};
use crate::game::hex::HexRows;
use crate::game::level::tests::synthetic_world;
use crate::game::level::{Placement, Point, World};
use crate::game::view::Viewport;

/// A world whose level 0 has a turret (type 5), a generator (type 0x49) and an egg (type 0x5F)
/// at known places, with the real values of the tables for those states.
fn world() -> World {
    let mut world = synthetic_world();
    let mut data: ObjectData = world.objects.clone();
    let hit = |data: &mut ObjectData, state: u8, value: u8| {
        let index = if state < 0x80 {
            usize::from(state)
        } else {
            usize::from(state) - 0x5E
        };
        data.hit_points.0[index] = value;
    };
    data.types[5] = (0x24, 0x8A);
    data.types[0x49] = (0xFF, 0xAE);
    data.types[0x5F] = (0x9A, 0xB8);
    hit(&mut data, 0x8A, 3);
    hit(&mut data, 0xAE, 255);
    hit(&mut data, 0xB8, 1);
    hit(&mut data, 0x15, 1);
    hit(&mut data, 0x1E, 4);
    data.flags_low.0[0x15] = 0x61;
    data.flags_low.0[0x1E] = 0x04;
    data.flags_high.0[0x0A] = 0x80;
    data.flags_high.0[0x38] = 0x86;
    for frame in [0x24, 0x25, 0x26] {
        data.half_width.0[frame] = 6;
        data.half_height.0[frame] = 6;
    }
    data.half_width.0[0x33] = 1;
    data.half_height.0[0x33] = 1;
    data.half_width.0[0x9A] = 4;
    data.half_height.0[0x9A] = 4;
    data.placements[0] = vec![
        Placement {
            x: 300,
            y: 500,
            kind: 5,
        },
        Placement {
            x: 700,
            y: 100,
            kind: 0x49,
        },
        Placement {
            x: 20,
            y: 20,
            kind: 0x5F,
        },
    ];
    world.objects = data;
    // Level 0 of the synthetic world: margin 64, a 2 x 2 chunk map (256 x 384 px).
    world
}

fn table(world: &World) -> ObjectTable {
    ObjectTable::new(&world.objects, &world.levels[0])
}

/// Runs the A-pass sweep on a table with the pod at (`pod_x`, `pod_y`).
fn sweep(table: &mut ObjectTable, world: &World, pod: &Pod, rng: &mut Rng, counter: u32) -> Score {
    sweep_rules(table, world, &mut pod.clone(), rng, counter, true)
}

/// The same with the pod open to change and the rules chosen: the original's (validation
/// mode, `Gameplay::set_original_rules`) or the remake's.
fn sweep_rules(
    table: &mut ObjectTable,
    world: &World,
    pod: &mut Pod,
    rng: &mut Rng,
    counter: u32,
    original_rules: bool,
) -> Score {
    let mut score = Score(0);
    let mut env = Env {
        data: &world.objects,
        level: &world.levels[0],
        camera: Point { x: 0, y: 0 },
        view: Viewport::original(Point { x: 0, y: 0 }),
        pod,
        rng,
        counter,
        score: &mut score,
        original_rules,
        bonus_tag: 0,
        beam_spin: None,
        beam_slot: 0,
    };
    table.sweep(&mut env);
    score
}

fn pod_at(world: &World, x: u16, y: u16) -> Pod {
    let mut pod = Pod::at_level_start(&world.levels[0]);
    pod.body.x = x;
    pod.body.y = y;
    pod
}

#[test]
fn the_start_scan_spawns_what_is_in_range_and_the_step_scan_the_rest() {
    let world = world();
    let level = &world.levels[0];
    let mut table = table(&world);
    // Camera (0, 0), margin 64: the egg at (20, 20) is in range, the others are not.
    table.start_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 0, y: 0 }),
    );
    assert_eq!(table.slots[23].state, 0xB8);
    assert_eq!((table.slots[23].index, table.slots[23].kind), (2, 0x5F));
    assert_eq!(table.slots[23].hit_points, 1);
    // The step scan never spawns what is on screen; (300, 500) is 220 below the camera after
    // moving: from camera (100, 280) the turret is at (200, 220), on screen: no spawn.
    table.step_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 100, y: 280 }),
        false,
    );
    table.step_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 100, y: 280 }),
        false,
    );
    table.step_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 100, y: 280 }),
        false,
    );
    assert!(table.slots[24].state == 0, "{:?}", table.slots[24]);
    // From camera (100, 200) it is at (200, 300): off screen below, within the margin 64 of 256.
    // Spawned at once, in the lowest free slot of 23-30.
    table.cursor = 0;
    table.step_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 100, y: 200 }),
        false,
    );
    assert_eq!(table.slots[24].state, 0x8A);
    assert_eq!(table.slots[24].hit_points, 3);
    // A live object is not spawned twice.
    table.cursor = 0;
    table.step_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 100, y: 200 }),
        false,
    );
    assert_eq!(table.slots[25].state, 0);
}

#[test]
fn despawning_follows_the_window_and_respawn_brings_fresh_values() {
    let world = world();
    let level = &world.levels[0];
    let mut table = table(&world);
    table.start_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 0, y: 0 }),
    );
    table.slots[23].hit_points = 7;
    // Margin 64: kept while -64 <= rx < 320 (frame not $FF).
    table.despawn(level, Viewport::original(Point { x: 0, y: 0 }));
    assert_ne!(table.slots[23].state, 0);
    table.despawn(level, Viewport::original(Point { x: 100, y: 0 }));
    // rx = 20 - 100 = -80 < -64: freed.
    assert_eq!(table.slots[23].state, 0);
    // It spawns again from the list with fresh values once in range.
    table.start_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 0, y: 0 }),
    );
    assert_eq!(table.slots[23].hit_points, 1);
}

#[test]
fn a_wider_view_widens_the_windows() {
    // docs/widescreen.md: the windows run from the view's left edge to its width.
    let world = world();
    let level = &world.levels[0];
    let mut table = table(&world);
    table.create(&world.objects, 24, 0x8A, 0x24, 400, 100);
    let wide = Viewport {
        left: 0,
        top: 0,
        width: 456,
    };
    // Margin 64: rx 400 is kept within 456 + 64, freed beyond the original's 256 + 64.
    table.despawn(level, wide);
    assert_ne!(table.slots[24].state, 0);
    table.despawn(level, Viewport::original(Point { x: 0, y: 0 }));
    assert_eq!(table.slots[24].state, 0);
    // Drawn within 32 px of the view's sides.
    let object = crate::game::gameplay::objects::Object {
        state: 0x8A,
        frame: 0x24,
        x: 470,
        y: 100,
        ..Default::default()
    };
    assert!(ObjectTable::is_drawn(&object, wide));
    assert!(!ObjectTable::is_drawn(
        &object,
        Viewport::original(Point { x: 0, y: 0 })
    ));
}

#[test]
fn a_turret_opens_after_64_sweeps_and_fires_after_its_waits() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 23, 0x8A, 0x24, 100, 100);
    let pod = pod_at(&world, 10, 10);
    let mut rng = Rng {
        out: 7,
        high: 0,
        low: 0,
    };
    let mut open_at = None;
    let mut shot_at = None;
    for step in 1..=120u32 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step - 2);
        if open_at.is_none() && table.slots[23].work6 == 1 {
            open_at = Some(step);
        }
        if shot_at.is_none() && table.slots[15].state == state::ENEMY_SHOT {
            shot_at = Some(step);
        }
    }
    // Phase 0 lasts 64 sweeps from a fresh turret (behaviour.md 4.1).
    assert_eq!(open_at, Some(64));
    // Open: base + 1 on every 4th step, +2 at the second; then (R mod 16) + 4 = 11 and 4
    // steps of waiting; the shot comes out when the second wait ends.
    assert!(shot_at.is_some());
    let shot = table.slots[15];
    assert_eq!(shot.frame, 0x33);
    // Type 05 (state $8A): down-right shots at 0.5, 1.5 from (+5, +8).
    assert_eq!((shot.speed_x, shot.speed_y), (0x0080, 0x0180));
    assert!(shot.down() && !shot.left());
}

#[test]
fn enemy_shots_fly_and_expire() {
    let world = world();
    let mut table = table(&world);
    let shot = table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 100, 100);
    shot.speed_x = 0x0180;
    shot.speed_y = 0x0200;
    shot.flags_x = 1;
    shot.index = 3;
    let pod = pod_at(&world, 10, 10);
    let mut rng = Rng::default();
    sweep(&mut table, &world, &pod, &mut rng, 0);
    let shot = table.slots[15];
    // Left 1.5 and down... flags Y 0 is up: 2.0 up.
    assert_eq!((shot.x, shot.x_fraction), (98, 0x80));
    assert_eq!(shot.y, 98);
    sweep(&mut table, &world, &pod, &mut rng, 2);
    sweep(&mut table, &world, &pod, &mut rng, 4);
    assert_eq!(table.slots[15].state, 0, "freed when the lifetime runs out");
}

#[test]
fn the_egg_hatches_nothing_and_goes_after_64_steps() {
    let world = world();
    let level = &world.levels[0];
    let mut table = table(&world);
    table.start_scan(
        &world.objects,
        level,
        Viewport::original(Point { x: 0, y: 0 }),
    );
    let pod = pod_at(&world, 10, 10);
    let mut rng = Rng::default();
    for step in 1..=63u32 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step);
    }
    assert_eq!(table.slots[23].frame, 0x9A + 3);
    sweep(&mut table, &world, &pod, &mut rng, 200);
    assert_eq!(table.slots[23].state, 0);
    assert_eq!(table.present[2], 0, "and it never comes back");
}

#[test]
fn a_generator_acts_every_8th_step_and_makes_a_mine_when_the_numbers_fit() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 23, 0xAE, 0xFF, 700, 100);
    let pod = pod_at(&world, 60, 60);
    // R + $61 must be $1F mod 64 and ($5D / 256) even: R = $BE gives $11F = 31 mod 64.
    let mut rng = Rng {
        out: 0xBE,
        high: 0x10,
        low: 0x06,
    };
    for step in 1..=7u32 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step);
        assert_eq!(rng.out, 0xBE, "nothing before the 8th step");
    }
    sweep(&mut table, &world, &pod, &mut rng, 16);
    assert_eq!(rng.out, 0x1F);
    let mine = table
        .slots
        .iter()
        .position(|o| o.state == state::MINE)
        .expect("a mine");
    assert!((27..=30).contains(&mine));
    assert_eq!(table.slots[mine].frame, 0x9A);
    assert_eq!(table.slots[mine].hit_points, 4);
}

#[test]
fn a_mine_homes_on_the_pod() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 27, state::MINE, 0x9A, 100, 100);
    let pod = pod_at(&world, 200, 50);
    let mut rng = Rng::default();
    for step in 1..=10u32 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step);
    }
    let mine = table.slots[27];
    assert!(mine.x > 100 && mine.y < 100, "{mine:?}");
}

#[test]
fn the_pod_fires_a_shot_ahead_with_its_own_speed_added() {
    let world = world();
    let mut table = table(&world);
    let mut pod = pod_at(&world, 100, 100);
    pod.facing = 8;
    pod.body.h_speed = 0x0280;
    assert!(table.fire_pod_shot(&world.objects, &pod, 1, true));
    let shot = table.slots[3];
    // Facing right: 5.00 straight ahead plus the pod's 2.80 (player.md 10).
    assert_eq!(shot.speed_x, 0x0780);
    assert_eq!(shot.state, state::BULLET);
    assert_eq!((shot.index, shot.frame, shot.hit_points), (32, 0x37, 1));
    assert_eq!((shot.x, shot.y), (110, 100));
    // Slots fill up to 10; the ninth press finds none free (no sparks to take over).
    for _ in 0..7 {
        assert!(table.fire_pod_shot(&world.objects, &pod, 1, true));
    }
    assert!(!table.fire_pod_shot(&world.objects, &pod, 1, true));
}

#[test]
fn sparks_take_the_highest_free_slots_and_use_the_generator() {
    let world = world();
    let mut table = table(&world);
    let mut rng = Rng {
        out: 0b0101_0110,
        ..Rng::default()
    };
    table.make_sparks(&world.objects, &mut rng, &[(0, 0x80, (50, 60))], true);
    for slot in [10, 9, 8] {
        let spark = table.slots[slot];
        assert_eq!(spark.state, state::SPARK);
        assert_eq!(
            (spark.x, spark.y, spark.index, spark.frame),
            (50, 60, 0x80, 0x62)
        );
    }
    assert_eq!(table.slots[7].state, 0);
    // Speeds (1.80, 1.40), (0.40, 1.C0), (1.80, 0.40) (player.md 5.5).
    assert_eq!(
        (table.slots[10].speed_x, table.slots[10].speed_y),
        (0x0180, 0x0140)
    );
    assert_eq!(
        (table.slots[9].speed_x, table.slots[9].speed_y),
        (0x0040, 0x01C0)
    );
    assert_ne!(rng.out, 0b0101_0110, "six rotations changed the byte");
    // A spark lives until its counter counts up to 144.
    let pod = pod_at(&world, 10, 10);
    let mut rng = Rng::default();
    for step in 0..16 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step);
    }
    assert_eq!(table.slots[10].state, 0, "0x80 + 16 = 144");
}

#[test]
fn explosions_follow_their_sequence_and_destroying_scores() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 23, 0x8A, 0x24, 100, 100);
    let mut score = Score(0);
    table.destroy(&world.objects, 23, &mut score);
    // Half sizes 6 + 6 = 12: a medium explosion, state 5, frames 4-8 (objects.md 2.5), +5.
    assert_eq!(table.slots[23].state, 5);
    assert_eq!(score, Score(5));
    assert_eq!(
        table.present,
        vec![0, 0x49, 0x5F],
        "the entry of list index 0 is cleared"
    );
    // The shot of a destroyed enemy is a small explosion in 4 steps per frame.
    table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 1, 1);
    table.destroy(&world.objects, 15, &mut score);
    assert_eq!(table.slots[15].state, 4);
    assert_eq!(score, Score(5), "slots below 23 give no points");
}

#[test]
fn items_spawn_one_per_call_and_wrap_with_the_cursor() {
    let mut world = world();
    world.levels[0].objects = vec![
        Placement {
            x: 10,
            y: 10,
            kind: 24,
        },
        Placement {
            x: 5000,
            y: 10,
            kind: 3,
        },
        Placement {
            x: 20,
            y: 30,
            kind: 10,
        },
    ];
    let level = &world.levels[0];
    let mut table = ObjectTable::new(&world.objects, level);
    let camera = Viewport::original(Point { x: 0, y: 0 });
    table.item_scan(&world.objects, camera);
    assert_eq!(table.slots[11].state, state::MOTHERSHIP);
    assert_eq!(table.slots[11].frame, 0x8C);
    // The next call examines the far entry, then entry 2, which is in range.
    table.item_scan(&world.objects, camera);
    assert_eq!((table.slots[12].kind, table.slots[12].frame), (10, 0x5C));
    // Passing the last entry ends the call, which wraps to 0; entry 0 is live.
    table.item_scan(&world.objects, camera);
    assert_eq!(table.item_cursor, 1);
    assert!(
        ITEM_SLOTS
            .clone()
            .all(|s| table.slots[s].state != 0 || s > 12)
    );
}

#[test]
fn the_pod_pushes_an_object_and_takes_damage_by_its_flags() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 103, 100);
    let mut pod = pod_at(&world, 100, 100);
    let mut score = Score(0);
    let rng = Rng::default();
    collide(
        &mut table,
        &mut pod,
        &world.objects,
        &rng,
        &mut score,
        &mut 0,
        false,
    );
    // Flags $61: destroyed by the touch; unshielded it empties the tank.
    assert_eq!(table.slots[15].state, 4);
    assert_eq!(pod.fuel, 0);

    // Shielded, a shot costs $0800 (behaviour.md 3.1).
    let mut table = table_with_shot(&world);
    let mut pod = pod_at(&world, 100, 100);
    pod.shield = true;
    collide(
        &mut table,
        &mut pod,
        &world.objects,
        &rng,
        &mut score,
        &mut 0,
        false,
    );
    assert_eq!(pod.fuel, 0xE000 - 0x0800);
    // An invulnerable pod only pushes: no damage, and the speed on the axis is 0.
    let mut table = table_with_shot(&world);
    let mut pod = pod_at(&world, 100, 100);
    pod.invulnerable = 5;
    pod.body.h_speed = 0x0100;
    collide(
        &mut table,
        &mut pod,
        &world.objects,
        &rng,
        &mut score,
        &mut 0,
        false,
    );
    assert_eq!(pod.fuel, 0xE000);
    assert_eq!(table.slots[15].state, state::ENEMY_SHOT);
}

#[test]
fn a_vertical_push_on_a_protected_pod_has_two_rules() {
    // behaviour.md 3.1, Q11. Pod 3 px above the object: the original leaves the whole part
    // (0 + $FD) AND 3 = 1, fraction 0, moving up; the remake stops the pod.
    let world = world();
    let rng = Rng::default();
    let mut score = Score(0);
    for (original, expected) in [(true, 0x0100), (false, 0)] {
        let mut table = table(&world);
        table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 100, 103);
        let mut pod = pod_at(&world, 100, 100);
        pod.invulnerable = 5;
        pod.body.v_speed = 0x0080;
        pod.body.v_down = true;
        collide(
            &mut table,
            &mut pod,
            &world.objects,
            &rng,
            &mut score,
            &mut 0,
            original,
        );
        assert_eq!(pod.body.v_speed, expected, "original rules: {original}");
        assert!(!pod.body.v_down, "pushed away, up");
        assert_eq!(pod.fuel, 0xE000);
    }
}

fn table_with_shot(world: &World) -> ObjectTable {
    let mut table = table(world);
    table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 103, 100);
    table
}

#[test]
fn shots_glance_off_a_closed_turret_and_destroy_an_open_one() {
    let world = world();
    let rng = Rng {
        low: 0x10,
        ..Rng::default()
    };
    let pod = pod_at(&world, 10, 10);
    let make = |work6: u8| {
        let mut table = table(&world);
        let turret = table.create(&world.objects, 23, 0x8A, 0x24, 100, 100);
        turret.work6 = work6;
        let shot = table.create(&world.objects, 3, state::BULLET, 0x37, 100, 100);
        shot.hit_points = 1;
        shot.speed_x = 0x0300;
        (table, pod.clone())
    };
    let (mut table, mut pod_a) = make(0);
    let mut score = Score(0);
    collide(
        &mut table,
        &mut pod_a,
        &world.objects,
        &rng,
        &mut score,
        &mut 0,
        false,
    );
    assert_eq!(table.slots[3].state, state::SPARK, "deflected");
    assert_eq!(table.slots[23].state, 0x8A);
    let (mut table, mut pod_b) = make(2);
    collide(
        &mut table,
        &mut pod_b,
        &world.objects,
        &rng,
        &mut score,
        &mut 0,
        false,
    );
    // Open (work 6 = 2 or 3): destroyed by one hit, whatever its hit points.
    assert_eq!(table.slots[23].state, 5);
    assert_eq!(score, Score(5));
    assert_eq!(table.slots[3].state, 4, "the shot explodes");
}

#[test]
fn the_egg_leaves_an_exit_item_when_shot() {
    let world = world();
    let mut table = table(&world);
    table.create(&world.objects, 23, 0xB8, 0x9A, 50, 50);
    let shot = table.create(&world.objects, 4, state::BULLET, 0x37, 50, 50);
    shot.hit_points = 1;
    let mut pod = pod_at(&world, 200, 200);
    let mut score = Score(0);
    collide(
        &mut table,
        &mut pod,
        &world.objects,
        &Rng::default(),
        &mut score,
        &mut 0,
        false,
    );
    // Half sizes 4 + 4 = 8: a medium explosion; its exit item (kind 20) joins the items list.
    assert_eq!(table.slots[23].state, 5);
    assert_eq!(
        table.items.last().map(|i| (i.kind, i.x, i.y)),
        Some((20, 50, 50))
    );
}

#[test]
fn gameplay_starts_with_the_first_item_and_runs() {
    let world = world();
    let mut game = Gameplay::new(&world, 0, Seed::default());
    assert_eq!(game.score, Score::NEW_GAME);
    for _ in 0..40 {
        game.step(&world, Buttons(Buttons::A));
    }
    // A held: autofire made shots (every 6 steps for pod type 0).
    assert!(
        game.objects.slots[3..=10]
            .iter()
            .any(|o| o.state == state::BULLET)
    );
    let fields = game.object_fields();
    assert!(fields.iter().any(|&(letter, ..)| letter == 's'));
}

// Keep the synthetic tables' sizes in step with the real ones.
#[test]
fn the_synthetic_tables_have_the_documented_sizes() {
    let data = world().objects;
    assert_eq!(data.types.len(), TYPE_COUNT);
    assert_eq!(data.hit_points.len(), HIT_POINT_ENTRIES);
    assert_eq!(data.flags_low.len(), FLAG_ENTRIES_LOW);
    assert_eq!(data.flags_high.len(), FLAG_ENTRIES_HIGH);
    assert_eq!(data.half_width.len(), SIZED_FRAMES);
    let _ = HexRows::<16>(Vec::new());
}

// ---- The objects of levels 1-7 (behaviour.md 2.8, 2.9, 4, 5) ---------------------------------

use super::behave::{max_speed, slow_and_turn_overshoot};
use super::minions::missile_direction;

#[test]
fn homing_limits_are_decoded_from_one_byte() {
    // behaviour.md 2.3: M = $83 is 3.5 px a step, $C2 is 2.75 (whole part M mod 8).
    assert_eq!(max_speed(0x83), 0x0380);
    assert_eq!(max_speed(0xC2), 0x02C0);
}

#[test]
fn magnets_slow_down_past_zero_into_the_other_direction() {
    // behaviour.md 2.2: 5 - 12 overshoots to 7 the other way; exactly 0 also turns.
    let (mut speed, mut flip) = (5u16, false);
    slow_and_turn_overshoot(&mut speed, &mut flip, 12);
    assert_eq!((speed, flip), (7, true));
    let (mut speed, mut flip) = (12u16, false);
    slow_and_turn_overshoot(&mut speed, &mut flip, 12);
    assert_eq!((speed, flip), (0, true));
}

#[test]
fn a_missile_steers_the_way_round_it_does_in_the_original_and_the_way_it_should() {
    // The target is 100 right and 150 below: ay < 2 ax.
    let (missile, target) = ((500, 500), (600, 650));
    // Q13: the original flies straight down here; the remake takes the diagonal.
    assert_eq!(missile_direction(missile, target, true), 4);
    assert_eq!(missile_direction(missile, target, false), 3);
    // A target far below and little right: the remake flies straight down.
    assert_eq!(missile_direction((500, 500), (510, 650), false), 4);
    // Horizontal: ax >= 2 ay is straight right in both.
    assert_eq!(missile_direction((500, 500), (700, 590), true), 2);
    assert_eq!(missile_direction((500, 500), (700, 590), false), 2);
}

fn dome_world() -> World {
    let mut world = world();
    world.objects.flags_high.0[0x17] = 0x20;
    world.objects.hit_points.0[0x97 - 0x5E] = 48;
    world.objects.half_width.0[0x34] = 20;
    world.objects.half_height.0[0x34] = 21;
    world
}

#[test]
fn an_aimed_shot_nearly_level_keeps_its_vertical_sign_only_in_the_remake() {
    // behaviour.md 2.8, Q12: the pod is level with the muzzle and 300 px away; R2 mod 64 = 0
    // takes 32 off the vertical speed, which goes negative.
    for original in [true, false] {
        let world = dome_world();
        let mut table = table(&world);
        table.create(&world.objects, 23, state::DOME, 0x34, 400, 300);
        // work 6 = 1 so the step's decrement makes the aimed shot (work 6 mod 32 = 0).
        table.slots[23].work6 = 1;
        let mut pod = pod_at(&world, 700, 292);
        let mut rng = Rng {
            out: 2,
            high: 0,
            low: 0,
        };
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 4, original);
        // R mod 4 = 2: entry 1, (+21, +8) from the dome: the shot is at y = 308, 16 below the
        // pod: dy = 16 doubled... the vertical speed is below the random offset.
        let shot = table.slots[15];
        assert_eq!(shot.state, state::ENEMY_SHOT);
        if original {
            // A huge magnitude, as the original reads the negative sum.
            assert!(shot.speed_y > 0x8000, "{:#x}", shot.speed_y);
        } else {
            assert!(shot.speed_y < 0x0100, "{:#x}", shot.speed_y);
        }
    }
}

#[test]
fn the_dome_pulls_the_pod_onto_its_row_towards_it_in_the_remake() {
    // behaviour.md 4.7 step 3, Q18: the pod 4 px below the dome's row, 30 px aside, heading up.
    for original in [true, false] {
        let world = dome_world();
        let mut table = table(&world);
        table.create(&world.objects, 23, state::DOME, 0x34, 400, 300);
        table.slots[23].work6 = 0x80;
        let mut pod = pod_at(&world, 430, 304);
        pod.body.v_down = false;
        let mut rng = Rng::default();
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 4, original);
        assert_eq!(
            table.slots[25].state,
            state::FLAME,
            "a flame on the pod's side"
        );
        if original {
            // The direction flips to down and the pod moves 4 px that way: away.
            assert_eq!((pod.body.y, pod.body.v_down), (308, true));
        } else {
            // Always onto the dome's row, heading towards it.
            assert_eq!((pod.body.y, pod.body.v_down), (300, false));
        }
    }
}

#[test]
fn a_lifted_dome_moves_twice_in_the_original_while_it_homes() {
    // behaviour.md 4.7 step 1, Q14: m = (s + F) mod 256 above 128 homes on the pod.
    let mut results = Vec::new();
    for original in [true, false] {
        let world = dome_world();
        let mut table = table(&world);
        table.create(&world.objects, 23, state::DOME_LIFT, 0x34, 400, 300);
        table.slots[23].work5 = 2;
        table.slots[23].work6 = 0x80;
        table.slots[23].speed_x = 0x0100;
        let mut pod = pod_at(&world, 900, 300);
        let mut rng = Rng::default();
        // F = 206, so m = 229. The homing speed is capped at M = $80, 0.5 px a step.
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 206, original);
        results.push(table.slots[23].x);
    }
    // 0.5 px moved once (the remake) or twice (the original).
    assert_eq!(results, vec![401, 400]);
}

#[test]
fn the_large_machines_volley_overwrites_slots_in_the_original_only() {
    // behaviour.md 4.11, Q15: tick 3 (index 2) fires four shots into slots 15-18.
    for original in [true, false] {
        let mut world = world();
        world.objects.hit_points.0[0xA8 - 0x5E] = 16;
        let mut table = table(&world);
        table.create(&world.objects, 23, state::BIG_MACHINE, 0x85, 400, 300);
        // Frame $87 after the next tick's index 2: work 5 = 1 now, work 6 = 1 so the
        // decrement is the tick (work 6 mod 128 = 0 after it).
        table.slots[23].frame = 0x87;
        table.slots[23].work5 = 1;
        table.slots[23].work6 = 1;
        // A shot already in slot 16.
        table.create(&world.objects, 16, state::ENEMY_SHOT, 0x33, 1, 1);
        table.slots[16].index = 9;
        let mut pod = pod_at(&world, 10, 10);
        let mut rng = Rng::default();
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 0, original);
        let lifetimes: Vec<u8> = (15..=18).map(|s| table.slots[s].index).collect();
        if original {
            assert_eq!(lifetimes, vec![0xC0; 4]);
            // The two down-going shots carry the loop counter in their speeds.
            assert_eq!(table.slots[16].speed_x, 0x0301);
            assert_eq!(table.slots[16].speed_y, 0x01D8);
        } else {
            // The shot in slot 16 stays; the others are made with equal speeds.
            assert_eq!(lifetimes, vec![0xC0, 8, 0xC0, 0xC0]);
            assert_eq!(table.slots[17].speed_x, 0x0300);
            assert_eq!(table.slots[18].speed_y, 0x00D8);
        }
    }
}

#[test]
fn shots_wear_an_indestructible_object_down_only_in_the_original() {
    // behaviour.md 3.5, Q16: flags $01 and 255 hit points (a walker).
    for original in [true, false] {
        let mut world = world();
        world.objects.flags_high.0[0x19] = 0x01;
        world.objects.hit_points.0[0x99 - 0x5E] = 255;
        world.objects.half_width.0[0x5D] = 8;
        world.objects.half_height.0[0x5D] = 12;
        let mut table = table(&world);
        table.create(&world.objects, 23, 0x99, 0x5D, 100, 100);
        let shot = table.create(&world.objects, 3, state::BULLET, 0x37, 100, 100);
        shot.hit_points = 8;
        let mut pod = pod_at(&world, 10, 10);
        let rng = Rng::default();
        let mut score = Score(0);
        collide(
            &mut table,
            &mut pod,
            &world.objects,
            &rng,
            &mut score,
            &mut 0,
            original,
        );
        let expected = if original { 247 } else { 255 };
        assert_eq!(table.slots[23].hit_points, expected);
    }
}

#[test]
fn reused_slots_keep_their_fractions_only_in_the_original() {
    // behaviour.md 2.5, Q17.
    for original in [true, false] {
        let world = world();
        let mut table = table(&world);
        table.slots[15].x_fraction = 0x77;
        table.slots[15].y_fraction = 0x33;
        table.slots[15].hit_points = 5;
        // A turret about to fire.
        table.create(&world.objects, 23, 0x8A, 0x24, 100, 100);
        table.slots[23].work6 = 3;
        table.slots[23].work5 = 1;
        let mut pod = pod_at(&world, 10, 10);
        let mut rng = Rng::default();
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 0, original);
        let shot = table.slots[15];
        assert_eq!(shot.state, state::ENEMY_SHOT);
        if original {
            assert_eq!(
                (shot.x_fraction, shot.y_fraction, shot.hit_points),
                (0x77, 0x33, 5)
            );
        } else {
            assert_eq!(
                (shot.x_fraction, shot.y_fraction, shot.hit_points),
                (0, 0, 1)
            );
        }
    }
}

#[test]
fn a_patrolling_creature_turns_back_where_the_route_says_only_in_the_remake() {
    // behaviour.md 2.7, Q1: level 2, list index 25, from (1280, 1627) to (1280, 1920).
    for original in [true, false] {
        let mut world = world();
        world.objects.hit_points.0[0xA7 - 0x5E] = 16;
        let mut level = world.levels[0].clone();
        level.number = 2;
        let mut table = table(&world);
        // Heading for the second waypoint, 270 px below (low byte 14): the original counts
        // it as reached.
        table.create(&world.objects, 23, state::CREATURE, 0x83, 1280, 1650);
        table.slots[23].index = 25;
        table.slots[23].work5 = 0x80;
        let mut pod = pod_at(&world, 10, 10);
        let mut rng = Rng::default();
        let mut score = Score(0);
        let mut env = Env {
            data: &world.objects,
            level: &level,
            camera: Point { x: 0, y: 0 },
            view: Viewport::original(Point { x: 0, y: 0 }),
            pod: &mut pod,
            rng: &mut rng,
            counter: 0,
            score: &mut score,
            original_rules: original,
            bonus_tag: 0,
            beam_spin: None,
            beam_slot: 0,
        };
        table.sweep(&mut env);
        // The target flag flipped (the original) or stays (the remake).
        assert_eq!(table.slots[23].work5 & 0x80 == 0, original);
    }
}

#[test]
fn formation_parts_circle_their_home_a_quarter_turn_in_64_steps() {
    // behaviour.md 4.12: the leader starts at phase 0, then +1 a step; type $23 is part 0.
    let mut world = world();
    world.objects.hit_points.0[0xA9 - 0x5E] = 8;
    let mut table = table(&world);
    table.create(&world.objects, 23, state::FORMATION_A, 0x8D, 200, 200);
    table.slots[23].kind = 0x23;
    let pod = pod_at(&world, 10, 10);
    let mut rng = Rng::default();
    for step in 0..64u32 {
        sweep(&mut table, &world, &pod, &mut rng, 2 * step);
    }
    // Phase 64: a = 64, q = 1, i = 0, j = 63: x = home + T[63], y = home + T[127].
    assert_eq!(
        (table.slots[23].x, table.slots[23].y),
        (200 + 0x3F, 200 + 0x01)
    );
}

// ---- The special weapons (player.md 10, behaviour.md 5.11) ----------------------------------

/// Counter values at which a bomb in `slot` counts a frame: (slot + counter / 2) mod 32 = 0.
fn bomb_counters(slot: usize) -> impl Iterator<Item = u32> {
    let first = (32 - slot as u32 % 32) % 32 * 2;
    (0..).map(move |n| first + 64 * n)
}

#[test]
fn a_time_bomb_goes_off_on_the_fifth_count_and_every_victim_in_range_explodes() {
    for original in [false, true] {
        let world = world();
        let mut table = table(&world);
        let bomb = table.create(&world.objects, 3, state::TIME_BOMB, 0x3D, 500, 500);
        bomb.hit_points = 0x40;
        // In range: (+40, -5), (+63, +58); out: (+64, 0), (-65, 0).
        for (slot, dx, dy) in [(23, 40, -5), (24, 63, 58), (25, 64, 0), (26, -65, 0)] {
            table.create(
                &world.objects,
                slot,
                0x8A,
                0x24,
                500u16.wrapping_add_signed(dx),
                500u16.wrapping_add_signed(dy),
            );
        }
        let mut pod = pod_at(&world, 10, 10);
        let mut rng = Rng::default();
        let mut counters = bomb_counters(3);
        for count in 1..=5 {
            // Steps between the counts do nothing.
            sweep_rules(&mut table, &world, &mut pod, &mut rng, 2, original);
            assert_eq!(table.slots[3].state, state::TIME_BOMB);
            let counter = counters.next().unwrap();
            let score = sweep_rules(&mut table, &world, &mut pod, &mut rng, counter, original);
            if count < 5 {
                assert_eq!(table.slots[3].frame, 0x3D + count);
                assert_eq!(score, Score(0));
            } else {
                // The bomb itself becomes a large explosion; victims score 5 each.
                assert_eq!(table.slots[3].state, state::EXPLOSION_LARGE);
                assert_eq!(score, Score(10));
                // Q29: the original frees each earlier victim's explosion.
                assert_eq!(
                    table.slots[23].state,
                    if original { 0 } else { 5 },
                    "{original}"
                );
                assert_eq!(table.slots[24].state, 5);
                assert_eq!(table.slots[25].state, 0x8A);
                assert_eq!(table.slots[26].state, 0x8A);
            }
        }
    }
}

#[test]
fn a_smart_bomb_destroys_everything_in_use_that_is_visible() {
    for original in [false, true] {
        let world = world();
        let mut table = table(&world);
        table.create(&world.objects, 15, state::ENEMY_SHOT, 0x33, 50, 50);
        table.create(&world.objects, 23, 0x8A, 0x24, 60, 60);
        table.create(&world.objects, 24, 0x8A, 0x24, 70, 70);
        table.create(&world.objects, 25, 0xAE, 0xFF, 80, 80);
        table.create(&world.objects, 5, state::BULLET, 0x37, 90, 90);
        let mut score = Score(0);
        table.smart_bomb(&world.objects, &mut score, original);
        assert_eq!(table.slots[15].state, state::EXPLOSION_SMALL);
        assert_eq!(table.slots[25].state, 0xAE, "invisible objects are spared");
        assert_eq!(
            table.slots[5].state,
            state::BULLET,
            "so are the pod's shots"
        );
        assert_eq!(score, Score(10));
        assert_eq!(table.slots[23].state, if original { 0 } else { 5 });
        assert_eq!(table.slots[24].state, 5);
    }
}

#[test]
fn a_homing_missile_without_a_target_circles_the_pod_only_in_the_original() {
    for original in [false, true] {
        let world = world();
        let mut table = table(&world);
        let missile = table.create(&world.objects, 3, state::MISSILE, 0x27, 100, 100);
        missile.work5 = 0xC0;
        missile.kind = 2;
        missile.speed_x = 0x0100;
        let mut pod = pod_at(&world, 200, 250);
        let mut rng = Rng::default();
        // (slot + counter / 2) mod 8 = 0 at counter 26: the direction is recomputed.
        sweep_rules(&mut table, &world, &mut pod, &mut rng, 26, original);
        // No placed object exists: the pod (original) is below, in play the course holds.
        assert_eq!(table.slots[3].kind, if original { 4 } else { 2 });
        assert_eq!(table.slots[3].index, 0);
    }
}

#[test]
fn star_children_age_in_every_slot_in_play_but_not_in_even_ones_originally() {
    for original in [false, true] {
        let world = world();
        let mut table = table(&world);
        let mut pod = pod_at(&world, 10, 10);
        let mut rng = Rng::default();
        for slot in [3, 4] {
            let child = table.create(&world.objects, slot, state::STAR_CHILD, 0x27, 100, 100);
            child.hit_points = 16;
        }
        // (counter + slot + 1) mod 16 = 0 for slot 3 at 12 and for slot 4 at 11 (never even).
        for counter in (0..64).step_by(2) {
            sweep_rules(&mut table, &world, &mut pod, &mut rng, counter, original);
        }
        assert_ne!(table.slots[3].frame, 0x27, "slot 3 aged ({original})");
        assert_eq!(
            table.slots[4].frame != 0x27,
            !original,
            "slot 4 ages only in play ({original})"
        );
    }
}

#[test]
fn anti_gravity_toggles_with_the_device() {
    let world = world();
    let mut pod = pod_at(&world, 100, 100);
    pod.equipment.weapon = 2;
    assert!(pod.press_special());
    assert!(pod.antigravity && pod.antigravity_active());
    assert!(pod.press_special());
    assert!(!pod.antigravity);
    // Another weapon clears the switch.
    pod.antigravity = true;
    pod.equipment.weapon = 5;
    assert!(!pod.press_special());
    assert!(!pod.antigravity);
}

#[test]
fn a_destroyed_pod_loses_its_fittings() {
    let world = world();
    let mut game = Gameplay::new(&world, 0, Seed::default());
    game.pod.equipment.weapon = 2;
    game.pod.equipment.device = 10;
    game.pod.antigravity = true;
    game.pod.fuel = 0;
    game.step(&world, Buttons::default());
    assert_eq!(game.pod.equipment.weapon, 0);
    assert_eq!(game.pod.equipment.device, 0);
    assert!(!game.pod.antigravity);
}

/// A game on `level` of the synthetic world with a wormhole in item slot 11 at the pod.
fn game_at_wormhole(world: &World, level: usize) -> Gameplay {
    let mut game = Gameplay::new(world, level, Seed::default());
    let (x, y) = (game.pod.body.x, game.pod.body.y);
    game.objects.slots[11] = super::objects::Object {
        state: state::ITEM,
        kind: 18,
        x,
        y,
        frame: 0x52,
        ..Default::default()
    };
    game
}

#[test]
fn an_open_wormhole_draws_the_pod_in_for_193_steps_and_asks_for_a_level_change() {
    let world = world();
    let level = &world.levels[13];
    let mut game = game_at_wormhole(&world, 13);
    game.guard = 0;
    game.touch_wormhole(&world, level, 11);
    assert_eq!(game.pod.state, super::pod::STATE_GATE);
    assert!(game.sfx.contains(&crate::game::sound::Request::Song {
        id: 2,
        always: false
    }));
    let mut steps = 0;
    while game.transition.is_none() {
        game.gate_step(level);
        steps += 1;
        // A step is two frames; the first call left the counter odd, so its pass ends the
        // step at once.
        game.counter += if steps == 1 { 1 } else { 2 };
        assert!(steps < 400, "the gate state does not end");
    }
    assert_eq!(steps, 193);
    assert_eq!(game.pod.state, super::pod::STATE_POD);
    assert_eq!(game.pod.invulnerable, 64);
    assert_eq!(
        game.transition,
        Some(super::economy::Transition::ChangeLevel)
    );
    // The wormhole is gone from the items list for the rest of the visit.
    assert_eq!(game.objects.items.first().map(|e| e.kind), Some(0));
}

#[test]
fn a_refused_touch_does_nothing_and_a_closed_wormhole_bounces() {
    let world = world();
    let level = &world.levels[13];
    // On a second level the first 64 steps after the change are refused.
    let mut game = game_at_wormhole(&world, 13);
    game.guard = 10;
    game.touch_wormhole(&world, level, 11);
    assert_eq!(game.pod.state, super::pod::STATE_POD);
    assert!(game.sfx.is_empty());
    // A planet with the tank not full: a direction bit flips, a sparkle appears in a free
    // shot slot, sound $44 on both pulses, no damage.
    let level = &world.levels[0];
    let mut game = game_at_wormhole(&world, 0);
    game.objects.items[0].kind = 20;
    let before = (game.pod.body.h_left, game.pod.body.v_down);
    game.touch_wormhole(&world, level, 11);
    assert_ne!((game.pod.body.h_left, game.pod.body.v_down), before);
    assert_eq!(game.objects.slots[3].state, state::SPARKLE);
    assert_eq!(game.objects.slots[3].index, 8);
    assert_eq!(game.pod.hit_pause, 1);
    assert_eq!(game.sfx.len(), 2);
    assert_eq!(game.pod.state, super::pod::STATE_POD);
}

#[test]
fn the_sparkle_runs_down_in_eight_steps() {
    let world = world();
    let mut table = table(&world);
    let mut pod = pod_at(&world, 100, 100);
    let mut rng = Rng::default();
    table
        .create(&world.objects, 3, state::SPARKLE, 0x69, 100, 100)
        .index = 8;
    let mut frames = Vec::new();
    for counter in (0..16).step_by(2) {
        sweep(&mut table, &world, &pod, &mut rng, counter);
        frames.push((table.slots[3].state, table.slots[3].frame));
    }
    assert_eq!(frames[0], (state::SPARKLE, 0x6C));
    assert_eq!(frames[6], (state::SPARKLE, 0x69));
    assert_eq!(frames[7].0, 0);
    pod.state = 1;
}

#[test]
fn levels_follow_the_wormholes_and_the_flags() {
    let world = world();
    let mut game = Gameplay::new(&world, 0, Seed::default());
    assert_eq!(game.level_after_change(), 13);
    game = Gameplay::new(&world, 13, Seed::default());
    // The flag of planet 0 clear: back to the planet; set: the warp level.
    assert_eq!(game.level_after_change(), 0);
    game.planet_flags = 1;
    assert_eq!(game.level_after_change(), 29);
    // Planet 12 uses planet 10's flag.
    let mut game = Gameplay::new(&world, 25, Seed::default());
    assert_eq!(game.level_after_change(), 12);
    game.planet_flags = 1 << 10;
    assert_eq!(game.level_after_change(), 41);
    // A warp level always leads to its planet.
    let game = Gameplay::new(&world, 34, Seed::default());
    assert_eq!(game.level_after_change(), 5);
}

#[test]
fn a_planet_reached_from_a_second_level_is_entered_128_px_above_the_mothership() {
    let world = world();
    let mut second = Gameplay::new(&world, 13, Seed::default());
    second.pod.fuel = 0x7000;
    second.pod.body.v_down = true;
    let planet = second.change_level(&world);
    assert_eq!(planet.level(), 0);
    let home = world.levels[0].home;
    assert_eq!(planet.pod.body.x, home.x);
    assert_eq!(planet.pod.body.y, (home.y - 3).saturating_sub(128));
    assert_eq!(
        (planet.pod.body.h_speed, planet.pod.body.v_speed),
        (0, 0x40)
    );
    assert!(planet.pod.body.v_down);
    assert_eq!(planet.pod.fuel, 0x7000);
    // A gate that ended directly in the change leaves 64 steps of protection.
    assert_eq!(planet.pod.invulnerable, 64);
    // A warp level and the planet after it: none.
    second.planet_flags = 1;
    let warp = second.change_level(&world);
    assert_eq!(warp.level(), 29);
    assert_eq!(warp.pod.invulnerable, 0);
    assert_eq!(warp.change_level(&world).pod.invulnerable, 0);
}

#[test]
fn the_badge_counts_a_hundred_a_crystal_and_a_thousand_more_for_the_tenth() {
    assert_eq!(Gameplay::badge_points(0), 0);
    assert_eq!(Gameplay::badge_points(3), 300);
    assert_eq!(Gameplay::badge_points(10), 2000);
}

#[test]
fn the_warp_countdown_shows_a_digit_every_64_steps_and_ends_at_the_705th() {
    let world = world();
    let mut game = Gameplay::new(&world, 33, Seed::default());
    for _ in 0..704 {
        game.step(&world, Buttons::default());
    }
    assert!(game.transition().is_none());
    assert_eq!(game.objects.slots[3].state, state::COUNTDOWN);
    assert_eq!(game.objects.slots[3].frame, 0x7D);
    game.step(&world, Buttons::default());
    assert_eq!(game.transition(), Some(super::economy::Transition::WarpEnd));
}

#[test]
fn the_final_level_is_entered_flying_up_with_nothing_left_to_lose() {
    let world = world();
    let mut game = Gameplay::new(&world, 3, Seed::default());
    game.score = Score(4200);
    game.pod.equipment.owned = 0xC1;
    game.pod.fuel = 0x1000;
    let next = game.enter_final_level(&world);
    assert_eq!(next.level(), 26);
    assert_eq!((next.lives, next.jetpods), (0, 0));
    assert_eq!(next.pod.state, super::pod::STATE_FINAL);
    assert_eq!(next.pod.fuel, super::pod::FULL_FUEL);
    assert_eq!((next.pod.body.h_speed, next.pod.body.v_speed), (0, 0x180));
    assert!(!next.pod.body.v_down);
    assert_eq!(next.score, Score(4200));
    assert_eq!(next.pod.equipment.owned, 0xC1);
    assert_eq!(next.camera.position, world.levels[26].camera_start);
}

#[test]
fn a_piece_delivered_on_planet_12_sets_planet_10s_flag() {
    assert_eq!(super::gate::planet_flag_bit(12), 10);
    assert_eq!(super::gate::planet_flag_bit(25), 10);
    assert_eq!(super::gate::planet_flag_bit(41), 10);
    assert_eq!(super::gate::planet_flag_bit(34), 5);
}

#[test]
fn an_edge_position_carries_into_the_high_byte_only_in_the_remake() {
    // Q10: bit 1 of R2 set (a side edge), bit 2 clear (the right one); R1 = 100 gives an
    // offset of 100 - 64 + 1 = 37 on the pod's y.
    let world = world();
    // A map wide enough for the right edge (1024 px).
    let mut level = world.levels[0].clone();
    (level.width, level.height) = (8, 8);
    let mut rng = Rng {
        high: 100,
        low: 0b010,
        ..Rng::default()
    };
    for original in [false, true] {
        let mut pod = pod_at(&world, 100, 0x01F0);
        let mut score = Score(0);
        let env = Env {
            data: &world.objects,
            level: &level,
            camera: Point { x: 0, y: 0 },
            view: Viewport::original(Point { x: 0, y: 0 }),
            pod: &mut pod,
            rng: &mut rng,
            counter: 0,
            score: &mut score,
            original_rules: original,
            bonus_tag: 0,
            beam_spin: None,
            beam_slot: 0,
        };
        let (_, y) = super::behave::edge_position(&env);
        // 0x01F0 + 37 = 0x0215; the original loses the carry and lands at 0x0115.
        assert_eq!(y, if original { 0x0115 } else { 0x0215 }, "{original}");
    }
}

#[test]
fn edge_positions_follow_the_view_and_fall_back() {
    // behaviour.md 2.10 and objects.md 3.7 rule 7: the view's left edge - 32 or right edge
    // + 32, the other side when the first is off the map, the top or bottom when both are.
    let world = world();
    let edge = |width: u8, left: i16, low: u8| {
        let mut level = world.levels[0].clone();
        (level.width, level.height) = (width, 8);
        let mut pod = pod_at(&world, 100, 300);
        let mut rng = Rng {
            low,
            ..Rng::default()
        };
        let mut score = Score(0);
        let env = Env {
            data: &world.objects,
            level: &level,
            camera: Point { x: 400, y: 200 },
            view: Viewport {
                left: left as u16,
                top: 200,
                width: 456,
            },
            pod: &mut pod,
            rng: &mut rng,
            counter: 0,
            score: &mut score,
            original_rules: false,
            bonus_tag: 0,
            beam_spin: None,
            beam_slot: 0,
        };
        super::behave::edge_position(&env)
    };
    // Width 8 half-chunks: a map 1024 px wide.
    assert_eq!(edge(8, 300, 0b110).0, 268);
    assert_eq!(edge(8, 300, 0b010).0, 300 + 456 + 32);
    assert_eq!(edge(8, 10, 0b110).0, 10 + 456 + 32);
    assert_eq!(edge(8, 600, 0b010).0, 568);
    // Width 2: 256 px, narrower than the view; neither side fits, so the top edge.
    assert_eq!(edge(2, -100, 0b110).1, 200 - 48);
}

#[test]
fn the_mesh_helper_homes_on_levels_37_and_39_in_the_remake_and_is_overwritten_originally() {
    let mut world = world();
    // Level 37 (entry 8): both helpers home ($C0); every source byte is 0, the constant 5.
    world.objects.warp_mesh.0[8] = 0xC0;
    for original in [false, true] {
        let mut game = Gameplay::new(&world, 37, Seed::default());
        game.set_original_rules(original);
        for _ in 0..40 {
            game.step(&world, Buttons::default());
        }
        let [wx, wy, ..] = game.helper_bytes();
        if original {
            // The lookup's value overwrites the low bytes every frame.
            assert_eq!((wx, wy), (5, 5));
        } else {
            // The helper creeps towards the pod: 0.3125 px a frame at most, from (0, 16).
            assert!(wx > 5 && wx < 40, "{wx}");
            assert_ne!(wy, 5);
        }
    }
}

#[test]
fn the_original_mesh_lookup_reads_the_rom_bytes_after_the_tables_by_slot_24s_x() {
    let mut world = world();
    world.objects.warp_mesh.0[8] = 0xC0;
    // Every byte the byte-sized index can reach names the frame counter, the level's own
    // entries (the constant 5) are not used.
    let mut game = Gameplay::new(&world, 37, Seed::default());
    game.set_original_rules(true);
    game.set_validation_input(Some(super::ValidationInput {
        warp_shimmer: vec![5; super::objdata::SHIMMER_BYTES].into(),
    }));
    for _ in 0..10 {
        game.step(&world, Buttons::default());
        let [wx, wy, vx, vy] = game.helper_bytes();
        let counter = game.counter as u8;
        assert_eq!((wx, wy, vx, vy), (counter, counter, counter, counter));
    }
}

#[test]
fn cheats_are_off_by_default_and_keep_their_promises_when_on() {
    let world = world();
    let mut game = Gameplay::new(&world, 0, Seed::default());
    game.pod.fuel = 0x1000;
    game.pod.weapon_energy = 3;
    game.step(&world, Buttons::default());
    assert!(game.pod.fuel < 0x1000 + 1 && game.pod.weapon_energy <= 3);

    let mut game = Gameplay::new(&world, 0, Seed::default());
    game.set_cheats(super::Cheats {
        infinite_fuel: true,
        full_gauge: true,
        infinite_lives: true,
    });
    game.pod.fuel = 0;
    game.pod.weapon_energy = 3;
    game.step(&world, Buttons::default());
    assert_eq!(game.pod.fuel, super::pod::FULL_FUEL);
    assert_eq!(game.pod.weapon_energy, 0xE0);
    assert!(game.pod.free_fuel);
    // Damage takes no fuel, even a touch that would empty the tank, but still makes its sound.
    game.pod.invulnerable = 0;
    game.pod.hurt_by_object(0);
    game.pod.hurt_by_object(0x30);
    assert_eq!(game.pod.fuel, super::pod::FULL_FUEL);
    assert!(!game.pod.sfx.is_empty());
    // The last life is not lost.
    game.lives = 1;
    game.pod.events.push(super::pod::Event::JetmanKilled);
    game.step(&world, Buttons::default());
    assert_eq!(game.lives, 1);
}

#[test]
fn the_view_blends_between_passes() {
    let world = synthetic_world();
    let mut game = Gameplay::new(&world, 0, Seed::default());
    // Free flight in open air: anti-gravity on, with a steady speed down and to the right.
    (game.pod.body.x, game.pod.body.y) = (150, 230);
    game.pod.antigravity = true;
    game.pod.weapon_energy = 0xFF;
    (game.pod.body.h_speed, game.pod.body.v_speed) = (0x0058, 0x0033);
    (game.pod.body.h_left, game.pod.body.v_down) = (false, true);
    game.pass(&world, Buttons::default());
    for _ in 0..20 {
        game.pass(&world, Buttons::default());
        // The pod's place on the screen moves one way between two passes.
        let on_screen = |blend: f32| {
            let (camera, pod) = game.view_at(blend);
            (pod.0 - i32::from(camera.x), pod.1 - i32::from(camera.y))
        };
        let (from, to) = (on_screen(0.0), on_screen(1.0));
        let between = |a: i32, m: i32, b: i32| m >= a.min(b) && m <= a.max(b);
        for blend in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let mid = on_screen(blend);
            assert!(between(from.0, mid.0, to.0) && between(from.1, mid.1, to.1));
        }
        assert!((to.0 - from.0).abs() <= 2, "{from:?} to {to:?}");
    }
}

#[test]
fn after_an_a_pass_objects_are_shown_half_way_through_their_move() {
    let before = [
        None,
        Some((100.0, 50.0, 7, 1)),
        Some((100.0, 50.0, 7, 1)),
        Some((100.0, 50.0, 7, 1)),
        None,
    ];
    let mut now = [
        None,
        // Moved by 3, -1: shown half-way.
        Some((103.0, 49.0, 7, 1)),
        // Jumped: shown where it landed.
        Some((140.0, 50.0, 7, 1)),
        // Another object in the slot: shown where it is.
        Some((103.0, 49.0, 8, 1)),
        // New: shown where it is.
        Some((10.0, 20.0, 7, 1)),
    ];
    super::halfway_objects(&before, &mut now);
    assert_eq!(now[1], Some((101.5, 49.5, 7, 1)));
    assert_eq!(now[2], Some((140.0, 50.0, 7, 1)));
    assert_eq!(now[3], Some((103.0, 49.0, 8, 1)));
    assert_eq!(now[4], Some((10.0, 20.0, 7, 1)));
}
