use super::clock::STEP;
use super::*;
use crate::game::gameplay::Gameplay;
use crate::game::level::tests::synthetic_world;

#[test]
fn the_level_starts_where_the_original_starts_it() {
    let world = synthetic_world();
    let original = Gameplay::new(&world, 0, Seed::default());
    let scaled = Scaled::new(&world, 0, Seed::default());
    assert_eq!(scaled.level(), 0);
    assert_eq!(scaled.clock().time(), 0);
    assert_eq!(scaled.counter(), original.counter());
    let (x, y) = scaled.pod_q16();
    assert_eq!(
        ((x >> 16) as u16, (y >> 16) as u16),
        (original.pod.body.x, original.pod.body.y)
    );
    assert_eq!(scaled.camera.position, original.camera.position);
    assert_eq!(scaled.hud(), original.hud());
}

#[test]
fn a_saved_state_plays_on_at_another_rate() {
    let world = synthetic_world();
    let mut scaled = Scaled::new(&world, 0, Seed::default());
    for _ in 0..37 {
        scaled.tick(&world, Buttons(0), Buttons(0), 144);
    }
    let text = ron::ser::to_string(&scaled).unwrap();
    let mut loaded: Scaled = ron::de::from_str(&text).unwrap();
    assert_eq!(loaded.clock(), scaled.clock());
    assert_eq!(loaded.counter(), scaled.counter());
    let time = loaded.clock().time();
    loaded.tick(&world, Buttons(0), Buttons(0), 60);
    assert_eq!(loaded.clock().time(), time + STEP / 2);
}

#[test]
fn what_hangs_on_the_pod_is_drawn_with_it_between_steps() {
    let world = synthetic_world();
    let mut scaled = Scaled::new(&world, 0, Seed::default());
    for _ in 0..10 {
        scaled.tick(&world, Buttons(0), Buttons(0), 144);
    }
    let (x, y) = (scaled.pod.body.x, scaled.pod.body.y);
    scaled.set_object(11, objects::state::ITEM, x + 30, y + 10, 0, 1);
    scaled.tow.catch(11, 2, 30, 10);
    let beam = &mut scaled.objects.slots[2];
    (beam.state, beam.x, beam.y) = (objects::state::BEAM, x, y);
    let swing = Buttons(Buttons::B | Buttons::RIGHT);
    let mut moved = false;
    for _ in 0..60 {
        let before = scaled.shown_now.objects[11];
        scaled.tick(&world, swing, swing, 144);
        assert!(scaled.tow.holding());
        moved |= before != scaled.shown_now.objects[11];
        for slot in [2, 11] {
            let o = &scaled.objects.slots[slot];
            let (shown_x, shown_y, ..) = scaled.shown_now.objects[slot].unwrap();
            assert_eq!((shown_x as u16, shown_y as u16), (o.x, o.y), "slot {slot}");
        }
    }
    assert!(moved);
}
