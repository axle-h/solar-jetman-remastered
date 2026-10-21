use exp::contact::*;
use std::sync::Arc;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { if v.is_empty() { return 0.0; } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() - 1) as f64 * p) as usize] }

fn rest_stats(world: &solar_jetman::game::level::World, level: usize, site: (u16,u16), rules: Rules, rate: i64) -> (f64, f64) {
    let (mut fl, mut cr) = (0.0f64, 0.0f64); let n = 12;
    for k in 0..n {
        let init = Init { x: site.0, y: site.1 + (k % 6) as u16, yf: (k * 53) as u8, vs: (k as u16 % 4) * 0x40, vd: true, ..Default::default() };
        let tr = run(world, level, rules, rate, init, 100.0);
        let late: Vec<_> = tr.ev.iter().filter(|e| ms(e.t) >= 40000.0).collect();
        cr += late.len() as f64 / 60.0; fl += late.iter().map(|e| e.loss).sum::<i64>() as f64 / 60.0;
    }
    (fl / n as f64, cr / n as f64)
}

fn main() {
    let world = Arc::new(exp::world());
    let scens = Arc::new(scenarios(&world));
    let cands: Vec<(&str, String)> = vec![
        ("A final: +approach gate with 1-step allowance (appr_cd)", "corr+tp1+appr+acd+push+wfull+fxcd".into()),
    ];
    let rates = [60i64, 75, 90, 100, 120, 137, 144, 165, 200];
    let sites = [(3usize, (40u16, 1264u16)), (2, (117, 1840)), (1, (201, 448)), (4, (138, 640)), (5, (68, 3400)), (10, (201, 136)), (11, (40, 1936))];
    for (name, spec) in &cands {
        let rules = rules_from(spec);
        // rest (per rate): mean over levels of |%err| fuel and contacts
        let mut handles = vec![];
        for &rate in &rates {
            let (w, ) = (world.clone(),);
            handles.push(std::thread::spawn(move || {
                let base: Vec<(f64, f64)> = sites.iter().map(|&(l, s)| rest_stats(&w, l, s, rules, 30)).collect();
                let got: Vec<(f64, f64)> = sites.iter().map(|&(l, s)| rest_stats(&w, l, s, rules, rate)).collect();
                let ef = got.iter().zip(&base).map(|(a, b)| 100.0 * (a.0 / b.0.max(1e-9) - 1.0)).collect::<Vec<_>>();
                let ec = got.iter().zip(&base).map(|(a, b)| 100.0 * (a.1 / b.1.max(1e-9) - 1.0)).collect::<Vec<_>>();
                (rate, ef, ec)
            }));
        }
        println!("### {name} [{spec}]");
        for h in handles { let (rate, ef, ec) = h.join().unwrap();
            println!("  rest hop R{rate:3}: fuel/s err mean {:+5.1}% worst {:+5.1}% | contacts/s err mean {:+5.1}% worst {:+5.1}%", ef.iter().sum::<f64>() / ef.len() as f64, ef.iter().cloned().fold(0.0, |a: f64, b| if b.abs() > a.abs() { b } else { a }), ec.iter().sum::<f64>() / ec.len() as f64, ec.iter().cloned().fold(0.0, |a: f64, b| if b.abs() > a.abs() { b } else { a })); }
        // first-contact + pressed
        let mut handles = vec![];
        for &rate in &rates {
            let (w, sc) = (world.clone(), scens.clone());
            handles.push(std::thread::spawn(move || {
                let mut t1 = vec![]; let mut dv = vec![]; let mut evr = vec![];
                for s in sc.iter() {
                    for v in variants(s.class) {
                        let init = with_variant(s, v);
                        let a = run(&w, s.level, rules, 30, init, 1.5);
                        if a.ev.is_empty() { continue; }
                        let b = run(&w, s.level, rules, rate, init, 1.5);
                        if b.ev.is_empty() { t1.push(999.0); continue; }
                        t1.push(ms(b.ev[0].t) - ms(a.ev[0].t));
                        let k = ((ms(a.ev[0].t) / 33.333) as usize + 3).min(a.s.len() - 1);
                        dv.push(((a.s[k].vx - b.s[k].vx).powi(2) + (a.s[k].vy - b.s[k].vy).powi(2)).sqrt());
                        evr.push(b.ev.len() as f64 / a.ev.len() as f64);
                    }
                }
                // pressed
                let (mut oe, mut se, mut of, mut sf) = (0.0, 0.0, 0.0, 0.0);
                for s in sc.iter() {
                    let thr: (i64, i64) = match (s.class, s.tag.contains("wallL")) { ("F", _) | ("S", _) => (0, 16), ("C", _) | ("SC", _) => (0, -16), ("W", false) => (16, 0), _ => (-16, 0) };
                    let a = run_thr(&w, s.level, rules, 30, s.init, thr, 8.0); let b = run_thr(&w, s.level, rules, rate, s.init, thr, 8.0);
                    oe += a.ev.iter().filter(|e| ms(e.t) >= 2000.0).count() as f64; se += b.ev.iter().filter(|e| ms(e.t) >= 2000.0).count() as f64;
                    of += a.ev.iter().filter(|e| ms(e.t) >= 2000.0).map(|e| e.loss).sum::<i64>() as f64; sf += b.ev.iter().filter(|e| ms(e.t) >= 2000.0).map(|e| e.loss).sum::<i64>() as f64;
                }
                (rate, t1, dv, evr, 100.0 * (se / oe - 1.0), 100.0 * (sf / of - 1.0))
            }));
        }
        for h in handles { let (rate, mut t1, mut dv, mut evr, pe, pf) = h.join().unwrap();
            let miss = t1.iter().filter(|x| **x >= 999.0).count(); t1.retain(|x| *x < 999.0);
            let mean = t1.iter().sum::<f64>() / t1.len() as f64;
            println!("  contacts R{rate:3}: first-contact lead mean {:+6.1} ms (p10 {:+5.1} p90 {:+5.1}) misses {miss} | exit dv p50 {:4.1} p90 {:5.1} (1/256 px/step) | event count ratio 1.5 s p50 {:.2} p90 {:.2} | pressed 6 s: events {:+5.1}% fuel {:+5.1}%", mean, pct(&mut t1.clone(), 0.1), pct(&mut t1.clone(), 0.9), pct(&mut dv.clone(), 0.5), pct(&mut dv, 0.9), pct(&mut evr.clone(), 0.5), pct(&mut evr, 0.9), pe, pf); }
    }
}
