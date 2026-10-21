use exp::contact::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }
fn main() {
    let world = exp::world();
    let which = std::env::args().nth(1).unwrap_or("corr+appr+push+wfull".into());
    let mut rules = Rules::naive();
    rules.motion_corr = which.contains("corr"); rules.push_scaled = which.contains("push"); rules.approach = which.contains("appr"); rules.wall_full = which.contains("wfull");
    let scens = scenarios(&world);
    println!("rules {which}. Latency = first-contact time minus reference (R=3000 Hz model), ms, over {} F + W + C variants; misses = no event within 150 ms of reference", scens.len());
    for class in ["F", "W", "C"] {
        for &rate in &[30i64, 60, 75, 90, 100, 120, 144, 165, 200, 137] {
            let mut lat = vec![]; let mut miss = 0; let mut n = 0;
            for s in scens.iter().filter(|s| s.class == class) { for v in variants(class) {
                let init = with_variant(s, v);
                let r = run(&world, s.level, rules, 3000, init, 1.2);
                if r.ev.is_empty() { continue; }
                n += 1;
                let b = run(&world, s.level, rules, rate, init, 1.2);
                if b.ev.is_empty() || (ms(b.ev[0].t) - ms(r.ev[0].t)).abs() > 150.0 { miss += 1; continue; }
                lat.push(ms(b.ev[0].t) - ms(r.ev[0].t));
            } }
            let mean = lat.iter().sum::<f64>() / lat.len() as f64;
            println!("{class} R{rate:3}: n={n:4} mean {:6.2} p50 {:6.2} p90 {:6.2} max {:6.2} min {:6.2} misses {miss}", mean, pct(&mut lat.clone(), 0.5), pct(&mut lat.clone(), 0.9), lat.iter().cloned().fold(-1e9, f64::max), lat.iter().cloned().fold(1e9, f64::min));
        }
    }
}
