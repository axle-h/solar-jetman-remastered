//! Free-flight pod model with a towed load, per tick of fraction f (Q16). U = 2^-24 px/step.
use super::rec::*;
use super::scaled::*;
use super::seg::*;
use solar_jetman::game::gameplay::tables::THRUST;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pod {
    pub x: i64, // 2^-24 px
    pub y: i64,
    pub hm: u64, // magnitudes in U
    pub hl: bool,
    pub vm: u64,
    pub vd: bool,
}

const CAP3: u64 = 3 << 24;
const CAP4: u64 = 4 << 24;

impl Pod {
    pub fn from_p(p: &P) -> Pod {
        Pod {
            x: ((i64::from(p.x) << 8) | i64::from(p.xf)) << 16,
            y: ((i64::from(p.y) << 8) | i64::from(p.yf)) << 16,
            hm: ((u64::from(p.h) << 8) | u64::from(p.ah)) << 8,
            hl: p.hl,
            vm: ((u64::from(p.v) << 8) | u64::from(p.av)) << 8,
            vd: p.vd,
        }
    }
    /// position in 2^-8 px (as the original's 16.8), the quantity the engine keeps
    pub fn x8(&self) -> f64 { self.x as f64 / 65536.0 }
    pub fn y8(&self) -> f64 { self.y as f64 / 65536.0 }
}

fn trunc(m: u64) -> i64 {
    (m & !0xFFFF) as i64
}
fn signed(m: u64, pos: bool) -> i64 {
    if pos { trunc(m) } else { -trunc(m) }
}

/// pull_axis: add in the direction, or subtract with discard at zero.
fn pull_axis(mag: u64, positive: bool, amount: i64) -> (u64, bool) {
    if amount == 0 {
        return (mag, positive);
    }
    let by = amount.unsigned_abs();
    if (amount > 0) == positive {
        (mag + by, positive)
    } else if mag >= by {
        (mag - by, positive)
    } else {
        (0, !positive)
    }
}

#[derive(Clone, Copy)]
pub struct POpt {
    pub rope: Opt,
    /// use W held at step start for the pull on the pod (not implemented: placeholder)
    pub lift: bool,
}

/// One piece of a tick. Returns nothing; updates pod and rope.
pub fn piece(pod: &mut Pod, rope: &mut Rope, r: &Rec, t0: u64, dur: u64, o: POpt) {
    let f = (dur >> 16) as u32;
    if f == 0 {
        return;
    }
    let fq = u64::from(f);
    let opt = o.rope;
    let ws_old = rope.ws();
    rope.rest = { let fr = ((t0 + dur) & 0xFFFF_FFFF) >> 16; if fr == 0 { 0 } else { 65536 - fr as u32 } };
    let tau_t = if std::env::var("TAUMID").is_ok() { t0 + dur / 2 } else { t0 };
    let tau = if opt.hold { (((tau_t & 0xFFFF_FFFF) >> 16) as u32).min(65535) } else { 0 };
    let ang = rope.hold_angle(tau);
    // velocities at tick start (signed, truncated to 2^-8 for the position integral)
    let (hv0, vv0) = (signed(pod.hm, !pod.hl), signed(pod.vm, pod.vd));
    // 1. pull on the pod
    {
        let save = rope.a;
        // pull_on_pod uses the held angle
        rope.a = (u32::from(ang) << 24) | (rope.a & 0x00FF_FFFF);
        let (pr, pd) = rope.pull_on_pod(f);
        rope.a = save;
        let h = pull_axis(pod.hm, !pod.hl, pr);
        let v = pull_axis(pod.vm, pod.vd, pd);
        pod.hm = h.0;
        pod.hl = !h.1;
        pod.vm = v.0;
        pod.vd = v.1;
    }
    // 2. gravity
    if !r.anti {
        let d = u64::from(r.g_a) * fq;
        if pod.vd == r.g_down {
            pod.vm += d;
            if pod.vm >> 24 >= 3 {
                pod.vm = CAP3;
            }
        } else if pod.vm > d {
            pod.vm -= d;
        } else {
            pod.vm = d - pod.vm;
            pod.vd = r.g_down;
        }
        // 3. lift and the pull on W
        if o.lift {
            let amount = rope.lift(r.g_a, f).unsigned_abs();
            let angle = ang;
            let adds = (32..96).contains(&angle) == pod.vd;
            if adds {
                pod.vm += amount;
            } else if pod.vm >= amount {
                pod.vm -= amount;
            } else {
                pod.vm = 0;
                pod.vd = !pod.vd;
            }
        }
        rope.pull_at(ang, r.g_a, !r.g_down, f, opt.cap);
    }
    // 4. caps
    pod.hm = pod.hm.min(CAP4);
    pod.vm = pod.vm.min(CAP4);
    let (hv1, vv1) = (signed(pod.hm, !pod.hl), signed(pod.vm, pod.vd));
    // 5. thrust (after the move; applied to velocities now, position handled below)
    let ws_old_rope = ws_old;
    if thrust_of(r) {
        let fac = r.post_p.facing;
        let fi = usize::from(fac);
        let (tv, th) = (u32::from(THRUST[fi & 15]), u32::from(THRUST[(fi + 8) & 15]));
        let (av0, ah0) = ((tv / 2) / 2, (th / 2) / 2);
        let scale = rope.thrust_scale(fac);
        let (pv, ph) = (u64::from(av0 * scale), u64::from(ah0 * scale)); // 2^-16 per step^2
        let dv = pv * fq >> 8; // U
        let dh = ph * fq >> 8;
        let thrust_down = (9..=23).contains(&fi);
        let thrust_left = (17..=31).contains(&fi);
        if thrust_down == pod.vd {
            pod.vm += dv;
            if pod.vm >> 24 >= 3 { pod.vm = CAP3; }
        } else if pod.vm > dv {
            pod.vm -= dv;
        } else {
            pod.vm = dv - pod.vm;
            pod.vd = thrust_down;
            pod.vm += dv;
            if pod.vm >> 24 >= 3 { pod.vm = CAP3; }
        }
        if (ph >> 8) != 0 {
            if thrust_left == pod.hl {
                pod.hm += dh;
                if pod.hm >> 24 >= 3 { pod.hm = CAP3; }
            } else if pod.hm > dh {
                pod.hm -= dh;
            } else {
                pod.hm = dh - pod.hm;
                pod.hl = thrust_left;
            }
        }
        rope.torque_at(ang, fac, true, 0, f, opt.cap);
    }
    let (hv2, vv2) = (signed(pod.hm, !pod.hl), signed(pod.vm, pod.vd));
    // position: trapezoid on the whole tick, with the original's offsets: pre-move forces count
    // half an extra tick ahead, post-move (thrust) forces half a tick behind.
    let upd = |v0: i64, v1: i64, v2: i64| -> i64 { (i64::from(f) * (v0 + v2) / 2 + 65536 * (v1 - v0) / 2 - 65536 * (v2 - v1) / 2) >> 16 };
    pod.x += upd(hv0, hv1, hv2);
    pod.y += upd(vv0, vv1, vv2);
    rope.swing_v(f, ws_old_rope, opt.trap, opt.damp);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Err {
    pub pos: f64, // px
    pub vel: f64, // px/step
    pub ang: f64,
    pub item: f64, // px, item position error (integer pod + rope offset)
    pub a8mis: i32, // scaled angle8 minus real angle8 (wrapped)
}

/// Spin events: (time in steps from the window start).
pub fn spin_events(seg: &Seg, k0: usize, h: usize, rate: u32, o: POpt) -> (Vec<f64>, Vec<f64>) {
    let (m, r) = window_full(seg, k0, h, rate, o, 0.0);
    let _ = m;
    (r.0, r.1)
}

/// Window: run `h` steps from the real state at `k0`; error vs real after each step.
pub fn window(seg: &Seg, k0: usize, h: usize, rate: u32, o: POpt, phase: f64) -> Vec<Err> {
    window_full(seg, k0, h, rate, o, phase).0
}

fn spin_step(spin: &mut u8, angle: u8) -> bool {
    if angle >> 4 == 4 {
        *spin |= 0x80;
    } else if angle >> 4 == 7 && *spin & 0x80 != 0 {
        *spin = 0;
        return true;
    }
    false
}

/// Returns the errors and ((model event times), (real event times)) in steps from the window start.
pub fn window_full(seg: &Seg, k0: usize, h: usize, rate: u32, o: POpt, phase: f64) -> (Vec<Err>, (Vec<f64>, Vec<f64>)) {
    let f32_ = fq32(rate);
    let n = seg.recs.len();
    let h = h.min(n - k0);
    let first = &seg.recs[k0];
    let mut rope = Rope::from_real(first.pre_t.angle, first.pre_t.speed, first.pre_t.sense, first.pre_t.mass);
    let mut pod = Pod::from_p(&first.pre_p);
    let mut out = vec![];
    let mut t: u64 = 0;
    let mut next = 1usize;
    let mut tick_end: u64 = ((f32_ as f64) * (1.0 - phase)).max(1.0) as u64;
    let mut st0 = (pod, rope);
    let mut t_start = 0u64;
    let mut spin_m = 0u8;
    let mut spin_r = 0u8;
    let mut ev_m = vec![];
    let mut ev_r = vec![];
    for i in 0..h {
        if spin_step(&mut spin_r, (seg.recs[k0 + i].post_t.angle >> 8) as u8) {
            ev_r.push((i + 1) as f64);
        }
    }
    while out.len() < h {
        let mut ts = t;
        if std::env::var("NOSPLIT").is_ok() {
            let idx = (((t + tick_end) / 2 >> 32) as usize).min(h - 1);
            piece(&mut pod, &mut rope, &seg.recs[k0 + idx], ts, tick_end - ts, o);
        } else {
        while ts < tick_end {
            let boundary = ((ts >> 32) + 1) << 32;
            let te = tick_end.min(boundary);
            let idx = ((ts >> 32) as usize).min(h - 1);
            piece(&mut pod, &mut rope, &seg.recs[k0 + idx], ts, te - ts, o);
            ts = te;
        }
        }
        {
            let tau = if o.rope.hold { (((tick_end & 0xFFFF_FFFF) >> 16) as u32).min(65535) } else { 0 };
            if spin_step(&mut spin_m, rope.hold_angle(tau)) {
                ev_m.push(tick_end as f64 / 4294967296.0);
            }
        }
        while next <= h && ((next as u64) << 32) <= tick_end {
            let tk = (next as u64) << 32;
            let frac = (tk - t_start) as f64 / (tick_end - t_start) as f64;
            let rl = &seg.recs[k0 + next - 1].post_p;
            let rt = &seg.recs[k0 + next - 1].post_t;
            let lerp = |a: f64, b: f64| a + (b - a) * frac;
            let (px, py) = (lerp(st0.0.x8(), pod.x8()), lerp(st0.0.y8(), pod.y8()));
            let real = Pod::from_p(rl);
            let (ex, ey) = ((px - real.x8()) / 256.0, (py - real.y8()) / 256.0);
            let sv = |p: &Pod| (if p.hl { -(p.hm as f64) } else { p.hm as f64 } / 16777216.0, if p.vd { p.vm as f64 } else { -(p.vm as f64) } / 16777216.0);
            let (a, b) = (sv(&st0.0), sv(&pod));
            let (rv0, rv1) = (sv(&real).0, sv(&real).1);
            let (evx, evy) = (lerp(a.0, b.0) - rv0, lerp(a.1, b.1) - rv1);
            let ra = rt.angle as f64 / 256.0;
            let sa = lerp(st0.1.a as f64 / 16777216.0, {
                let mut x = rope.a as f64 / 16777216.0;
                let s0 = st0.1.a as f64 / 16777216.0;
                while x - s0 > 64.0 { x -= 128.0 }
                while x - s0 < -64.0 { x += 128.0 }
                x
            });
            let mut ea = sa - ra;
            while ea > 64.0 { ea -= 128.0 }
            while ea < -64.0 { ea += 128.0 }
            let rope_off = |a8: u8| solar_jetman::game::gameplay::tow::rope_offset(a8);
            let a8s = (sa.rem_euclid(128.0)) as u8;
            let a8r = (rt.angle >> 8) as u8;
            let (ox, oy) = rope_off(a8s);
            let (rx, ry) = rope_off(a8r);
            let itx = ((px / 256.0).floor() as i64 + i64::from(ox)) - (rl.x as i64 + i64::from(rx));
            let ity = ((py / 256.0).floor() as i64 + i64::from(oy)) - (rl.y as i64 + i64::from(ry));
            let mut dm = i32::from(a8s) - i32::from(a8r);
            if dm > 64 { dm -= 128 } else if dm < -64 { dm += 128 }
            out.push(Err { pos: (ex * ex + ey * ey).sqrt(), vel: (evx * evx + evy * evy).sqrt(), ang: ea, item: ((itx * itx + ity * ity) as f64).sqrt(), a8mis: dm });
            next += 1;
        }
        t = tick_end;
        t_start = t;
        st0 = (pod, rope);
        tick_end += f32_;
    }
    (out, (ev_m, ev_r))
}
