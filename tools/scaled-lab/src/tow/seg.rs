//! Tow segments and rope-only simulation at a rate.
use super::rec::*;
use super::scaled::*;

pub const RATES: [u32; 10] = [30, 60, 75, 90, 100, 120, 144, 165, 200, 137];

#[derive(Clone)]
pub struct Seg {
    pub name: String,
    pub recs: Vec<Rec>,
}

/// Maximal runs of steps where the same slot is held and active in the pre-state.
pub fn segments(name: &str, recs: &[Rec], min: usize) -> Vec<Seg> {
    let mut out = vec![];
    let mut i = 0;
    while i < recs.len() {
        if recs[i].pre_t.active && recs[i].state == 1 {
            let slot = recs[i].pre_t.slot;
            let mut j = i;
            while j < recs.len() && recs[j].pre_t.active && recs[j].pre_t.slot == slot && recs[j].state == 1 && recs[j].post_t.slot == slot {
                j += 1;
            }
            if j - i >= min {
                out.push(Seg { name: format!("{name}@{}", recs[i].step), recs: recs[i..j].to_vec() });
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

pub fn fq16(r: u32) -> u32 {
    (65536.0 * 30.0 / r as f64).round() as u32
}
pub fn fq32(r: u32) -> u64 {
    (4294967296.0 * 30.0 / r as f64).round() as u64
}

pub fn thrust_of(r: &Rec) -> bool {
    r.held & 0x40 != 0 && r.post_p.fuel < r.pre_p.fuel && r.state == 1
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub a: f64, // unwrapped angle units
    pub w: f64, // W (1.0 = 65536 in W24)
}

fn unwrap_near(x: f64, reference: f64) -> f64 {
    let mut d = x - reference;
    while d > 64.0 { d -= 128.0 }
    while d < -64.0 { d += 128.0 }
    reference + d
}

/// Per-tick options for the rope-only simulation.
#[derive(Clone, Copy)]
pub struct Opt {
    pub cap: Cap,
    pub trap: bool,
    pub damp: bool,
    pub hold: bool,
}

/// Simulates the rope over the segment at rate `rate`, returns state at the end of each original
/// step (interpolated), `resync` > 0 resets to the real state every that many steps.
pub fn simulate(seg: &Seg, rate: u32, opt: Opt, resync: usize) -> Vec<Sample> {
    let n = seg.recs.len();
    let f = fq16(rate);
    let f32_ = fq32(rate);
    let first = &seg.recs[0].pre_t;
    let mut rope = Rope::from_real(first.angle, first.speed, first.sense, first.mass);
    let mut out = Vec::with_capacity(n);
    let mut t: u64 = 0;
    let mut prev = Sample { a: rope.a as f64 / (1u64 << 24) as f64, w: rope.w as f64 / (1u64 << 24) as f64 };
    let mut next_sample = 1usize; // sample at end of step index next_sample-1 => time = next_sample
    let mut last_resync_k = 0usize;
    let _ = last_resync_k;
    while out.len() < n {
        let t0 = t;
        t += f32_;
        let mid = ((t0 + t) / 2) >> 32;
        let idx = (mid as usize).min(n - 1);
        let r = &seg.recs[idx];
        let thrust = thrust_of(r);
        let ws_old = rope.ws();
        let tau_t = t0;
    let tau = if opt.hold { (((tau_t & 0xFFFF_FFFF) >> 16) as u32).min(65535) } else { 0 };
        let ang = rope.hold_angle(tau);
        if !r.anti {
            rope.pull_at(ang, r.g_a, !r.g_down, f, opt.cap);
        }
        if thrust {
            rope.torque_at(ang, r.post_p.facing, true, 0, f, opt.cap);
        }
        rope.swing_v(f, ws_old, opt.trap, opt.damp);
        let cur = Sample { a: unwrap_near(rope.a as f64 / (1u64 << 24) as f64, prev.a), w: rope.w as f64 / (1u64 << 24) as f64 };
        // emit samples whose time is within (t0, t]
        while next_sample <= n && ((next_sample as u64) << 32) <= t {
            let tk = (next_sample as u64) << 32;
            let frac = (tk - t0) as f64 / (t - t0) as f64;
            let s = Sample { a: prev.a + (cur.a - prev.a) * frac, w: prev.w + (cur.w - prev.w) * frac };
            out.push(s);
            if resync > 0 && next_sample % resync == 0 && next_sample < n {
                // reset to the real state at this time (post of step next_sample-1)
                let p = &seg.recs[next_sample - 1].post_t;
                rope = Rope::from_real(p.angle, p.speed, p.sense, p.mass);
                // keep the sub-tick phase: nothing to do (state reset at sample time only exact at aligned ticks)
            }
            next_sample += 1;
        }
        prev = Sample { a: unwrap_near(rope.a as f64 / (1u64 << 24) as f64, cur.a), w: rope.w as f64 / (1u64 << 24) as f64 };
        let _ = &mut last_resync_k;
    }
    out
}

/// Real samples (end of each step), angle unwrapped.
pub fn real_samples(seg: &Seg) -> Vec<Sample> {
    let mut out: Vec<Sample> = vec![];
    for r in &seg.recs {
        let a = r.post_t.angle as f64 / 256.0;
        let a = match out.last() { Some(p) => unwrap_near(a, p.a), None => unwrap_near(a, r.pre_t.angle as f64 / 256.0) };
        out.push(Sample { a, w: r.post_t.speed as f64 / 65536.0 });
    }
    out
}

/// One piece of a tick (never straddling a step boundary): forces, swing. `t0` start in Q32 steps
/// since the window start, `dur` its length, `idx` the original step it lies in.
pub fn piece(rope: &mut Rope, r: &Rec, t0: u64, dur: u64, opt: Opt) {
    let ff = (dur >> 16) as u32;
    if ff == 0 {
        return;
    }
    let ws_old = rope.ws();
    rope.rest = { let fr = ((t0 + dur) & 0xFFFF_FFFF) >> 16; if fr == 0 { 0 } else { 65536 - fr as u32 } };
    let tau_t = if std::env::var("TAUMID").is_ok() { t0 + dur / 2 } else { t0 };
    let tau = if opt.hold { (((tau_t & 0xFFFF_FFFF) >> 16) as u32).min(65535) } else { 0 };
    let ang = rope.hold_angle(tau);
    if !r.anti {
        rope.pull_at(ang, r.g_a, !r.g_down, ff, opt.cap);
    }
    if thrust_of(r) {
        rope.torque_at(ang, r.post_p.facing, true, 0, ff, opt.cap);
    }
    rope.swing_v(ff, ws_old, opt.trap, opt.damp);
}

/// Window test: from the real state at step `k0`, run `h` steps at `rate`; returns errors at the
/// end of each step 1..=h (angle, W), interpolated at non-aligned ticks. Ticks that straddle a step
/// boundary are split there so that each part gets its own step's input (the ideal continuous input).
/// `phase` shifts the tick grid (fraction of a tick).
pub fn window(seg: &Seg, k0: usize, h: usize, rate: u32, opt: Opt, phase: f64) -> Vec<(f64, f64)> {
    let f32_ = fq32(rate);
    let n = seg.recs.len();
    let h = h.min(n - k0);
    let first = &seg.recs[k0].pre_t;
    let mut rope = Rope::from_real(first.angle, first.speed, first.sense, first.mass);
    let real = real_samples(seg);
    let mut out = vec![];
    let mut t: u64 = 0;
    let mut next = 1usize;
    let mut tick_end: u64 = ((f32_ as f64) * (1.0 - phase)).max(1.0) as u64;
    let mut prev_state = Sample { a: rope.a as f64 / (1u64 << 24) as f64, w: rope.w as f64 / (1u64 << 24) as f64 };
    let mut tick_start_state = prev_state;
    let mut tick_start_t = 0u64;
    while out.len() < h {
        // advance one tick from t to tick_end in pieces
        let mut ts = t;
        if std::env::var("NOSPLIT").is_ok() {
            let idx = (((t + tick_end) / 2 >> 32) as usize).min(h - 1);
            piece(&mut rope, &seg.recs[k0 + idx], ts, tick_end - ts, opt);
        } else {
        while ts < tick_end {
            let boundary = ((ts >> 32) + 1) << 32;
            let te = tick_end.min(boundary);
            let idx = ((ts >> 32) as usize).min(h - 1);
            piece(&mut rope, &seg.recs[k0 + idx], ts, te - ts, opt);
            ts = te;
        }
        }
        let cur = Sample { a: unwrap_near(rope.a as f64 / (1u64 << 24) as f64, tick_start_state.a), w: rope.w as f64 / (1u64 << 24) as f64 };
        while next <= h && ((next as u64) << 32) <= tick_end {
            let tk = (next as u64) << 32;
            let frac = (tk - tick_start_t) as f64 / (tick_end - tick_start_t) as f64;
            let sm = Sample { a: tick_start_state.a + (cur.a - tick_start_state.a) * frac, w: tick_start_state.w + (cur.w - tick_start_state.w) * frac };
            let rl = real[k0 + next - 1];
            let ra = unwrap_near(rl.a, sm.a);
            out.push((sm.a - ra, sm.w - rl.w));
            next += 1;
        }
        t = tick_end;
        tick_start_t = t;
        tick_start_state = cur;
        let _ = &mut prev_state;
        tick_end += f32_;
    }
    out
}

pub fn dataset(world: &solar_jetman::game::level::World, real: bool, synth: bool) -> Vec<Seg> {
    let mut v = vec![];
    if real {
        for n in ["tow", "delivery", "orb-held"] {
            let recs = replay_recs_with(world, n, false).unwrap();
            v.extend(segments(n, &recs, 20));
        }
    }
    if synth {
        for level in 0..=12usize {
            for pick in 0..3usize {
                for mode in 0..3u32 {
                    let recs = synth_tow(world, level, pick as u8, pick, mode, 1 + level as u64 * 7 + pick as u64, 800, false);
                    v.extend(segments(&format!("syn-L{level}-p{pick}-m{mode}"), &recs, 20));
                }
            }
        }
    }
    v
}

/// Free-run simulation from the start of a segment; returns (A, W) samples per step plus snap counts.
pub fn freerun(seg: &Seg, rate: u32, opt: Opt) -> (Vec<Sample>, u32, u32) {
    let n = seg.recs.len();
    let f32_ = fq32(rate);
    let first = &seg.recs[0].pre_t;
    let mut rope = Rope::from_real(first.angle, first.speed, first.sense, first.mass);
    let mut out = vec![];
    let mut t: u64 = 0;
    let end = (n as u64) << 32;
    let mut next = 1usize;
    let mut prev = Sample { a: rope.a as f64 / 16777216.0, w: rope.w as f64 / 16777216.0 };
    let mut t_prev = 0u64;
    let mut tick_end = f32_;
    while out.len() < n {
        let mut ts = t;
        while ts < tick_end {
            let boundary = ((ts >> 32) + 1) << 32;
            let te = tick_end.min(boundary);
            let idx = ((ts >> 32) as usize).min(n - 1);
            piece(&mut rope, &seg.recs[idx], ts, te - ts, opt);
            ts = te;
        }
        let cur = Sample { a: unwrap_near(rope.a as f64 / 16777216.0, prev.a), w: rope.w as f64 / 16777216.0 };
        while next <= n && ((next as u64) << 32) <= tick_end {
            let frac = (((next as u64) << 32) - t_prev) as f64 / (tick_end - t_prev) as f64;
            out.push(Sample { a: prev.a + (cur.a - prev.a) * frac, w: prev.w + (cur.w - prev.w) * frac });
            next += 1;
        }
        t = tick_end;
        t_prev = t;
        prev = cur;
        tick_end += f32_;
        if t > end + f32_ { break; }
    }
    (out, rope.snap0, rope.snap1)
}
