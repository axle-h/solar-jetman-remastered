use exp::contact::*;
fn stats(world: &solar_jetman::game::level::World, level: usize, site: (u16,u16), rules: Rules, rate: i64) -> (f64, f64, f64, f64) {
    // returns (fuel loss/s, contacts/s, dmg events/s, mean contact speed) over 40..100 s, averaged over 24 phases
    let (mut fl, mut cr, mut dm, mut sp, mut spn) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let n = 24;
    for k in 0..n {
        let init = Init { x: site.0, y: site.1 + (k % 6) as u16, yf: (k * 53) as u8, vs: (k as u16 % 4) * 0x40, vd: true, ..Default::default() };
        let tr = trace(world, level, rules, rate, init, 100.0);
        let late: Vec<_> = tr.ev.iter().filter(|e| ms(e.t) >= 40000.0).collect();
        let t = 60.0;
        cr += late.len() as f64 / t; dm += late.iter().filter(|e| e.dmg).count() as f64 / t;
        fl += late.iter().map(|e| e.loss).sum::<i64>() as f64 / t;
        for e in &late { sp += e.speed as f64; spn += 1.0; }
    }
    (fl / n as f64, cr / n as f64, dm / n as f64, sp / spn.max(1.0))
}
fn main() {
    let world = exp::world();
    let sites = [(3usize, (40u16, 1264u16)), (2, (117, 1840)), (1, (201, 448)), (4, (138, 640)), (5, (68, 3400)), (10, (201, 136)), (11, (40, 1936))];
    let which = std::env::args().nth(1).unwrap_or("naive".into());
    let rules = rules_from(&which);
    println!("rules {which}");
    for &(level, site) in &sites {
        println!("L{level} g{}", world.levels[level].gravity);
        let base = stats(&world, level, site, rules, 30);
        println!("  R 30: fuel/s {:8.0} contacts/s {:5.2} dmg/s {:5.3} speed {:5.1}", base.0, base.1, base.2, base.3);
        for &rate in &[60i64, 90, 144, 200, 137] {
            let s = stats(&world, level, site, rules, rate);
            println!("  R{rate:3}: fuel/s {:8.0} ({:+5.1}%) contacts/s {:5.2} ({:+5.1}%) dmg/s {:5.3} speed {:5.1} ({:+5.1}%)", s.0, 100.0*(s.0/base.0-1.0), s.1, 100.0*(s.1/base.1-1.0), s.2, s.3, 100.0*(s.3/base.3-1.0));
        }
    }
}
