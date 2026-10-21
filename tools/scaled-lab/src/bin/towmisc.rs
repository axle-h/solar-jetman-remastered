use exp::tow::podm::{POpt, window_full};
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let syn = dataset(&world, false, true);
    let o = POpt { rope: Opt { cap: Cap::GlitchPendHalf, trap: true, damp: true, hold: true }, lift: true };
    let o_nohold = POpt { rope: Opt { hold: false, ..o.rope }, ..o };
    let pump: Vec<_> = syn.iter().filter(|s| s.name.contains("-m2")).cloned().collect();
    let rnd: Vec<_> = syn.iter().filter(|s| !s.name.contains("-m2")).cloned().collect();
    for (label, segs) in [("REAL", &real), ("SYNTH-random", &rnd), ("SYNTH-pump", &pump)] {
        for (oname, oo) in [("hold", o), ("nohold", o_nohold)] {
        println!("{label} spin test {oname}: rate | windows | real events | model events | matched | mean dt ms (model-real) | max |dt| ms | item px rms/max@1s | a8 mismatch frac");
        for rate in [30u32, 60, 100, 144, 200] {
            let (mut nw, mut ner, mut nem, mut matched, mut sdt, mut mdt) = (0, 0, 0, 0, 0.0f64, 0.0f64);
            let (mut si, mut mi, mut ns, mut nmis) = (0.0f64, 0.0f64, 0usize, 0usize);
            for s in segs.iter() {
                let n = s.recs.len();
                let mut k0 = 0;
                while k0 + 30 <= n {
                    if s.recs[k0..k0 + 30].iter().all(|r| r.surface == 0 && r.pre_p.y >= 24 && r.post_p.y >= 24) {
                        let (e30, _) = window_full(s, k0, 30, 30, o, 0.0);
                        if e30[29].pos < 1.0 {
                            let (e, (em, er)) = window_full(s, k0, 30, rate, oo, 0.0);
                            nw += 1;
                            // interior events only (avoid window-edge effects): times in (1, 28)
                            let em2: Vec<f64> = em.iter().cloned().filter(|t| *t > 1.0 && *t < 28.0).collect();
                            let er2: Vec<f64> = er.iter().cloned().filter(|t| *t > 1.0 && *t < 28.0).collect();
                            ner += er2.len(); nem += em2.len();
                            for t in &er2 {
                                if let Some(m) = em2.iter().min_by(|a, b| (*a - t).abs().partial_cmp(&(*b - t).abs()).unwrap()) {
                                    if (m - t).abs() < 2.0 { matched += 1; let dt = (m - t) * 1000.0 / 30.0; sdt += dt; mdt = mdt.max(dt.abs()); }
                                }
                            }
                            for x in e.iter() { si += x.item.powi(2); mi = mi.max(x.item); ns += 1; if x.a8mis != 0 { nmis += 1; } }
                        }
                    }
                    k0 += 7;
                }
            }
            println!("R {rate:3} | {nw} | {ner} | {nem} | {matched} | {:.1} | {:.1} | {:.3}/{:.2} | {:.4}", sdt / matched.max(1) as f64, mdt, (si / ns as f64).sqrt(), mi, nmis as f64 / ns as f64);
        }}
    }
}
