//! Jetman walking (state 3): per-tick model of the horizontal speed ramp and the move.
use super::common::*;
use super::foot::{R, Run};
use crate::motion::integ::{ONE, Ticker};
use solar_jetman::game::gameplay::input::Buttons;

#[derive(Clone, Copy, Debug)]
pub struct W {
    /// Q.24 px
    pub x: i64,
    /// speed magnitude Q.24 px/step
    pub hm: i64,
    pub hl: bool,
    /// brake mode continues until this game time (Q16 steps): the rest of the step in which the speed crossed zero
    pub bu: i64,
}

const GB: bool = true;
const U: i64 = 0x10000;

pub fn from_rec(r: &R) -> W {
    W { x: (i64::from(r.x) << 24) | (i64::from(r.xf) << 16), hm: i64::from(r.h) << 16, hl: r.hl, bu: 0 }
}
pub fn signed(w: &W) -> i64 {
    if w.hl { -w.hm } else { w.hm }
}

/// Speed update of one tick of fraction f for the held buttons (state 3, player.md 6.2 step 7).
pub fn upd(w: &mut W, f: i64, held: u8) {
    upd_t(w, f, held, 0, false)
}

/// `t0`: game time at the tick start (Q16 steps); `gb`: keep braking to the end of the 30 Hz step after a flip.
pub fn upd_t(w: &mut W, f: i64, held: u8, t0: i64, gb: bool) {
    let left = held & Buttons::LEFT != 0;
    let right = held & Buttons::RIGHT != 0;
    if left || right {
        if w.hl != left || (gb && t0 < w.bu) {
            let amt = 0x28 * f;
            if w.hl == left {
                // continuing the flipped step: accelerate in the held direction at the brake rate
                w.hm += amt;
            } else if w.hm > amt {
                w.hm -= amt;
            } else {
                w.hm = amt - w.hm;
                w.hl = !w.hl;
                // the rest of this 30 Hz step still brakes: until the next grid line after the crossing
                let tc = t0 + f - f * w.hm / amt.max(1);
                w.bu = (tc / 65536 + 1) * 65536;
                if !gb {
                    w.bu = 0;
                }
            }
        } else if w.hm < 0x100 * U {
            w.hm = (w.hm + 0x13 * f).min(0x100 * U);
        }
    } else {
        w.hm = (w.hm - 0x20 * f).max(0);
    }
}

pub fn tick(w: &mut W, f: i64, held: u8, corr: bool) {
    tick_t(w, f, held, corr, 0, false)
}
pub fn tick_t(w: &mut W, f: i64, held: u8, corr: bool, t0: i64, gb: bool) {
    let v0 = signed(w);
    upd_t(w, f, held, t0, gb);
    let v1 = signed(w);
    let mut dx = (v1 * f) >> 16;
    if corr {
        dx += ((v1 - v0) * (65536 - f)) >> 17;
    }
    w.x += dx;
}

/// Clean walking segments: consecutive state-3 steps reproduced by the step rule at f = 1.
pub struct Seg {
    pub start: R,
    pub steps: Vec<R>,
}

fn orig_step_ok(prev: &R, cur: &R) -> bool {
    if prev.state != 3 || cur.state != 3 || cur.y != prev.y || cur.yf != prev.yf || cur.surface & 0x1E != 0 {
        return false;
    }
    let mut w = from_rec(prev);
    let x0 = w.x;
    tick(&mut w, 65536, prev.held, false); // M: advance uses prev's speed (already updated) -> see below
    let _ = x0;
    // original: x_k = x_{k-1} + h_{k-1}; h_k = upd(h_{k-1}, held_k)
    let mut w = from_rec(prev);
    let dx = signed(&w);
    w.x += dx;
    let mut w2 = W { x: w.x, hm: w.hm, hl: w.hl, bu: 0 };
    upd(&mut w2, 65536, cur.held);
    let c = from_rec(cur);
    w2.x == c.x && w2.hm >> 16 == c.hm >> 16 && w2.hl == c.hl
}

pub fn segments(runs: &[Run], min: usize) -> (Vec<Seg>, usize, usize) {
    let mut out = Vec::new();
    let (mut tot, mut bad) = (0usize, 0usize);
    for r in runs {
        let mut cur: Option<Seg> = None;
        for w in r.recs.windows(2) {
            let (prev, c) = (&w[0], &w[1]);
            if prev.state == 3 && c.state == 3 {
                tot += 1;
            }
            if orig_step_ok(prev, c) {
                match &mut cur {
                    Some(s) => s.steps.push(*c),
                    None => cur = Some(Seg { start: *prev, steps: vec![*c] }),
                }
            } else {
                if prev.state == 3 && c.state == 3 {
                    bad += 1;
                }
                if let Some(s) = cur.take() {
                    if s.steps.len() >= min {
                        out.push(s);
                    }
                }
            }
        }
        if let Some(s) = cur.take() {
            if s.steps.len() >= min {
                out.push(s);
            }
        }
    }
    (out, tot, bad)
}

/// States at the original's sample times (position Q.24, speed). mode 0: ticks split at step
/// edges; mode 1: free ticks, each tick takes the input of the step containing its start (lag: the
/// step before it). With `lag` the model starts at sample 1 (state x_1 and speed h_0) and returns the
/// samples 2..=n (index 0 = sample 2).
pub fn sim(seg: &Seg, rate: u32, lag: bool, mode: u8, corr: bool, gb: bool) -> Vec<(i64, i64)> {
    let n = seg.steps.len();
    let base = usize::from(lag);
    let mut tk = Ticker::new(&[(0, rate)]);
    let mut s = if lag {
        let mut w = from_rec(&seg.steps[0]);
        w.hm = i64::from(seg.start.h) << 16;
        w.hl = seg.start.hl;
        w
    } else {
        from_rec(&seg.start)
    };
    let mut out = Vec::new();
    let held = |k: usize| -> u8 {
        let r = if k == 0 { &seg.start } else { &seg.steps[(k - 1).min(n - 1)] };
        r.held
    };
    let total = n - base;
    while out.len() < total {
        let t0 = tk.t;
        let end = tk.next_end();
        let tin = if mode == 2 { t0 + (end - t0) / 2 } else { t0 };
        let k_int = (tin / u64::from(ONE)) as usize + 1 + base; // real interval index (1-based)
        let inp = if lag { held(k_int - 1) } else { held(k_int) };
        let k_next = out.len() + 1;
        let sample_t = k_next as u64 * u64::from(ONE);
        if mode == 0 {
            let b = (k_int - base) as u64 * u64::from(ONE);
            let eff = end.min(b);
            tick_t(&mut s, (eff - t0) as i64, inp, corr, t0 as i64, gb);
            if eff == end { tk.advance(end) } else { tk.t = eff }
            if tk.t == sample_t {
                out.push((s.x, signed(&s)));
            }
        } else {
            if end > sample_t && t0 < sample_t {
                let mut c = s;
                tick_t(&mut c, (sample_t - t0) as i64, inp, corr, t0 as i64, gb);
                out.push((c.x, signed(&c)));
            }
            tick_t(&mut s, (end - t0) as i64, inp, corr, t0 as i64, gb);
            tk.advance(end);
            if end == sample_t {
                out.push((s.x, signed(&s)));
            }
        }
    }
    out
}

/// Event flags of a segment at f = 1: 1 brake flip, 2 stopped by friction (floor at 0), 4 reached the 1.00 clamp.
pub fn flags(seg: &Seg) -> u8 {
    let mut fl = 0;
    let mut w = from_rec(&seg.start);
    for r in &seg.steps {
        let h0 = w.hm;
        let hl0 = w.hl;
        upd(&mut w, 65536, r.held);
        if w.hl != hl0 {
            fl |= 1;
        }
        if h0 > 0 && w.hm == 0 && r.held & 0x03 == 0 {
            fl |= 2;
        }
        if h0 < 0x100 * U && w.hm == 0x100 * U && w.hl == hl0 && (h0 + 0x13 * 65536 > 0x100 * U) {
            fl |= 4;
        }
    }
    fl
}

pub fn report(runs: &[Run]) {
    let (segs, tot, bad) = segments(runs, 6);
    let steps: usize = segs.iter().map(|s| s.steps.len()).sum();
    let steps_m: usize = segs.iter().map(|s| s.steps.len() - 1).sum();
    println!("walking: {} state-3 step pairs, {} not reproduced by the step rule (terrain/slope/direction quirks, cut); {} clean segments, {} steps", tot, bad, segs.len(), steps);
    // exactness at f=1, M mapping, no correction
    let mut mism = 0;
    for s in &segs {
        let out = sim(s, 30, true, 0, true, true);
        for (a, r) in out.iter().zip(&s.steps[1..]) {
            let c = from_rec(r);
            if a.0 != c.x {
                mism += 1;
            }
        }
    }
    println!("R=30 (lag mapping): {} mismatching samples of {} (position bit-exact in Q.24, whole speed)", mism, steps_m);
    let fl: Vec<u8> = segs.iter().map(flags).collect();
    for m in [0u8, 1, 2, 4] {
        println!("   segments with flag mask {m}: {}", fl.iter().filter(|&&f| f == m).count());
    }
    for (cname, sel) in [("pure ramps (no flip, stop or clamp)", 0u8), ("flip only", 1u8), ("stop only", 2u8), ("clamp only", 4u8), ("any", 255u8)] {
        println!("--- M split rule, class {cname}");
        for &r in &RATES {
            let mut pe = Vec::new();
            for (s, &f) in segs.iter().zip(&fl) {
                if sel != 255 && f != sel {
                    continue;
                }
                let out = sim(s, r, true, 0, true, GB);
                for (a, rec) in out.iter().zip(&s.steps[1..]) {
                    pe.push((a.0 - from_rec(rec).x) as f64 / 16777216.0);
                }
            }
            let ab: Vec<f64> = pe.iter().map(|x| x.abs()).collect();
            println!("   R={r:3} n {:6} rms {:7.4} p99 {:7.4} max {:7.4}", pe.len(), rms(&pe), pct(&ab, 0.99), absmax(&pe));
        }
    }
    for (name, lag, corr, mode) in [("M split (rule)", true, true, 0u8), ("M free ticks", true, true, 1), ("M free, input at tick middle", true, true, 2), ("L (no lag) free", false, true, 1)] {
        println!("--- walking {name}: position error vs original at its samples, px (rms / p99 / max, speed rms in 1/256 px/step, steps whose whole x>>2&3 frame differs %)");
        for &r in &RATES {
            let (mut pe, mut ve, mut fr) = (Vec::new(), Vec::new(), (0usize, 0usize));
            for s in &segs {
                let out = sim(s, r, lag, mode, corr, GB);
                let off = usize::from(lag);
                for (a, rec) in out.iter().zip(&s.steps[off..]) {
                    let c = from_rec(rec);
                    pe.push((a.0 - c.x) as f64 / 16777216.0);
                    ve.push((a.1 - signed(&c)) as f64 / 65536.0);
                    fr.1 += 1;
                    if ((a.0 >> 24) >> 2) & 3 != ((c.x >> 24) >> 2) & 3 {
                        fr.0 += 1;
                    }
                }
            }
            let ab: Vec<f64> = pe.iter().map(|x| x.abs()).collect();
            println!("   R={r:3} {:7.4} / {:7.4} / {:7.4} | v {:6.3} | frame {:5.2}%", rms(&pe), pct(&ab, 0.99), absmax(&pe), rms(&ve), 100.0 * fr.0 as f64 / fr.1 as f64);
        }
    }
}
