use exp::contact::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }
const SECS: f64 = 10.0;
fn main() {
    let world = exp::world();
    let which = std::env::args().nth(1).unwrap_or("naive".into());
    let classes: Vec<String> = std::env::args().nth(2).map(|s| s.split(',').map(String::from).collect()).unwrap_or(vec!["F".into(),"S".into(),"SC".into(),"W".into(),"C".into()]);
    let mut rules = Rules::naive();
    if which.contains("corr") { rules.motion_corr = true; }
    if which.contains("push") { rules.push_scaled = true; }
    if which.contains("appr") { rules.approach = true; }
    if which.contains("fxcd") { rules.fx_cd = true; }
    if which.contains("wfull") { rules.wall_full = true; }
    let scens = scenarios(&world);
    println!("rules {which}; {} scenarios", scens.len());
    for class in &classes {
        println!("== class {class}");
        let ss: Vec<&Scen> = scens.iter().filter(|s| s.class == class).collect();
        for &rate in &[60i64, 90, 144, 200, 137] {
            let (mut t1, mut ex, mut p05, mut p1, mut p3, mut fu, mut nev) = (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
            for s in &ss { for v in variants(class) {
                let init = with_variant(s, v);
                let a = run(&world, s.level, rules, 30, init, SECS);
                let b = run(&world, s.level, rules, rate, init, SECS);
                if a.ev.is_empty() { continue; }
                if b.ev.is_empty() { t1.push(999.0); continue; }
                t1.push(ms(b.ev[0].t) - ms(a.ev[0].t));
                // exit velocity error: velocity at sample 2 steps after first contact
                let k = ((ms(a.ev[0].t) / 33.333) as usize + 3).min(a.s.len() - 1);
                let dv = ((a.s[k].vx - b.s[k].vx).powi(2) + (a.s[k].vy - b.s[k].vy).powi(2)).sqrt();
                ex.push(dv);
                let dpos = |k: usize| ((a.s[k].x - b.s[k].x).powi(2) + (a.s[k].y - b.s[k].y).powi(2)).sqrt();
                let kk = |sec: f64| ((ms(a.ev[0].t) / 1000.0 + sec) * 30.0) as usize;
                if kk(0.5) < a.s.len() { p05.push(dpos(kk(0.5))); } if kk(1.0) < a.s.len() { p1.push(dpos(kk(1.0))); } if kk(3.0) < a.s.len() { p3.push(dpos(kk(3.0))); }
                fu.push((a.s.last().unwrap().fuel - b.s.last().unwrap().fuel) as f64);
                nev.push(b.ev.len() as f64 - a.ev.len() as f64);
            } }
            let mut ta = t1.iter().map(|x| x.abs()).collect::<Vec<_>>();
            println!("R{rate:3}: n={:4} t1(ms) mean {:+6.1} p90|.| {:5.1} max|.| {:6.1} | exit dv(1/256px) p50 {:5.1} p90 {:6.1} max {:7.1} | pos err px @0.5s p90 {:5.2} @1s p90 {:5.2} @3s p50 {:5.2} p90 {:6.2} | fuel@end err p50 {:6.0} p90 {:7.0} | dEvents mean {:+4.1}",
                t1.len(), t1.iter().sum::<f64>() / t1.len() as f64, pct(&mut ta, 0.9), ta.iter().cloned().fold(0.0, f64::max),
                pct(&mut ex, 0.5), pct(&mut ex, 0.9), ex.iter().cloned().fold(0.0, f64::max), pct(&mut p05, 0.9), pct(&mut p1, 0.9), pct(&mut p3, 0.5), pct(&mut p3, 0.9),
                pct(&mut fu.iter().map(|x| x.abs()).collect(), 0.5), pct(&mut fu.iter().map(|x| x.abs()).collect(), 0.9), nev.iter().sum::<f64>() / nev.len() as f64);
        }
    }
}
