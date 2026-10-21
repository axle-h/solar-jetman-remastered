use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, false, true);
    let pump: Vec<_> = segs.iter().filter(|s| s.name.contains("-m2")).collect();
    let rnd: Vec<_> = segs.iter().filter(|s| !s.name.contains("-m2")).collect();
    let real = dataset(&world, true, false);
    let realr: Vec<_> = real.iter().collect();
    for (label, set) in [("real", &realr), ("random", &rnd), ("pump", &pump)] {
        for (cn, cap) in [("Scaled", Cap::GlitchScaled), ("PendH", Cap::GlitchPendHalf)] {
            for rate in [60u32, 100, 144, 200] {
                let mut errs = vec![]; let mut werrs = vec![]; let mut ws = 0.0f64; let mut nw = 0;
                for s in set.iter() {
                    let n = s.recs.len(); let mut k0 = 0;
                    while k0 + 30 <= n {
                        let e = window(s, k0, 30, rate, Opt { cap, trap: true, damp: true, hold: true }, 0.0);
                        errs.push(e[29].0.abs()); werrs.push(e[9].1); ws += e[9].1; nw += 1;
                        k0 += 7;
                    }
                }
                errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let n = errs.len();
                let rms = (errs.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt();
                let wabs: f64 = werrs.iter().map(|x| x.abs()).sum::<f64>() / n as f64;
                println!("{label:6} {cn:6} R{rate:3}: n {n} angle@1s P50 {:.3} P90 {:.3} P99 {:.2} rms {:.3} | W@10steps bias {:+.4} mean|err| {:.4}", errs[n / 2], errs[n * 9 / 10], errs[n * 99 / 100], rms, ws / nw as f64, wabs);
            }
        }
    }
}
