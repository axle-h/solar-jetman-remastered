use exp::tow::podm::*;
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let real = dataset(&world, false, true);
    let o = POpt { rope: Opt { cap: Cap::GlitchScaled, trap: true, damp: true, hold: true }, lift: true };
    let mut shown = 0;
    let mut tot = 0; let mut bad = 0;
    for s in &real {
        for k in 0..s.recs.len() {
            let r = &s.recs[k];
            if r.surface != 0 || r.pre_p.y < 24 { continue; }
            let e = exp::tow::podm::window(s, k, 1, 30, o, 0.0);
            tot += 1;
            if e[0].pos > 0.1 || e[0].vel > 0.05 {
                bad += 1;
                if shown < 12 { shown += 1;
                    println!("{} k {k}: pos err {:.4} vel err {:.5} ang {:.3} held {:02x} fac {} pre h {} {} ({}) v {} {} ({}) ah {} av {} A {} W {:x} | post h {} {} v {} {}  g {} {}", s.name, e[0].pos, e[0].vel, e[0].ang, r.held, r.post_p.facing, r.pre_p.h, r.pre_p.hl, r.pre_p.ah, r.pre_p.v, r.pre_p.vd, r.pre_p.av, r.pre_p.ah, r.pre_p.av, r.pre_t.angle>>8, r.pre_t.speed, r.post_p.h, r.post_p.hl, r.post_p.v, r.post_p.vd, r.g_a, r.g_down);
                }
            }
        }
    }
    println!("tot {tot} bad {bad}");
}
