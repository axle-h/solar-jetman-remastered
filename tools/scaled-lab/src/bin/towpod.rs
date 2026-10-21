use exp::tow::podm::{POpt, Err};
use exp::tow::scaled::*;
use exp::tow::seg::*;

fn stats(label: &str, segs: &[Seg], o: POpt, rates: &[u32], thr: f64) {
    let hs = [1usize, 3, 10, 30];
    println!("{label}: pos err px rms/max vs REAL at 1,3,10,30 steps | vs model@30 | angle rms/max @30 | n windows");
    // build windows list and R30 reference
    let mut wins: Vec<(usize, usize)> = vec![];
    let mut ref30: Vec<Vec<Err>> = vec![];
    for (si, s) in segs.iter().enumerate() {
        let n = s.recs.len();
        let mut k0 = 0;
        while k0 + 30 <= n {
            if s.recs[k0..k0 + 30].iter().all(|r| r.surface == 0 && r.pre_p.y >= 24 && r.post_p.y >= 24) {
                let e = exp::tow::podm::window(s, k0, 30, 30, o, 0.0);
                if e[29].pos < thr {
                    wins.push((si, k0));
                    ref30.push(e);
                }
            }
            k0 += 7;
        }
    }
    for &rate in rates {
        let mut a = vec![(0.0f64, 0.0f64); 4];
        let mut b = vec![(0.0f64, 0.0f64); 4];
        let (mut sa, mut ma) = (0.0f64, 0.0f64);
        for (wi, &(si, k0)) in wins.iter().enumerate() {
            let ph: f64 = std::env::var("PHASE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let e = exp::tow::podm::window(&segs[si], k0, 30, rate, o, ph);
            for (i, &h) in hs.iter().enumerate() {
                a[i].0 += e[h - 1].pos.powi(2); a[i].1 = a[i].1.max(e[h - 1].pos);
                // vs model@30: need the vector difference; approximate with |err_R - err_30| bound via triangle (use difference of signed pos not available) -> use pos errors difference
                let d = (e[h - 1].pos - ref30[wi][h - 1].pos).abs();
                b[i].0 += d.powi(2); b[i].1 = b[i].1.max(d);
            }
            sa += e[29].ang.powi(2); ma = ma.max(e[29].ang.abs());
        }
        let n = wins.len() as f64;
        println!("R {rate:3}: {} | {} | {:.3}/{:.2} | {}", (0..4).map(|i| format!("{:.3}/{:.2}", (a[i].0 / n).sqrt(), a[i].1)).collect::<Vec<_>>().join(" "), (0..4).map(|i| format!("{:.3}/{:.2}", (b[i].0 / n).sqrt(), b[i].1)).collect::<Vec<_>>().join(" "), (sa / n).sqrt(), ma, wins.len());
    }
}
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let syn = dataset(&world, false, true);
    let rates = [30u32, 60, 75, 100, 120, 144, 200, 137];
    let cap = Cap::GlitchPendHalf;
    for (name, trap, damp, hold) in [("full", true, true, true), ("plain", false, false, false)] {
        let o = POpt { rope: Opt { cap, trap, damp, hold }, lift: true };
        println!("== variant {name}");
        stats("REAL", &real, o, &rates, 1.0);
        stats("SYNTH", &syn, o, &rates, 1.0);
    }
}
