use exp::contact::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }
fn main() {
    let world = exp::world();
    let scens: Vec<Scen> = scenarios(&world).into_iter().filter(|s| s.class == "F").collect();
    let spec = std::env::args().nth(1).unwrap_or("corr+tp1+appr+push+wfull+fxcd".into());
    let rules = rules_from(&spec);
    println!("flat-floor landings, rules {spec}: {} sites x 6 contact speeds, 12 s", scens.len());
    println!("rate | 1st contact speed err | bounce-1 apex ratio p10/p50/p90 | bounce-2 apex ratio p50 | time of 3rd contact err ms mean/p90 | rest onset (apex<1px) err ms p50/p90 | fuel at 12 s err (units) p50/p90 | damage events 12 s ratio");
    for &rate in &[60i64, 90, 100, 144, 200, 137] {
        let (mut sp, mut a1, mut a2, mut t3, mut rest, mut fu, mut de) = (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
        for s in &scens { for v in variants("F") {
            let init = with_variant(s, v);
            let a = run(&world, s.level, rules, 30, init, 12.0);
            let b = run(&world, s.level, rules, rate, init, 12.0);
            if a.ev.len() < 4 || b.ev.len() < 4 { continue; }
            sp.push(b.ev[0].speed as f64 - a.ev[0].speed as f64);
            if a.apex.len() > 2 && b.apex.len() > 2 { if a.apex[0] > 0.3 { a1.push(b.apex[0] / a.apex[0]); } if a.apex[1] > 0.3 { a2.push(b.apex[1] / a.apex[1]); } }
            t3.push(ms(b.ev[2].t) - ms(a.ev[2].t));
            let ro = |t: &Tr| t.apex.iter().position(|&h| h < 1.0).map(|i| ms(t.ev[i + 1].t));
            if let (Some(x), Some(y)) = (ro(&a), ro(&b)) { rest.push(y - x); }
            fu.push((b.s.last().unwrap().fuel - a.s.last().unwrap().fuel) as f64);
            de.push(b.ev.iter().filter(|e| e.dmg).count() as f64 / a.ev.iter().filter(|e| e.dmg).count().max(1) as f64);
        } }
        let absv = |v: &Vec<f64>| v.iter().map(|x| x.abs()).collect::<Vec<_>>();
        println!("R{rate:3} | {:+.1} (max {:.0}) | {:.2}/{:.2}/{:.2} | {:.2} | {:+.0}/{:.0} | {:+.0}/{:.0} | {:.0}/{:.0} | {:.2}", sp.iter().sum::<f64>() / sp.len() as f64, absv(&sp).iter().cloned().fold(0.0, f64::max), pct(&mut a1.clone(), 0.1), pct(&mut a1.clone(), 0.5), pct(&mut a1.clone(), 0.9), pct(&mut a2, 0.5), t3.iter().sum::<f64>() / t3.len() as f64, pct(&mut absv(&t3), 0.9), pct(&mut rest.clone(), 0.5), pct(&mut absv(&rest), 0.9), pct(&mut absv(&fu), 0.5), pct(&mut absv(&fu), 0.9), de.iter().sum::<f64>() / de.len() as f64);
    }
}
