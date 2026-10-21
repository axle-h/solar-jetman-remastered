use exp::tow::podm::POpt;
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[((v.len() as f64 - 1.0) * p) as usize] }
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let syn = dataset(&world, false, true);
    let o = POpt { rope: Opt { cap: Cap::GlitchScaled, trap: true, damp: true, hold: true }, lift: true };
    for (label, segs) in [("REAL", &real), ("SYNTH", &syn)] {
        for rate in [60u32, 100, 144, 200] {
            let (mut pe, mut ae) = (vec![], vec![]);
            for s in segs.iter() {
                let n = s.recs.len();
                let mut k0 = 0;
                while k0 + 30 <= n {
                    if s.recs[k0..k0 + 30].iter().all(|r| r.surface == 0 && r.pre_p.y >= 24 && r.post_p.y >= 24) {
                        let e30 = exp::tow::podm::window(s, k0, 30, 30, o, 0.0);
                        if e30[29].pos < 1.0 {
                            let e = exp::tow::podm::window(s, k0, 30, rate, o, 0.0);
                            pe.push(e[29].pos); ae.push(e[29].ang.abs());
                        }
                    }
                    k0 += 7;
                }
            }
            println!("{label} R{rate}: n {} pos@30steps P50 {:.3} P90 {:.3} P99 {:.3} max {:.2} | angle P50 {:.3} P90 {:.3} P99 {:.3} max {:.2}", pe.len(), pct(&mut pe.clone(), 0.5), pct(&mut pe.clone(), 0.9), pct(&mut pe.clone(), 0.99), pe.iter().cloned().fold(0.0, f64::max), pct(&mut ae.clone(), 0.5), pct(&mut ae.clone(), 0.9), pct(&mut ae.clone(), 0.99), ae.iter().cloned().fold(0.0, f64::max));
        }
    }
}
