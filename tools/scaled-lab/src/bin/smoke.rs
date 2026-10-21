fn main() {
    let world = exp::world();
    let mut g = exp::game(&world, 0);
    for _ in 0..60 { g.step(&world, solar_jetman::game::gameplay::input::Buttons(0)); }
    let b = &g.pod.body;
    println!("pod {} {} v {} {} facing {}", b.x, b.y, b.v_speed, b.v_down, g.pod.facing);
    println!("replays: {}", std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().count());
}
