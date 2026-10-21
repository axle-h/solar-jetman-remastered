use exp::tow::podm::{Pod, POpt, piece};
use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let real = dataset(&world, true, false);
    let o = POpt { rope: Opt { cap: Cap::GlitchScaled, trap: true, damp: true, hold: true }, lift: true };
    let s = &real[0];
    for k in [21usize, 22, 40] {
        let r = &s.recs[k];
        let mut rope = Rope::from_real(r.pre_t.angle, r.pre_t.speed, r.pre_t.sense, r.pre_t.mass);
        let mut pod = Pod::from_p(&r.pre_p);
        exp::tow::podm::piece(&mut pod, &mut rope, r, 0, 1u64 << 32, o);
        let real_post = Pod::from_p(&r.post_p);
        println!("k {k}: model h16 {:x} v16 {:x} hl {} vd {} | real h16 {:x} v16 {:x} hl {} vd {}  pre h16 {:x} v16 {:x}", pod.hm >> 8, pod.vm >> 8, pod.hl, pod.vd, real_post.hm >> 8, real_post.vm >> 8, real_post.hl, real_post.vd, Pod::from_p(&r.pre_p).hm>>8, Pod::from_p(&r.pre_p).vm>>8);
        println!("   mass {} A8 {} W {:x} facing {} thrust {}  rope after: W {:x} A {:x}; real post W {:x} A {:x}", r.pre_t.mass, r.pre_t.angle>>8, r.pre_t.speed, r.post_p.facing, exp::tow::seg::thrust_of(r), rope.speed24(), rope.angle16(), r.post_t.speed, r.post_t.angle);
    }
}
