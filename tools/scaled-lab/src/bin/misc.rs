use exp::misc::common::*;
use solar_jetman::game::gameplay::input::Buttons;
fn main() {
    let world = exp::world();
    let arg = std::env::args().nth(1).unwrap_or_default();
    if arg == "gate0" {
        let (mut g, r) = replay_game(&world, "wormhole");
        println!("steps {} level {}", r.steps.len(), r.level);
        let mut last_state = 255;
        for (i, &b) in r.steps.iter().enumerate() {
            setups(&mut g, &r, i + 1);
            if g.transition().is_some() { println!("transition at {i}"); break; }
            let c0 = g.counter();
            g.step(&world, b);
            let st = g.pod.state;
            if st == 9 || last_state == 9 {
                let k = g.tow.angle & 0xFF;
                println!("step {:4} state {} cnt {:6} (pre {:6}) k {:3} fades {} flash {:?} facing {} pal {} grp15 {} grp23 {} hitp {} inv {}", i + 1, st, g.counter(), c0, k, g.gate_fades(), g.gate_flash(), g.pod.facing, g.pod.palette_bits, g.objects.slots[15].state, g.objects.slots[23].state, g.pod.hit_pause, g.pod.invulnerable);
            }
            last_state = st;
        }
    }
    if arg == "foot0" { exp::misc::foot::summary(&world); }
    if arg == "foot1" {
        let rs = exp::misc::foot::runs(&world, 1, 120);
        for r in rs.iter().take(60) {
            let k = r.kill; if k == usize::MAX { continue; }
            print!("L{} kill {} :", r.level, k);
            for rec in &r.recs[k..(k+16).min(r.recs.len())] { print!(" [s{} c{:x} hh{} y{} sf{:x}]", rec.state, rec.counter, rec.hh, rec.y, rec.surface); }
            println!();
        }
    }
    if arg == "walk" {
        let rs = exp::misc::foot::runs(&world, 6, 600);
        exp::misc::foot::stick_stats(&rs);
        exp::misc::walk::report(&rs);
    }
    if arg == "land" { exp::misc::land::report(&world); }
    if arg == "expl0" { let rs = exp::misc::foot::runs(&world, 2, 100); exp::misc::land::debug_explosion(&world, &rs); }
    if arg == "expl" { let rs = exp::misc::foot::runs(&world, 6, 100); exp::misc::land::report_explosion(&world, &rs); }
    if arg == "timers" { exp::misc::timers::report_lateness(); exp::misc::timers::report_engine(&world); }
    if arg == "gdbg" { exp::misc::timers::debug_gauge(&world); }
    if arg == "cad" { exp::misc::timers::report_cadences(&world); exp::misc::timers::report_gauge(&world); exp::misc::timers::report_shield(&world); }
    if arg == "gdbg2" { exp::misc::gate::debug_homing(&world); }
    if arg == "gdbg3" { exp::misc::gate::debug_first_mismatch(&world); }
    if arg == "warp" { exp::misc::gate::warp_levels(&world); }
    if arg == "gate" { exp::misc::gate::groups(&world); }
    if arg == "warp" { exp::misc::gate::warp_levels(&world); }
    if arg == "gate" { exp::misc::gate::cadences(&world); exp::misc::gate::homing(&world); }
    if arg == "ship1" { exp::misc::ship::debug(&world); }
    if arg == "ship0" { exp::misc::ship::summary(&world); }
    if arg == "shipc" { exp::misc::ship::classify(&world); }
    if arg == "shipe" { exp::misc::ship::check_end(&world); }
    if arg == "ship" { exp::misc::ship::report(&world); }
    if arg == "scan" { exp::misc::scan::report(&world); }
    if arg == "orbd" { exp::misc::scan::orb_debug(&world); }
    if arg == "orb" { exp::misc::scan::orb(&world); }
    if arg == "save" { exp::misc::save::report_pod(true); exp::misc::save::report_pod(false); exp::misc::save::report_contact(&world); }
    let _ = Buttons(0);
}
