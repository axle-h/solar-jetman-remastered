use exp::contact::*;
fn main() {
    let world = exp::world();
    let scens = scenarios(&world);
    let mut rules = rules_from("corr+tp1+appr+acd+push+wfull+fxcd"); rules.vcap = 0x900;
    println!("tunnelling sweep: initial speed up to 9 px/step; miss = no contact within 400 ms of the 3000 Hz reference");
    for sp in [0x300u16, 0x400, 0x500, 0x600, 0x800, 0x900] {
        let mut line = format!("speed {:.2} px/step:", sp as f64 / 256.0);
        for &rate in &[30i64, 60, 100, 144, 200, 137] {
            let (mut n, mut miss) = (0, 0);
            for s in scens.iter().filter(|s| matches!(s.class, "F" | "W" | "C")) {
                let mut init = s.init;
                match s.class { "F" => { init.vs = sp; init.vd = true; } "C" => { init.vs = sp; init.vd = false; } _ => { init.hs = sp; } }
                let r = run(&world, s.level, rules, 3000, init, 0.4);
                if r.ev.is_empty() { continue; }
                n += 1;
                let b = run(&world, s.level, rules, rate, init, 0.4);
                if b.ev.is_empty() || (ms(b.ev[0].t) - ms(r.ev[0].t)).abs() > 100.0 { miss += 1; }
            }
            line += &format!("  R{rate}: {miss}/{n}");
        }
        println!("{line}");
    }
}
