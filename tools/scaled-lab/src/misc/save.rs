//! Area 5: rate-free save states. A mini pod (free flight + rotation + autofire + camera rule F) and the contact model
//! are saved mid-step at 144 Hz and loaded at other rates; the following 10 s are compared with an uninterrupted 144 Hz run.
use super::common::*;
use crate::contact::{Init, M, Rules, Sim, Scen, rules_from, scenarios};
use crate::motion::integ::{ONE, Opts, Order, S, TickIn, tick};
use crate::motion::rec::{Thrust, accel};
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::tables::THRUST;
use solar_jetman::game::level::World;

// ---------------- input script: piecewise constant in game time (Q16 steps), real-valued boundaries ----------------
#[derive(Clone)]
pub struct Script {
    segs: Vec<(u64, u8)>,
}
impl Script {
    pub fn make(seed: u64, total_steps: u64, rot: bool) -> Script {
        let mut r = Xs(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let mut segs = vec![(0u64, 0u8)];
        let mut t = 0u64;
        while t < total_steps * 65536 {
            let len = 65536 * 3 + r.below(65536 * 33);
            let mut b = 0u8;
            if r.below(100) < 70 {
                b |= Buttons::B;
            }
            match r.below(4) {
                0 if rot => b |= Buttons::LEFT,
                1 if rot => b |= Buttons::RIGHT,
                _ => {}
            }
            if r.below(100) < 35 {
                b |= Buttons::A;
            }
            segs.push((t, b));
            t += len;
        }
        Script { segs }
    }
    pub fn at(&self, t: u64) -> u8 {
        let i = self.segs.partition_point(|s| s.0 <= t);
        self.segs[i.saturating_sub(1)].1
    }
}

// ---------------- the mini pod ----------------
#[derive(Clone, Debug)]
pub struct Pod {
    /// game time, Q32 steps
    pub gt: u64,
    pub s: S,
    pub facing: u8,
    pub rot_acc: i64,
    pub prev_dir: i8,
    pub fire_acc: i64,
    pub prev_a: bool,
    pub cam: (i32, i32),
}

#[derive(Default, Clone)]
pub struct Log {
    pub shots: Vec<u64>,
    pub kicks: Vec<u64>,
    pub turns: Vec<u64>,
}

pub fn new_pod() -> Pod {
    let mut s = S::default();
    s.x = 1000i64 << 24;
    s.y = 1000i64 << 24;
    Pod { gt: 0, s, facing: 0, rot_acc: 0, prev_dir: 0, fire_acc: 0, prev_a: false, cam: (1000 - 128, 1000 - 80) }
}

pub fn thrust_for(facing: u8) -> Thrust {
    let f = usize::from(facing);
    let (tv, th) = (THRUST[f & 15], THRUST[(f + 8) & 15]);
    Thrust { av: accel(tv, false, false), ah: accel(th, false, false), down: (9..=23).contains(&f), left: (17..=31).contains(&f) }
}

/// For each event of `a`, the signed difference (ms) to the nearest event of `b`.
fn nearest(a: &[u64], b: &[u64]) -> Vec<f64> {
    a.iter()
        .filter_map(|&x| b.iter().map(|&y| y as f64 - x as f64).min_by(|p, q| p.abs().partial_cmp(&q.abs()).unwrap()))
        .map(|d| d / 4294967296.0 * 1000.0 / 30.0)
        .collect()
}

pub fn opts() -> Opts {
    Opts { cap_bias: true, ..Opts::exact(Order::BeforeNow) }
}

fn camera_f(p: &mut Pod, prev_whole: (i32, i32), r: u32) {
    let whole = ((p.s.x >> 24) as i32, (p.s.y >> 24) as i32);
    let d = (whole.0 - prev_whole.0, whole.1 - prev_whole.1);
    let cap = ((240 + r as i32 - 1) / r as i32).max(1);
    for (axis, dd) in [d.0, d.1].into_iter().enumerate() {
        let h1 = dd / 2;
        let h2 = dd - h1;
        for h in [h1, h2] {
            let h = h.clamp(-cap, cap);
            let (pod, cam) = if axis == 0 { (whole.0, &mut p.cam.0) } else { (whole.1, &mut p.cam.1) };
            let s = pod - *cam;
            let (hi, lo) = if axis == 0 { (144, 112) } else { (96, 64) };
            if (h > 0 && s >= hi) || (h < 0 && s < lo) {
                *cam += h;
            }
        }
    }
}

/// One tick at rate `r`: the clock advances by f32q(r), the physics by the clock-derived Q16 fraction.
pub fn pod_tick(p: &mut Pod, r: u32, script: &Script, log: &mut Log, with_log: bool) {
    let f32 = f32q(r);
    let gt1 = p.gt + f32;
    let f = ((gt1 >> 16) - (p.gt >> 16)) as i64;
    let held = script.at(p.gt >> 16);
    let t_end = gt1;
    // rotation (results-input): Left wins, new press seeds f - 32768, then once per 65536 accumulated
    let dir: i8 = if held & Buttons::LEFT != 0 { -1 } else if held & Buttons::RIGHT != 0 { 1 } else { 0 };
    if dir != 0 && p.prev_dir == 0 {
        p.facing = (p.facing as i32 + i32::from(dir)).rem_euclid(32) as u8;
        p.rot_acc = f - 32768;
        if with_log {
            log.turns.push(t_end);
        }
    } else if dir != 0 {
        p.rot_acc += f;
        if p.rot_acc >= 65536 {
            p.facing = (p.facing as i32 + i32::from(dir)).rem_euclid(32) as u8;
            p.rot_acc -= 65536;
            if with_log {
                log.turns.push(t_end);
            }
        }
    }
    p.prev_dir = dir;
    // autofire, N = 4
    let a = held & Buttons::A != 0;
    if a {
        if !p.prev_a {
            p.fire_acc = f - 32768;
            if with_log {
                log.shots.push(t_end);
            }
        } else {
            p.fire_acc += f;
            if p.fire_acc >= 4 * 65536 {
                p.fire_acc -= 4 * 65536;
                if with_log {
                    log.shots.push(t_end);
                }
            }
        }
    }
    p.prev_a = a;
    // physics
    let inp = TickIn { kill: false, g_down: true, g_a: 8, thrust: if held & Buttons::B != 0 { Some((thrust_for(p.facing), 16, 0)) } else { None }, phase: 0 };
    let prev_whole = ((p.s.x >> 24) as i32, (p.s.y >> 24) as i32);
    let ev = tick(&mut p.s, f as u32, &inp, &opts());
    if ev.kick_v && with_log {
        log.kicks.push(t_end);
    }
    camera_f(p, prev_whole, r);
    p.gt = gt1;
}

/// State at sample time `t` (Q32 steps, >= p.gt): clone and a partial tick (no events, accumulators included).
fn peek(p: &Pod, r: u32, script: &Script, t: u64) -> Pod {
    let mut c = p.clone();
    let gt1 = t;
    let f = ((gt1 >> 16) - (c.gt >> 16)) as i64;
    if f <= 0 {
        return c;
    }
    let held = script.at(c.gt >> 16);
    let inp = TickIn { kill: false, g_down: true, g_a: 8, thrust: if held & Buttons::B != 0 { Some((thrust_for(c.facing), 16, 0)) } else { None }, phase: 0 };
    let prev_whole = ((c.s.x >> 24) as i32, (c.s.y >> 24) as i32);
    let _ = tick(&mut c.s, f as u32, &inp, &opts());
    camera_f(&mut c, prev_whole, r);
    c.gt = gt1;
    c
}

#[derive(Clone, Copy, Debug)]
pub struct Samp {
    pub x: i64,
    pub y: i64,
    pub facing: u8,
    pub cam: (i32, i32),
}

/// Runs from `p` until `end_steps` whole steps; returns samples at each whole step after p.gt.
pub fn run_pod(mut p: Pod, r: u32, script: &Script, end_step: u64, log: &mut Log) -> (Vec<(u64, Samp)>, Pod) {
    let mut out = Vec::new();
    let mut next = (p.gt >> 32) + 1;
    while next <= end_step {
        let target = next << 32;
        while p.gt + f32q(r) <= target {
            pod_tick(&mut p, r, script, log, true);
        }
        let c = peek(&p, r, script, target);
        out.push((next, Samp { x: c.s.x, y: c.s.y, facing: c.facing, cam: c.cam }));
        next += 1;
    }
    (out, p)
}

pub fn report_pod(rot: bool) {
    let save_step = 60u64; // seconds-ish: 2 s
    let save_gt = (save_step << 32) + (0.37 * 4294967296.0) as u64; // mid-step
    let horizon = save_step + 1 + 300; // 10 s later
    println!("=== mini pod [rotation inputs: {rot}] (free flight, rotation accumulator, autofire accumulator, kick, camera rule F): save at 144 Hz at t = {:.4} s (mid-step), compare the next 10 s", save_gt as f64 / 4294967296.0 / 30.0);
    let rates = [144u32, 60, 75, 100, 120, 137, 165, 200];
    let mut agg: std::collections::BTreeMap<u32, (Vec<f64>, Vec<f64>, usize, usize, Vec<f64>, Vec<f64>)> = Default::default();
    let nscripts = 24;
    for seed in 1..=nscripts {
        let script = Script::make(seed, horizon + 5, rot);
        // reference: uninterrupted 144 Hz
        let mut lref = Log::default();
        let (sref, _) = run_pod(new_pod(), 144, &script, horizon, &mut lref);
        // the save: run to the first tick end >= save_gt at 144 Hz
        let mut p = new_pod();
        let mut dummy = Log::default();
        while p.gt < save_gt {
            pod_tick(&mut p, 144, &script, &mut dummy, true);
        }
        let saved = p.clone(); // rate-free: gt (Q32 steps) + values; nothing else
        let t_save = saved.gt;
        for &r in &rates {
            let mut l2 = Log::default();
            let (sl, _) = run_pod(saved.clone(), r, &script, horizon, &mut l2);
            let e = agg.entry(r).or_default();
            let sl: std::collections::HashMap<u64, Samp> = sl.into_iter().collect();
            for (k, a) in sref.iter() {
                if *k <= (t_save >> 32) + 1 {
                    continue;
                }
                let b = &sl[k];
                e.0.push(((a.x - b.x) as f64).hypot((a.y - b.y) as f64) / 16777216.0);
                e.1.push((i32::from(a.facing) - i32::from(b.facing)).rem_euclid(32).min((i32::from(b.facing) - i32::from(a.facing)).rem_euclid(32)) as f64);
                let cd = (f64::from(a.cam.0 - b.cam.0)).hypot(f64::from(a.cam.1 - b.cam.1));
                e.4.push(cd);
            }
            // events after the save
            let ev_cmp = |a: &Vec<u64>, b: &Vec<u64>, e: &mut (Vec<f64>, Vec<f64>, usize, usize, Vec<f64>, Vec<f64>)| {
                let a: Vec<u64> = a.iter().cloned().filter(|&t| t > t_save && t < (horizon << 32)).collect();
                let b: Vec<u64> = b.iter().cloned().filter(|&t| t > t_save && t < (horizon << 32)).collect();
                e.2 += a.len().max(b.len()) - a.len().min(b.len());
                e.3 += a.len();
                e.5.extend(nearest(&a, &b));
            };
            ev_cmp(&lref.shots, &l2.shots, e);
        }
        // the uninterrupted baselines at the other rates, from the start
        for &r in &rates[1..] {
            let mut lb = Log::default();
            let (sb, _) = run_pod(new_pod(), r, &script, horizon, &mut lb);
            let e = agg.entry(1000 + r).or_default();
            let sb: std::collections::HashMap<u64, Samp> = sb.into_iter().collect();
            for (k, a) in sref.iter() {
                if *k <= (t_save >> 32) + 1 {
                    continue;
                }
                let b = &sb[k];
                e.0.push(((a.x - b.x) as f64).hypot((a.y - b.y) as f64) / 16777216.0);
                e.1.push((i32::from(a.facing) - i32::from(b.facing)).rem_euclid(32).min((i32::from(b.facing) - i32::from(a.facing)).rem_euclid(32)) as f64);
                e.4.push((f64::from(a.cam.0 - b.cam.0)).hypot(f64::from(a.cam.1 - b.cam.1)));
            }
            let a: Vec<u64> = lref.shots.iter().cloned().filter(|&t| t > t_save && t < (horizon << 32)).collect();
            let b: Vec<u64> = lb.shots.iter().cloned().filter(|&t| t > t_save && t < (horizon << 32)).collect();
            e.2 += a.len().max(b.len()) - a.len().min(b.len());
            e.3 += a.len();
            e.5.extend(nearest(&a, &b));
        }
    }
    println!("{nscripts} input scripts, error vs the uninterrupted 144 Hz run at the 30 Hz samples of the 10 s after the save:");
    println!("  columns: pod position error px rms / max | facing error (units) max | camera error px rms / max | shots: count diff / total, nearest-event time diff ms mean / p99 / max (|.|)");
    for &r in &rates {
        for (label, key) in [("load", r), ("uninterrupted-from-start", 1000 + r)] {
            if key == r && r == 144 {
                // bit-exact check
            }
            if key == 1000 + r && r == 144 {
                continue;
            }
            let Some(e) = agg.get(&key) else { continue };
            let sh_abs: Vec<f64> = e.5.iter().map(|x| x.abs()).collect();
            println!("  R={r:3} {label:25}: pos {:7.4} / {:7.3} | facing {:.0} | cam {:.3} / {:.1} | shots {}/{} dt {:.2} / {:.1} / {:.1}", rms(&e.0), absmax(&e.0), absmax(&e.1), rms(&e.4), e.4.iter().cloned().fold(0.0, f64::max), e.2, e.3, mean(&sh_abs), pct(&sh_abs, 0.99), sh_abs.iter().cloned().fold(0.0, f64::max));
        }
    }
}

// ---------------- the contact model: hop on a floor ----------------
pub fn report_contact(world: &World) {
    let scens: Vec<Scen> = scenarios(world).into_iter().filter(|s| s.class == "F").collect();
    let rules: Rules = rules_from("corr+tp1+appr+acd+push+wfull+fxcd");
    let save_gt = (60u64 << 32) + (0.37 * 4294967296.0) as u64;
    let horizon = 60 + 1 + 300;
    println!("=== contact model (hop on a flat floor, constant thrust into the floor): save at 144 Hz mid-step at 2.012 s, 10 s after");
    let rates = [144u32, 60, 75, 100, 120, 137, 165, 200];
    let mut agg: std::collections::BTreeMap<u32, (Vec<f64>, Vec<f64>, usize, usize)> = Default::default();
    let mut nsc = 0;
    for (si, s) in scens.iter().enumerate() {
        if si % 2 != 0 {
            continue;
        }
        nsc += 1;
        let mut init = s.init;
        init.vs = 0x80;
        init.vd = true;
        let sim = Sim { world, level: &world.levels[s.level], rules };
        let thr = if si % 4 == 0 { (0i64, 0i64) } else { (0, 6) }; // 8.8 px/step^2 down: pressed or not (thrust acts along +y here)
        let start = |m: &mut M| {
            m.thr = thr;
        };
        let run = |sim: &Sim, mut m: M, mut gt: u64, r: u32, end_step: u64| -> (Vec<(u64, f64, f64)>, M, u64) {
            let mut out = Vec::new();
            let mut next = (gt >> 32) + 1;
            while next <= end_step {
                let target = next << 32;
                while gt + f32q(r) <= target {
                    let gt1 = gt + f32q(r);
                    let f = ((gt1 >> 16) - (gt >> 16)) as i64;
                    sim.tick(&mut m, f);
                    gt = gt1;
                }
                let mut c = m.clone();
                let f = ((target >> 16) - (gt >> 16)) as i64;
                if f > 0 {
                    sim.tick(&mut c, f);
                }
                out.push((next, c.x as f64 / 65536.0, c.y as f64 / 65536.0));
                next += 1;
            }
            (out, m, gt)
        };
        let mut m0 = M::from_init(init);
        start(&mut m0);
        let (sref, mref, _) = run(&sim, m0.clone(), 0, 144, horizon);
        // save at the first 144 Hz tick end >= save_gt
        let mut gt = 0u64;
        let mut m = m0.clone();
        while gt < save_gt {
            let gt1 = gt + f32q(144);
            let f = ((gt1 >> 16) - (gt >> 16)) as i64;
            sim.tick(&mut m, f);
            gt = gt1;
        }
        let (saved_m, saved_gt) = (m.clone(), gt);
        let ev_ref: Vec<(f64, bool)> = mref.ev.iter().filter(|e| (e.t as u64) << 16 > saved_gt).map(|e| (e.t as f64 / 65536.0, e.dmg)).collect();
        for &r in &rates {
            let (sl, ml, _) = run(&sim, saved_m.clone(), saved_gt, r, horizon);
            let e = agg.entry(r).or_default();
            let slm: std::collections::HashMap<u64, (f64, f64)> = sl.iter().map(|x| (x.0, (x.1, x.2))).collect();
            for (k, ax, ay) in sref.iter() {
                if *k <= (saved_gt >> 32) + 1 {
                    continue;
                }
                let (bx, by) = slm[k];
                e.0.push((ax - bx).hypot(ay - by));
            }
            let ev: Vec<(f64, bool)> = ml.ev.iter().filter(|x| (x.t as u64) << 16 > saved_gt).map(|x| (x.t as f64 / 65536.0, x.dmg)).collect();
            e.2 += ev.len().max(ev_ref.len()) - ev.len().min(ev_ref.len());
            e.3 += ev_ref.len();
            for (a, b) in ev_ref.iter().zip(&ev) {
                e.1.push((b.0 - a.0) * 1000.0 / 30.0);
            }
            // the uninterrupted baseline at this rate
            if r != 144 {
                let (sb, mb, _) = run(&sim, m0.clone(), 0, r, horizon);
                let e = agg.entry(1000 + r).or_default();
                let sbm: std::collections::HashMap<u64, (f64, f64)> = sb.iter().map(|x| (x.0, (x.1, x.2))).collect();
                for (k, ax, ay) in sref.iter() {
                    if *k <= (saved_gt >> 32) + 1 {
                        continue;
                    }
                    let (bx, by) = sbm[k];
                    e.0.push((ax - bx).hypot(ay - by));
                }
                let ev: Vec<(f64, bool)> = mb.ev.iter().filter(|x| (x.t as u64) << 16 > saved_gt).map(|x| (x.t as f64 / 65536.0, x.dmg)).collect();
                e.2 += ev.len().max(ev_ref.len()) - ev.len().min(ev_ref.len());
                e.3 += ev_ref.len();
                for (a, b) in ev_ref.iter().zip(&ev) {
                    e.1.push((b.0 - a.0) * 1000.0 / 30.0);
                }
            }
        }
        let _ = Init::default;
    }
    println!("{nsc} floor sites; error vs the uninterrupted 144 Hz run, 30 Hz samples of the 10 s after the save:");
    println!("  columns: position error px rms / max (hops are chaotic: expect the same size as the rate-to-rate baseline) | contact events: count diff / total, time diff ms |mean| / max");
    for &r in &rates {
        for (label, key) in [("load", r), ("uninterrupted-from-start", 1000 + r)] {
            if key == 1000 + 144 {
                continue;
            }
            let Some(e) = agg.get(&key) else { continue };
            let ab: Vec<f64> = e.1.iter().map(|x| x.abs()).collect();
            println!("  R={r:3} {label:25}: pos {:7.4} / {:7.3} | events {}/{} dt {:.2} / {:.1}", rms(&e.0), absmax(&e.0), e.2, e.3, mean(&ab), ab.iter().cloned().fold(0.0, f64::max));
        }
    }
}
