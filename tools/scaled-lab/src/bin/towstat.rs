use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, false, true);
    let pump: Vec<_> = segs.iter().filter(|s| s.name.contains("-m2")).collect();
    let rnd: Vec<_> = segs.iter().filter(|s| !s.name.contains("-m2")).collect();
    for (label, set) in [("pump", &pump), ("random", &rnd)] {
        // original statistics
        let mut n = 0usize; let (mut sw, mut ab) = (0.0f64, 0usize); let mut hist = [0usize; 5];
        let mut sa = 0.0f64;
        for s in set.iter() { for r in &s.recs { let w = r.post_t.speed as f64 / 65536.0; n += 1; sw += w; if w >= 1.0 { ab += 1; } hist[((w * 2.5) as usize).min(4)] += 1; let a = (r.post_t.angle >> 8) as f64; sa += ((a - 64.0 + 64.0).rem_euclid(128.0) - 64.0).abs(); } }
        println!("{label}: ORIGINAL: steps {n} mean W {:.3} frac W>=1 {:.4} hist(W*2.5) {:?} mean |A-hang| {:.2}", sw / n as f64, ab as f64 / n as f64, hist.iter().map(|h| (100.0 * *h as f64 / n as f64).round()).collect::<Vec<_>>(), sa / n as f64);
        for (cn, cap) in [("Glitch", Cap::Glitch), ("GlitchScaled", Cap::GlitchScaled), ("PendHalf", Cap::GlitchPendHalf)] {
            for rate in [30u32, 60, 144] {
                let mut n = 0usize; let (mut sw, mut ab) = (0.0f64, 0usize); let mut hist = [0usize; 5]; let mut sa = 0.0f64; let (mut s0, mut s1) = (0, 0);
                for s in set.iter() {
                    let (smp, a0, a1) = freerun(s, rate, Opt { cap, trap: true, damp: true, hold: true });
                    s0 += a0; s1 += a1;
                    for x in &smp { n += 1; sw += x.w; if x.w >= 1.0 { ab += 1; } hist[((x.w * 2.5) as usize).min(4)] += 1; sa += ((x.a - 64.0 + 64.0).rem_euclid(128.0) - 64.0).abs(); }
                }
                println!("   {cn:13} R{rate:3}: mean W {:.3} frac W>=1 {:.4} hist {:?} mean |A-hang| {:.2} snaps below/above {s0}/{s1}", sw / n as f64, ab as f64 / n as f64, hist.iter().map(|h| (100.0 * *h as f64 / n as f64).round()).collect::<Vec<_>>(), sa / n as f64);
            }
        }
    }
}
