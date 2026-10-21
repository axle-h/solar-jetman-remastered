use exp::tow::rec::*;
use exp::tow::scaled::*;
use exp::tow::seg::*;
use solar_jetman::game::gameplay::tow::Tow;
fn main() {
    let world = exp::world();
    // 1. rope update vs the real engine, original-rules replays, all steps with a rope or a thrust
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    let (mut ok, mut bad, mut skipped) = (0, 0, 0);
    let cap = Cap::GlitchPendHalf;
    let f = 65536;
    for n in names {
        let Some(recs) = replay_recs(&world, &n) else { continue };
        for r in &recs {
            let pre = r.pre_t;
            let mut ro = Rope::from_real(pre.angle, pre.speed, pre.sense, pre.mass);
            let thrust = r.held & 0x40 != 0 && r.post_p.fuel < r.pre_p.fuel && r.state == 1;
            if pre.slot != post_slot(r) { skipped += 1; continue; }
            if pre.slot == 0 && !thrust { continue; }
            let ws_old = ro.ws();
            ro.rest = 0;
            let ang = ro.hold_angle(0);
            if pre.active && !r.anti { ro.pull_at(ang, r.g_a, !r.g_down, f, cap); }
            if thrust { ro.torque_at(ang, r.post_p.facing, pre.active, 20 + (r.post_p.y & 15) as u8, f, cap); }
            if pre.slot != 0 && !pre.taken_in { ro.swing_v(f, ws_old, true, true); }
            let post = r.post_t;
            if (post.angle, post.speed, post.sense) == (ro.angle16(), ro.speed24(), ro.sense) { ok += 1 } else { bad += 1; if bad < 10 { println!("mismatch {n} step {}", r.step); } }
        }
    }
    println!("rope vs real engine (all replays, R=30): exact {ok}, mismatch {bad}, skipped {skipped}");
    // 2. pull_on_pod, lift, thrust_scale, vs Tow at random states
    let mut x = 0x1234_5678_9abc_def1u64;
    let mut nx = || { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x };
    let (mut bad_p, mut bad_l, mut bad_s) = (0, 0, 0);
    for _ in 0..200_000 {
        let angle = (nx() % 32768) as u16;
        let speed = (nx() % 0x30000) as u32;
        let mass = (nx() % 3) as u8;
        let a = (nx() % 32) as u16;
        let t = Tow { angle, speed, mass, ..Tow::default() };
        let ro = Rope::from_real(angle, speed, false, mass);
        let (h, v) = t.pull_on_pod();
        let (h2, v2) = ro.pull_on_pod(65536);
        if (i64::from(h) * 256, i64::from(v) * 256) != (h2, v2) { bad_p += 1; }
        if i64::from(t.lift(a).0) * 256 != ro.lift(a, 65536) { bad_l += 1; }
        let fac = (nx() % 32) as u8;
        if u32::from(t.thrust_scale(fac)) != ro.thrust_scale(fac) { bad_s += 1; }
    }
    println!("pull_on_pod / lift / thrust_scale at f=1 vs src/game on 200k random states: mismatches {bad_p} {bad_l} {bad_s}");
}
fn post_slot(r: &Rec) -> usize { r.post_t.slot }
