//! The pilot (`src/game/pilot`) on the real levels, flying both engines. They come from the
//! player's ROM, so these tests skip without one.

use super::test_world;
use crate::game::gameplay::economy::Transition;
use crate::game::gameplay::{Gameplay, Seed};
use crate::game::level::World;
use crate::game::pilot::nav::{NavMap, UNREACHED};
use crate::game::pilot::{Pilot, bay};
use crate::game::scaled::Scaled;

/// The attract demo's pod: shields and boosters, with a weapon fitted.
fn seed(weapon: u8) -> Seed {
    Seed {
        lives: 4,
        jetpods: 4,
        equipment: 0xC0,
        weapon,
        ..Seed::default()
    }
}

#[test]
fn the_map_is_open_around_the_mothership_and_shut_in_the_rock() {
    let Some(world) = test_world() else { return };
    let level = &world.levels[6];
    let home = (i32::from(level.home.x), i32::from(level.home.y));
    let map = NavMap::new(world, level, (level.home.x, level.home.y - 3), bay(home));
    // Above the mothership is open sky, the hill under it rock; the bay is shut.
    assert!(map.clearance_at(home.0 as f32, (home.1 - 96) as f32) >= 4);
    assert_eq!(map.clearance_at(home.0 as f32, (home.1 + 120) as f32), 0);
    assert_eq!(map.clearance_at(home.0 as f32, home.1 as f32), 0);
    // The caves below are reached the long way round, and a path leads down to them.
    let field = map.field((home.0, home.1 - 96), 2).unwrap();
    let item = level.objects.iter().find(|p| p.kind == 16).unwrap();
    assert_ne!(
        field.cost_at(&map, f32::from(item.x), f32::from(item.y)),
        UNREACHED
    );
    let path = field.path(&map, (f32::from(item.x), f32::from(item.y)), 20);
    assert_eq!(path.len(), 20);
    assert!(
        path.windows(2)
            .all(|pair| field.cost(&map, pair[1]) < field.cost(&map, pair[0]))
    );
}

/// Flies `level` with the pilot until a scene change, at most `steps` logic steps; returns
/// the scene change and the step it came at.
fn fly(
    world: &World,
    level: usize,
    weapon: u8,
    rate: Option<u32>,
    steps: f64,
) -> Option<(Transition, f64)> {
    let mut pilot = Pilot::new();
    let mut original = Gameplay::new(world, level, seed(weapon));
    let mut scaled = Scaled::new(world, level, seed(weapon));
    loop {
        let seen = match rate {
            None => original.observe(),
            Some(_) => scaled.observe(),
        };
        if seen.time > steps || seen.state == 2 {
            return None;
        }
        let buttons = pilot.decide(world, &seen);
        let transition = match rate {
            None => {
                original.pass(world, buttons);
                original.transition()
            }
            Some(rate) => {
                scaled.tick(world, buttons, buttons, rate);
                scaled.transition()
            }
        };
        if let Some(transition) = transition {
            return Some((transition, seen.time));
        }
    }
}

#[test]
fn the_pilot_tows_an_item_home_in_either_engine() {
    let Some(world) = test_world() else { return };
    // Level 4, the second demo's: a cargo container, in about 45 seconds.
    for rate in [None, Some(60), Some(144)] {
        let (transition, step) = fly(world, 4, 8, rate, 120.0 * 30.0).expect("a delivery");
        assert!(
            matches!(transition, Transition::Delivery(delivery) if delivery.kind == 9),
            "{rate:?}: {transition:?}"
        );
        assert!(step < 60.0 * 30.0, "{rate:?}: {step}");
    }
}
