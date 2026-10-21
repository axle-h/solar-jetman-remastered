use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, false, true);
    let s = segs.iter().find(|s| s.name == "syn-L7-p0-m0@2").unwrap();
    let k0 = 77;
    let p = &s.recs[k0].pre_t;
    let mut rope = Rope::from_real(p.angle, p.speed, p.sense, p.mass);
    let f = fq16(120);
    for tick in 0..8 {
        let r = &s.recs[k0 + tick / 4];
        let w0 = rope.w as f64 / (1u64<<24) as f64;
        let ang = rope.angle8();
        rope.pull_at(ang, r.g_a, !r.g_down, f, Cap::Glitch);
        let w1 = rope.w as f64 / (1u64<<24) as f64;
        rope.torque_at(ang, r.post_p.facing, true, 0, f, Cap::Glitch);
        let w2 = rope.w as f64 / (1u64<<24) as f64;
        println!("tick {tick}: W {w0:.4} -> pull {w1:.4} -> torque {w2:.4} (f {f}) sense {} snaps {} {}", rope.sense, rope.snap0, rope.snap1);
    }
}
