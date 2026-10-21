use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, false, true);
    let s = segs.iter().find(|s| s.name == "syn-L5-p0-m2@2").unwrap();
    let real = real_samples(s);
    let k0 = 560;
    for rate in [120u32, 200] {
        let e = window(s, k0, 30, rate, Opt { cap: Cap::GlitchScaled, trap: true, damp: true, hold: true }, 0.0);
        println!("rate {rate}");
        for i in 0..30 {
            let r = &s.recs[k0 + i];
            println!("{i:2} real A {:7.2} W {:.4} sense {} B {} fac {:2} g {} | err A {:6.2} W {:6.3}", real[k0 + i].a, real[k0 + i].w, r.post_t.sense, (r.held >> 6) & 1, r.post_p.facing, r.g_a, e[i].0, e[i].1);
        }
    }
}
