use exp::contact::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }
fn main() {
    let world = exp::world();
    let which = std::env::args().nth(1).unwrap_or("corr+appr+push".into());
    let class = std::env::args().nth(2).unwrap_or("W".into());
    let rate: i64 = std::env::args().nth(3).map(|s| s.parse().unwrap()).unwrap_or(200);
    let mut rules = Rules::naive();
    rules.motion_corr = which.contains("corr"); rules.push_scaled = which.contains("push"); rules.approach = which.contains("appr"); rules.wall_full = which.contains("wfull");
    let scens = scenarios(&world);
    for v in variants(&class) {
        let (mut ex, mut p05, mut ratio) = (vec![], vec![], vec![]);
        for s in scens.iter().filter(|s| s.class == class) {
            let init = with_variant(s, v);
            let a = run(&world, s.level, rules, 30, init, 2.0);
            let b = run(&world, s.level, rules, rate, init, 2.0);
            if a.ev.is_empty() || b.ev.is_empty() { continue; }
            let k = ((ms(a.ev[0].t) / 33.333) as usize + 3).min(a.s.len() - 1);
            ex.push(((a.s[k].vx - b.s[k].vx).powi(2) + (a.s[k].vy - b.s[k].vy).powi(2)).sqrt());
            let k5 = (k + 15).min(a.s.len() - 1);
            p05.push(((a.s[k5].x - b.s[k5].x).powi(2) + (a.s[k5].y - b.s[k5].y).powi(2)).sqrt());
            let va = (a.s[k].vx.powi(2) + a.s[k].vy.powi(2)).sqrt(); let vb = (b.s[k].vx.powi(2) + b.s[k].vy.powi(2)).sqrt();
            ratio.push(vb / va.max(1.0));
        }
        println!("variant {:?}: n={} exit dv p50 {:.1} p90 {:.1} max {:.1} | pos@0.5s p50 {:.2} p90 {:.2} | exit speed ratio scaled/orig p10 {:.2} p50 {:.2} p90 {:.2}", v, ex.len(), pct(&mut ex.clone(), 0.5), pct(&mut ex.clone(), 0.9), ex.iter().cloned().fold(0.0, f64::max), pct(&mut p05.clone(), 0.5), pct(&mut p05, 0.9), pct(&mut ratio.clone(), 0.1), pct(&mut ratio.clone(), 0.5), pct(&mut ratio, 0.9));
    }
}
