use exp::tow::scaled::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    for (label, segs) in [("real", dataset(&world, true, false)), ("synth", dataset(&world, false, true))] {
        let (mut steps, mut s0, mut s1, mut above, mut hist) = (0usize, 0u32, 0u32, 0usize, [0usize; 8]);
        let mut per_seg = vec![];
        for s in &segs {
            let first = &s.recs[0].pre_t;
            let mut rope = Rope::from_real(first.angle, first.speed, first.sense, first.mass);
            let f = 65536;
            for r in &s.recs {
                // replay real state
                let p = &r.pre_t;
                rope.a = u32::from(p.angle) << 16; rope.w = p.speed << 8; rope.sense = p.sense;
                if !r.anti { rope.pull(r.g_a, !r.g_down, f, Cap::Glitch); }
                if thrust_of(r) { rope.torque(r.post_p.facing, true, 0, f, Cap::Glitch); }
                steps += 1;
                if p.speed >= 0x10000 { above += 1; }
                let w = (p.speed >> 13).min(15) as usize; hist[(w / 2).min(7)] += 1;
            }
            per_seg.push((s.name.clone(), rope.snap0, rope.snap1, s.recs.len()));
            s0 += rope.snap0; s1 += rope.snap1;
        }
        println!("{label}: steps {steps} snaps from below 1.0 {s0}, from above 1.0 {s1}; steps with W>=1.0: {above} ({:.1}%); snaps/s: {:.3} from-above {:.3}", 100.0 * above as f64 / steps as f64, (s0 + s1) as f64 / (steps as f64 / 30.0), s1 as f64 / (steps as f64 / 30.0));
        println!("W histogram (u=W*8 pairs 0-1,2-3,..): {hist:?}");
        if label == "real" { for p in &per_seg { println!("{p:?}"); } }
    }
}
