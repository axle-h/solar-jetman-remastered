use exp::tow::rec::*;
use solar_jetman::game::gameplay::input::Buttons;
fn main() {
    let world = exp::world();
    let mut g = solar_jetman::game::gameplay::Gameplay::new(&world, 0, seed_for(0, 0));
    let (x, y) = (g.pod.body.x, g.pod.body.y);
    println!("start {x} {y} cam {:?} state {}", g.camera.position, g.pod.state);
    for i in 0..12 {
        println!("{i}: apass {} lvl {} trans {:?} state {} y {} holding {}", g.next_is_a_pass(), g.level(), g.transition().is_some(), g.pod.state, g.pod.body.y, g.tow.holding());
        g.step(&world, Buttons(0));
    }
}
