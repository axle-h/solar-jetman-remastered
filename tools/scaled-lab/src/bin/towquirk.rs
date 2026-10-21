use exp::tow::scaled::*;
use exp::tow::seg::*;
fn run(seg: &Seg, low: u32, sense: bool, n: usize) -> Vec<(f64, f64)> {
    let first = &seg.recs[0].pre_t;
    let mut ro = Rope::from_real(first.angle, low, sense, first.mass);
    let mut out = vec![];
    for r in seg.recs.iter().take(n) {
        let ws_old = ro.ws();
        let ang = ro.hold_angle(0);
        if !r.anti { ro.pull_at(ang, r.g_a, !r.g_down, 65536, Cap::GlitchPendHalf); }
        if thrust_of(r) { ro.torque_at(ang, r.post_p.facing, true, 0, 65536, Cap::GlitchPendHalf); }
        ro.swing_v(65536, ws_old, true, true);
        out.push((ro.a as f64 / 16777216.0, ro.w as f64 / 16777216.0));
    }
    out
}
const HORIZON: usize = 120;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, true, true);
    let mut worst = [0.0f64; 4];
    let mut nseg = 0;
    let mut sum = [0.0f64; 4];
    for s in &segs {
        if s.recs.len() < 130 { continue; }
        nseg += 1;
        let base = run(s, 0, false, 120);
        // the real catch: W upper bytes zero, low byte stale: 0..255 (in W24 units), sense stale
        for (vi, (low, sense)) in [(0u32, true), (0xFFu32, false), (0x80, true)].iter().enumerate() {
            let v = run(s, *low, *sense, 120);
            let mut m = 0.0f64;
            for (a, b) in base.iter().zip(v.iter()).take(HORIZON) {
                let mut d = a.0 - b.0; while d > 64.0 { d -= 128.0 } while d < -64.0 { d += 128.0 }
                m = m.max(d.abs());
            }
            worst[vi] = worst[vi].max(m); sum[vi] += m;
        }
    }
    println!("segments {nseg}; max/mean angle difference over 120 steps (units; 1 unit = 1.57 px of item) for stale (low byte, sense) vs (0, false):");
    for (vi, name) in ["(0,true) sense only", "(0xFF,false) low byte only", "(0x80,true)"].iter().enumerate() {
        println!("  {name}: max {:.4} mean {:.4}", worst[vi], sum[vi] / nseg as f64);
    }
}
