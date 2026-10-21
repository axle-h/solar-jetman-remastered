use exp::tow::rec::*;
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn all_segs(world: &solar_jetman::game::level::World) -> Vec<Seg> {
    let mut v = vec![];
    for n in ["tow", "delivery", "orb-held"] {
        let recs = replay_recs(world, n).unwrap();
        v.extend(segments(n, &recs, 20));
    }
    v
}
fn main() {
    let world = exp::world();
    let segs = all_segs(&world);
    for s in &segs { println!("{} len {}", s.name, s.recs.len()); }
    for (trap, damp) in [(false, false), (true, false), (false, true), (true, true)] {
      let cap = Cap::Glitch;
        for resync in [0usize, 30] {
            println!("trap {trap} damp {damp} resync {resync}");
            for &rate in &RATES {
                let (mut sq, mut sw, mut n, mut ma, mut mw) = (0.0, 0.0, 0usize, 0.0f64, 0.0f64);
                for s in &segs {
                    let sim = simulate(s, rate, Opt { cap, trap, damp, hold: false }, resync);
                    let real = real_samples(s);
                    for k in 0..real.len() {
                        let e = sim[k].a - real[k].a; let ew = sim[k].w - real[k].w;
                        sq += e * e; sw += ew * ew; n += 1; ma = ma.max(e.abs()); mw = mw.max(ew.abs());
                    }
                }
                println!("R {rate:3}: angle max {ma:7.3} rms {:6.3} | W max {mw:6.3} rms {:6.3}", (sq / n as f64).sqrt(), (sw / n as f64).sqrt());
            }
        }
    }
}
