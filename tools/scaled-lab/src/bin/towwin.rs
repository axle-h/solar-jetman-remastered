use exp::tow::scaled::*;
use exp::tow::seg::*;
fn run(label: &str, segs: &[Seg], variants: &[(bool, bool, bool)], cap: Cap) {
    let hs = [1usize, 3, 10, 30];
    for &(trap, damp, hold) in variants {
        println!("{label} cap {cap:?} trap {trap} damp {damp} hold {hold}: angle err units rms/max at 1,3,10,30 steps | W rms/max@30");
        for &rate in &RATES {
            let mut line = format!("R {rate:3}:");
            let mut acc = vec![(0.0f64, 0.0f64, 0usize, 0.0f64, 0.0f64); hs.len()];
            for s in segs {
                let n = s.recs.len();
                let mut k0 = 0;
                while k0 + 30 <= n {
                    let e = window(s, k0, 30, rate, Opt { cap, trap, damp, hold }, 0.0);
                    for (i, &h) in hs.iter().enumerate() {
                        let (ea, ew) = e[h - 1];
                        acc[i].0 += ea * ea; acc[i].1 = acc[i].1.max(ea.abs()); acc[i].2 += 1; acc[i].3 += ew * ew; acc[i].4 = acc[i].4.max(ew.abs());
                    }
                    k0 += 7;
                }
            }
            for a in &acc { line += &format!(" {:.3}/{:.2}", (a.0 / a.2 as f64).sqrt(), a.1); }
            line += &format!(" | W {:.4}/{:.2}", (acc[3].3 / acc[3].2 as f64).sqrt(), acc[3].4);
            println!("{line}");
        }
    }
}
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let syn = dataset(&world, false, true);
    let pump: Vec<Seg> = syn.iter().filter(|s| s.name.contains("-m2")).cloned().collect();
    let rnd: Vec<Seg> = syn.iter().filter(|s| !s.name.contains("-m2")).cloned().collect();
    let v = [(false, false, false), (true, true, true)];
    for cap in [Cap::GlitchPendHalf] {
        run("REAL", &real, &v, cap);
        run("RANDOM", &rnd, &v, cap);
        run("PUMP", &pump, &v, cap);
    }
}
