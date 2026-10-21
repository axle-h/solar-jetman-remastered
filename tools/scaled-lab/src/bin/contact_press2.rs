use exp::contact::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }
fn main() {
    let world = exp::world();
    let scens = scenarios(&world);
    let which = std::env::args().nth(1).unwrap_or("corr+appr+push+wfull+fxcd".into());
    let rules = rules_from(&which);
    let nothr = std::env::args().nth(2).is_some();
    let secs: f64 = std::env::args().nth(3).map(|x| x.parse().unwrap()).unwrap_or(12.0);
    println!("rules {which}: thrust-pressed scenarios, 12 s each, window 2..12 s");
    for class in ["F", "S", "W", "C", "SC"] {
        let ss: Vec<&Scen> = scens.iter().filter(|s| s.class == class).collect();
        let mut base: Vec<(f64, f64, f64, f64)> = vec![];
        let thr_of = |s: &Scen| -> (i64, i64) { match (s.class, s.tag.contains("wallL")) { ("F", _) | ("S", _) => (0, 16), ("C", _) | ("SC", _) => (0, -16), ("W", false) => (16, 0), _ => (-16, 0) } };
        let measure = |s: &Scen, rate: i64| -> (f64, f64, f64, f64) {
            let init = s.init;
            let tr = run_thr(&world, s.level, rules, rate, init, if nothr { (0, 0) } else { thr_of(s) }, secs);
            let span = secs - 2.0;
            let w: Vec<_> = tr.ev.iter().filter(|e| ms(e.t) >= 2000.0).collect();
            let dmg = w.iter().map(|e| e.loss).sum::<i64>() as f64 / span;
            let last = tr.s.len() - 1; let mid = 60;
            let drift = ((tr.s[last].x - tr.s[mid].x).powi(2) + (tr.s[last].y - tr.s[mid].y).powi(2)).sqrt() / 10.0;
            (w.len() as f64 / span, w.iter().filter(|e| e.dmg).count() as f64 / span, dmg, drift)
        };
        for s in &ss { base.push(measure(s, 30)); }
        let tot = |v: &Vec<(f64, f64, f64, f64)>, i: usize| v.iter().map(|x| match i { 0 => x.0, 1 => x.1, 2 => x.2, _ => x.3 }).sum::<f64>();
        println!("== {class}: n={} orig: events/s {:.2} dmg-events/s {:.3} impact-fuel/s {:.0} drift px/s {:.2}", ss.len(), tot(&base, 0) / ss.len() as f64, tot(&base, 1) / ss.len() as f64, tot(&base, 2) / ss.len() as f64, tot(&base, 3) / ss.len() as f64);
        for &rate in &[60i64, 90, 144, 200, 137] {
            let v: Vec<_> = ss.iter().map(|s| measure(s, rate)).collect();
            let r = |i: usize| 100.0 * (tot(&v, i) / tot(&base, i).max(1e-9) - 1.0);
            // per-scenario ratio of events
            let mut ratios: Vec<f64> = v.iter().zip(&base).filter(|(_, b)| b.0 > 0.2).map(|(a, b)| a.0 / b.0).collect();
            println!("  R{rate:3}: events/s {:+6.1}% dmg-events {:+6.1}% impact-fuel {:+6.1}% drift {:+6.1}% | per-scenario events ratio p10 {:.2} p50 {:.2} p90 {:.2}", r(0), r(1), r(2), r(3), pct(&mut ratios.clone(), 0.1), pct(&mut ratios.clone(), 0.5), pct(&mut ratios, 0.9));
        }
    }
}
