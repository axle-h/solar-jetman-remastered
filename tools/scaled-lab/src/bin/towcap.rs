use exp::tow::podm::POpt;
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let syn = dataset(&world, false, true);
    for (label, segs) in [("REAL", &real), ("SYNTH", &syn)] {
        for (cn, cap) in [("glitch(orig, exact@30)", Cap::Glitch), ("clamp 1.00", Cap::Clamp), ("no cap", Cap::None)] {
            let o = POpt { rope: Opt { cap, trap: true, damp: true, hold: true }, lift: true };
            let (mut n, mut nw, mut sp, mut sa, mut spw, mut saw, mut mp, mut ma) = (0, 0, 0.0, 0.0, 0.0, 0.0, 0.0f64, 0.0f64);
            for s in segs.iter() {
                let len = s.recs.len();
                let mut k0 = 0;
                while k0 + 30 <= len {
                    if s.recs[k0..k0 + 30].iter().all(|r| r.surface == 0 && r.pre_p.y >= 24 && r.post_p.y >= 24) {
                        let e30 = exp::tow::podm::window(s, k0, 30, 30, POpt { rope: Opt { cap: Cap::Glitch, ..o.rope }, ..o }, 0.0);
                        if e30[29].pos < 1.0 {
                            let e = exp::tow::podm::window(s, k0, 30, 30, o, 0.0);
                            let wmax = s.recs[k0..k0 + 30].iter().map(|r| r.post_t.speed).max().unwrap();
                            n += 1; sp += e[29].pos.powi(2); sa += e[29].ang.powi(2); mp = mp.max(e[29].pos); ma = ma.max(e[29].ang.abs());
                            if wmax >= 0x10000 || s.recs[k0].pre_t.speed >= 0x10000 || true && s.recs[k0..k0+30].iter().any(|r| r.post_t.speed >> 8 >= 0xFE) { nw += 1; spw += e[29].pos.powi(2); saw += e[29].ang.powi(2); }
                        }
                    }
                    k0 += 7;
                }
            }
            println!("{label} {cn}: all windows {n}: pos@1s rms {:.3} max {:.2}; angle rms {:.3} max {:.2} | windows near/above cap {nw}: pos rms {:.3} angle rms {:.3}", (sp / n as f64).sqrt(), mp, (sa / n as f64).sqrt(), ma, (spw / nw.max(1) as f64).sqrt(), (saw / nw.max(1) as f64).sqrt());
        }
    }
}
