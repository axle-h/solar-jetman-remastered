//! The pilot's geometry. Its flights on the real levels, which need the player's ROM, are in
//! `src/tools/pilot_tests.rs`.

use super::*;

#[test]
fn facings_point_where_asked() {
    assert_eq!(facing_toward(0.0, -1.0), 0);
    assert_eq!(facing_toward(1.0, 0.0), 8);
    assert_eq!(facing_toward(0.0, 1.0), 16);
    assert_eq!(facing_toward(-1.0, 0.0), 24);
    assert_eq!(facing_toward(1.0, -1.0), 4);
    assert_eq!((turn(30, 2), turn(2, 30), turn(0, 16)), (4, -4, -16));
}

#[test]
fn a_shot_on_course_is_imminent_and_one_passing_by_is_not() {
    let seen = Observation::default();
    let shot = |x, y, vx, vy| Threat {
        x,
        y,
        missile: true,
        state: 0x15,
        vx,
        vy,
    };
    assert!(imminent(&seen, &shot(60.0, 0.0, -2.0, 0.0), 24.0, 20.0));
    assert!(!imminent(&seen, &shot(60.0, 40.0, -2.0, 0.0), 24.0, 20.0));
    // Moving away, or too far off to arrive within the time looked at.
    assert!(!imminent(&seen, &shot(60.0, 0.0, 2.0, 0.0), 24.0, 20.0));
    assert!(!imminent(&seen, &shot(200.0, 0.0, -2.0, 0.0), 24.0, 20.0));
}
