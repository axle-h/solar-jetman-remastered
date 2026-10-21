use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let segs: Vec<_> = dataset(&world, false, true).into_iter().filter(|s| s.name.contains("-m2")).collect();
    let rate = 120;
    let mut worst = vec![];
    let mut all = vec![];
    for s in &segs {
        let real = real_samples(s);
        let n = s.recs.len();
        let mut k0 = 0;
        while k0 + 30 <= n {
            let e = window(s, k0, 30, rate, Opt { cap: Cap::GlitchScaled, trap: true, damp: true, hold: true }, 0.0);
            let ea = e[29].0.abs();
            // number of real snaps (W drop to ~1.00 from higher or crossing)
            let mut snaps = 0;
            for k in k0..k0 + 30 { let r = &s.recs[k]; if r.post_t.speed >> 16 == 1 && (r.post_t.speed >> 8) & 0xFF == 0 { snaps += 1; } }
            let _ = &real;
            all.push((ea, snaps, s.name.clone(), k0));
            worst.push((ea, snaps, s.name.clone(), k0));
            k0 += 7;
        }
    }
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = all.len();
    println!("windows {n}: P50 {:.3} P90 {:.3} P99 {:.3} max {:.2}", all[n / 2].0, all[n * 9 / 10].0, all[n * 99 / 100].0, all[n - 1].0);
    // by snaps
    for lo in [0usize, 1, 5, 10] {
        let sel: Vec<_> = all.iter().filter(|a| a.1 >= lo && a.1 < lo + if lo == 0 { 1 } else { 100 }).collect();
        if sel.is_empty() { continue; }
        let rms = (sel.iter().map(|a| a.0 * a.0).sum::<f64>() / sel.len() as f64).sqrt();
        println!("snaps>={lo}: n {} rms {:.3}", sel.len(), rms);
    }
    worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for w in worst.iter().take(5) { println!("{:?}", w); }
}
