//! Area 2: the final-level ship (state 6): flight, carry along the route, screen limits.
use super::common::*;
use crate::motion::integ::{ONE, Ticker};
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug, Default)]
pub struct Step {
    pub held: u8,
    pub x: u16,
    pub xf: u8,
    pub y: u16,
    pub yf: u8,
    pub h: u16,
    pub hl: bool,
    pub v: u16,
    pub vd: bool,
    pub h_byte: u8,
    pub cam_used: (u16, u16), // camera at the end of the previous step: what the clamp saw
    pub cam_end: (u16, u16),  // camera after the step's B pass
    pub cam_a: (u16, u16),    // after the A pass
    pub state: u8,
    pub end: bool,
    pub my_push: i32,
    pub surface: u8,
    pub fuel: u16,
}

pub struct Rec {
    pub cam0: (u16, u16),
    pub steps: Vec<Step>,
}

/// Runs a level-26 replay (optionally continuing with `extra` random-input steps with the tank kept full).
pub fn record(world: &World, name: &str, extra: usize, seed: u64) -> Rec {
    let (mut g, r) = replay_game(world, name);
    let mut rec = Rec { cam0: (g.camera.position.x, g.camera.position.y), steps: Vec::new() };
    let mut rng = Xs(seed | 1);
    let mut cur = 0u8;
    let total = r.steps.len() + extra;
    for i in 0..total {
        let b = if i < r.steps.len() {
            setups(&mut g, &r, i + 1);
            r.steps[i]
        } else {
            if i % 9 == 0 {
                cur = 0;
                for bit in [Buttons::LEFT, Buttons::RIGHT, Buttons::UP, Buttons::DOWN] {
                    if rng.below(100) < 35 {
                        cur |= bit;
                    }
                }
                if rng.below(100) < 20 {
                    cur |= Buttons::A;
                }
            }
            g.pod.fuel = 0xE000;
            Buttons(cur)
        };
        if g.transition().is_some() {
            break;
        }
        let mut cam_a = (0, 0);
        let mut ok = true;
        for pass in 0..2 {
            if g.paused() {
                ok = false;
                break;
            }
            g.pass(world, b);
            if pass == 0 {
                cam_a = (g.camera.position.x, g.camera.position.y);
            }
        }
        if !ok {
            continue;
        }
        let p = &g.pod;
        rec.steps.push(Step {
            held: b.0,
            x: p.body.x,
            xf: p.body.x_fraction,
            y: p.body.y,
            yf: p.body.y_fraction,
            h: p.body.h_speed,
            hl: p.body.h_left,
            v: p.body.v_speed,
            vd: p.body.v_down,
            h_byte: p.h_byte,
            cam_used: (p.route_camera.x, p.route_camera.y),
            cam_end: (g.camera.position.x, g.camera.position.y),
            cam_a,
            state: p.state,
            end: p.route_end,
            my_push: p.route_my,
            surface: p.surface,
            fuel: p.fuel,
        });
    }
    rec
}

pub fn summary(world: &World) {
    let rec = record(world, "level26", 0, 1);
    let st = &rec.steps;
    let mut hist = std::collections::BTreeMap::new();
    for s in st {
        *hist.entry(s.state).or_insert(0usize) += 1;
    }
    println!("level26 replay: {} steps, states {:?}, route_end steps {}", st.len(), hist, st.iter().filter(|s| s.end).count());
    let on_x = st.iter().filter(|s| i32::from(s.x) - i32::from(s.cam_used.0) == 24 || i32::from(s.x) - i32::from(s.cam_used.0) == 232).count();
    println!("pod on the x clamp edge in {on_x} steps; camera used == previous step's end camera: {}", st.windows(2).filter(|w| w[1].cam_used == w[0].cam_end).count());
    let (runs, ok, bad) = clean_runs(&rec);
    println!("f=1 model reproduces {ok} steps exactly, {bad} not; {} clean runs, longest {}", runs.len(), runs.iter().map(|r| r.1).max().unwrap_or(0));
}

// ---------------- the model ----------------
const U: i64 = 0x10000;

#[derive(Clone, Copy, Debug)]
pub struct Sh {
    pub x: i64, // Q.24 px
    pub y: i64,
    pub hm: i64, // Q.24 px/step
    pub hl: bool,
    pub vm: i64,
    pub vd: bool,
    /// pinned to a screen limit: (side -1 low / +1 high / 0 free, offset c from the limit in [0,1) px, Q.24)
    pub pin_x: (i8, i64),
    pub pin_y: (i8, i64),
}

pub fn from_step(s: &Step) -> Sh {
    Sh {
        x: (i64::from(s.x) << 24) | (i64::from(s.xf) << 16),
        y: (i64::from(s.y) << 24) | (i64::from(s.yf) << 16),
        hm: i64::from(s.h) << 16,
        hl: s.hl,
        vm: i64::from(s.v) << 16,
        vd: s.vd,
        pin_x: (0, 0),
        pin_y: (0, 0),
    }
}

fn sg(m: i64, neg: bool) -> i64 {
    if neg { -m } else { m }
}

/// Keys of the ship: horizontal wish (Some(left)), vertical wish (Some(down)).
fn wishes(held: u8) -> (Option<bool>, Option<bool>) {
    let h = if held & Buttons::LEFT != 0 { Some(true) } else if held & Buttons::RIGHT != 0 { Some(false) } else { None };
    let v = if held & Buttons::UP != 0 { Some(false) } else if held & Buttons::DOWN != 0 { Some(true) } else { None };
    (h, v)
}

/// Speeds and move of one tick (player.md 9.1 items 2-5), fraction f.
pub fn tick(s: &mut Sh, f: i64, held: u8) {
    let (vx0, vy0) = (sg(s.hm, s.hl), sg(s.vm, !s.vd));
    let (wh, wv) = wishes(held);
    if wh == Some(s.hl) {
        s.hm = (s.hm + 0x80 * f).min(0x500 * U);
    } else {
        s.hm = (s.hm - 0xFF * f).max(0);
        if s.hm == 0 {
            s.hl = wh.unwrap_or(false);
        }
    }
    if wv == Some(s.vd) {
        s.vm = (s.vm + 0xFF * f).min(0x300 * U);
    } else {
        s.vm = (s.vm - 0x80 * f).max(0);
        if s.vm == 0 {
            s.vd = wv.unwrap_or(false);
        }
    }
    let (vx1, vy1) = (sg(s.hm, s.hl), sg(s.vm, !s.vd));
    let k = ONE as i64 - f;
    s.x += ((vx1 * f) >> 16) + (((vx1 - vx0) * k) >> 17);
    s.y += ((vy1 * f) >> 16) + (((vy1 - vy0) * k) >> 17);
}

/// Screen limits (not at the route's end) against a camera (Q.24 px, sub-pixel allowed). The original sets the whole
/// pixel to the limit and keeps the fraction. A pod held at a limit keeps only the fractional part of what it moves (whole
/// pixels are swallowed): with the limit sliding through sub-pixel positions the pod's offset c from the limit is carried
/// as state and advanced by frac(speed) per tick (not by the tick's displacement).
pub fn clamp(s: &mut Sh, _prev: (i64, i64), f: i64, cam: (i64, i64)) {
    const M: i64 = (1 << 24) - 1;
    let vq = [sg(s.hm, s.hl), sg(s.vm, !s.vd)];
    let fr = |axis: usize| -> i64 {
        let v = vq[axis];
        ((v - (v / (1 << 24)) * (1 << 24)) * f) >> 16
    };
    for axis in 0..2 {
        let (pos, c0, lo_off, hi_off, pin) = if axis == 0 { (s.x, cam.0, 24i64 << 24, 232i64 << 24, s.pin_x) } else { (s.y, cam.1, 16i64 << 24, 176i64 << 24, s.pin_y) };
        let (lo, hi) = (c0 + lo_off, c0 + hi_off);
        let mut npin = (0i8, 0i64);
        let mut np = pos;
        if pos < lo {
            let c = if pin.0 == -1 { (pin.1 + fr(axis)) & M } else { (pos - lo) & M };
            np = lo + c;
            npin = (-1, c);
        } else if pos >= hi {
            let c = if pin.0 == 1 { (pin.1 + fr(axis)) & M } else { (pos - hi) & M };
            np = hi + c;
            npin = (1, c);
        }
        if axis == 0 {
            s.x = np;
            s.pin_x = npin;
        } else {
            s.y = np;
            s.pin_y = npin;
        }
    }
}

/// The route's carry (player.md 9.1 item 10), once per A frame; (mx, my) the route's move, held the step's keys.
pub fn carry(s: &mut Sh, held: u8, mx: i32, my: i32) {
    if held & (Buttons::UP | Buttons::DOWN) == 0 && my != 0 {
        let d = my > 0;
        if s.vd == d {
            if s.vm >> 24 < 2 {
                s.vm = 0x200 * U;
            }
        } else {
            s.vm = (s.vm - 0x20 * U).max(0);
            if s.vm == 0 {
                s.vd = d;
            }
        }
    }
    if held & (Buttons::LEFT | Buttons::RIGHT) == 0 && mx != 0 {
        let d = mx < 0;
        if s.hl == d {
            if s.hm >> 24 < 2 {
                s.hm = 0x200 * U;
            }
        } else {
            s.hm = (s.hm - 0x20 * U).max(0);
            if s.hm == 0 {
                s.hl = d;
            }
        }
    }
}

/// Camera path: half-step knots (time in half steps, position), lerp.
pub struct Cam {
    pub knots: Vec<(i64, i64)>, // index j = time j half-steps -> (x, y) integer px
}
impl Cam {
    pub fn new(rec: &Rec) -> Cam {
        let mut k = vec![(i64::from(rec.cam0.0), i64::from(rec.cam0.1))];
        for s in &rec.steps {
            k.push((i64::from(s.cam_a.0), i64::from(s.cam_a.1)));
            k.push((i64::from(s.cam_end.0), i64::from(s.cam_end.1)));
        }
        Cam { knots: k }
    }
    /// camera at time t (Q16 steps), Q.24 px
    pub fn at(&self, t: i64) -> (i64, i64) {
        let t = t.max(0);
        let half = t / (ONE as i64 / 2);
        let rem = t % (ONE as i64 / 2); // Q16 steps within the half step
        let j = (half as usize).min(self.knots.len() - 1);
        let j1 = (j + 1).min(self.knots.len() - 1);
        let (a, b) = (self.knots[j], self.knots[j1]);
        let w = rem * 2; // Q16 fraction of the half step
        let lerp = |p: i64, q: i64| (p << 24) + (((q - p) * w) << 8);
        (lerp(a.0, b.0), lerp(a.1, b.1))
    }
}

/// Clean stretches: steps the f = 1 model reproduces (no terrain contact, route not ended).
pub struct SegS {
    pub start: usize, // index of the record taken as the start state
    pub len: usize,
}

fn model_step(s: &mut Sh, held: u8, cam_used: (u16, u16), mx: i32, my: i32) {
    let prev = (s.x, s.y);
    tick(s, ONE as i64, held);
    clamp(s, prev, ONE as i64, ((i64::from(cam_used.0)) << 24, (i64::from(cam_used.1)) << 24));
    carry(s, held, mx, my);
}

fn moves(rec: &Rec, k: usize) -> (i32, i32) {
    let s = &rec.steps[k];
    (i32::from(s.cam_end.0) - i32::from(s.cam_a.0), i32::from(s.cam_end.1) - i32::from(s.cam_a.1))
}

/// Maximal runs where the f = 1 model, started from the record before, reproduces the next record exactly.
pub fn clean_runs(rec: &Rec) -> (Vec<(usize, usize)>, usize, usize) {
    let st = &rec.steps;
    let mut runs = Vec::new();
    let (mut ok_n, mut bad_n) = (0, 0);
    let mut cur: Option<usize> = None;
    for k in 1..st.len() {
        // state after step k-1 includes the carry of step k-1 (applied after its pod step). The record holds the speeds
        // after the whole step (carry included) since we sample after both passes.
        let mut s = from_step(&st[k - 1]);
        let (mx, my) = moves(rec, k);
        model_step(&mut s, st[k].held, st[k].cam_used, mx, my);
        let r = from_step(&st[k]);
        let good = st[k].surface == 0 && !st[k].end && (s.x, s.y, s.hm >> 16, s.vm >> 16, s.hl, s.vd) == (r.x, r.y, r.hm >> 16, r.vm >> 16, r.hl, r.vd);
        if good {
            ok_n += 1;
            if cur.is_none() {
                cur = Some(k - 1);
            }
        } else {
            bad_n += 1;
            if let Some(a) = cur.take() {
                runs.push((a, k - 1 - a));
            }
        }
    }
    if let Some(a) = cur.take() {
        runs.push((a, st.len() - 1 - a));
    }
    (runs, ok_n, bad_n)
}

pub fn debug(world: &World) {
    let rec = record(world, "level26", 0, 1);
    let st = &rec.steps;
    let mut n = 0;
    for k in 1..st.len() {
        let mut s = from_step(&st[k - 1]);
        let (mx, my) = moves(&rec, k);
        model_step(&mut s, st[k].held, st[k].cam_used, mx, my);
        let r = from_step(&st[k]);
        if (s.x, s.y, s.hm >> 16, s.vm >> 16, s.hl, s.vd) != (r.x, r.y, r.hm >> 16, r.vm >> 16, r.hl, r.vd) && st[k].surface == 0 {
            if n < 6 {
                println!("k {k} held {:02x} prev h {:#x}{} v {:#x}{} hbyte {} | real x {:.3} y {:.3} h {:#x}{} v {:#x}{} hbyte {} cam_used {:?} mxmy {:?} | model x {:.3} y {:.3} h {:#x}{} v {:#x}{}", st[k].held, st[k-1].h, if st[k-1].hl {"L"} else {"R"}, st[k-1].v, if st[k-1].vd {"D"} else {"U"}, st[k-1].h_byte,
                    r.x as f64 / 16777216.0, r.y as f64 / 16777216.0, r.hm >> 16, if r.hl {"L"} else {"R"}, r.vm >> 16, if r.vd {"D"} else {"U"}, st[k].h_byte, st[k].cam_used, (mx, my),
                    s.x as f64 / 16777216.0, s.y as f64 / 16777216.0, s.hm >> 16, if s.hl {"L"} else {"R"}, s.vm >> 16, if s.vd {"D"} else {"U"});
            }
            n += 1;
        }
    }
}

/// Tick-level run of a clean stretch (records a..a+len): returns the model state at each of the len sample times.
/// mode 0: ticks split at step boundaries and the carry applied exactly there; 1: free ticks, carry on the first
/// tick end at/after the boundary, input of the step containing the tick start.
pub fn sim(rec: &Rec, cam: &Cam, a: usize, len: usize, rate: u32, mode: u8) -> Vec<Sh> {
    let st = &rec.steps;
    let base = (a as i64 + 1) * ONE as i64;
    let mut s = from_step(&st[a]);
    let mut tk = Ticker::new(&[(0, rate)]);
    let mut out = Vec::new();
    let mut next_carry = 1usize; // carry event i: end of step record a+i
    while out.len() < len {
        let t0 = tk.t as i64;
        let mut end = tk.next_end() as i64;
        let i_int = (t0 / ONE as i64) as usize + 1;
        let held = st[(a + i_int).min(st.len() - 1)].held;
        let sample = (out.len() as i64 + 1) * ONE as i64;
        let mut eff = end;
        if mode == 0 {
            eff = end.min(i_int as i64 * ONE as i64);
        }
        let f = eff - t0;
        // sample inside this tick (free mode)
        if mode == 1 && end > sample && t0 < sample {
            let mut c = s;
            let prev = (c.x, c.y);
            tick(&mut c, sample - t0, held);
            clamp(&mut c, prev, sample - t0, cam.at(base + sample - ONE as i64));
            out.push(c);
        }
        let prev = (s.x, s.y);
        tick(&mut s, f, held);
        clamp(&mut s, prev, f, cam.at(base + eff - ONE as i64));
        if eff == end {
            tk.advance(end as u64);
        } else {
            tk.t = eff as u64;
            end = eff;
        }
        while next_carry <= len && end >= next_carry as i64 * ONE as i64 {
            let m = (a + next_carry).min(st.len() - 1);
            let (mx, my) = moves(rec, m);
            carry(&mut s, st[m].held, mx, my);
            next_carry += 1;
        }
        if end == sample && (mode == 0 || mode == 1) && out.len() < len && !(mode == 1 && out.len() as i64 + 1 > len as i64) {
            // exact sample on a tick end (mode 0 always; mode 1 when aligned): the value after the clamp, before this tick's carry
            // is not distinguishable in position, so record the state
            if mode == 0 || end == sample {
                out.push(s);
            }
        }
    }
    out.truncate(len);
    out
}

pub fn report(world: &World) {
    let mut recs = vec![("level26 replay".to_string(), record(world, "level26", 0, 1))];
    for seed in 1..=3u64 {
        recs.push((format!("level26 + random {seed}"), record(world, "level26", 2500, seed)));
    }
    for (name, rec) in &recs {
        let (runs, ok, bad) = clean_runs(rec);
        let tot: usize = runs.iter().map(|r| r.1).sum();
        let end_steps = rec.steps.iter().filter(|s| s.end).count();
        println!("{name}: {} steps, f=1 model exact on {ok}, not on {bad} (object pushes, terrain); {} clean runs of total {tot} steps; route ended in {end_steps} steps", rec.steps.len(), runs.len());
    }
    for mode in [0u8, 1] {
        println!("--- ship tick model, mode {mode} ({}) position error vs original at its samples, px: rms / p99 / max (x; y)", if mode == 0 { "split rule" } else { "free ticks" });
        for &r in &RATES {
            let (mut ex, mut ey) = (Vec::new(), Vec::new());
            for (_, rec) in &recs {
                let cam = Cam::new(rec);
                let (runs, _, _) = clean_runs(rec);
                for &(a, len) in &runs {
                    if len < 8 {
                        continue;
                    }
                    let out = sim(rec, &cam, a, len, r, mode);
                    for (i, s) in out.iter().enumerate() {
                        let real = from_step(&rec.steps[a + i + 1]);
                        ex.push((s.x - real.x) as f64 / 16777216.0);
                        ey.push((s.y - real.y) as f64 / 16777216.0);
                    }
                }
            }
            let ax: Vec<f64> = ex.iter().map(|x| x.abs()).collect();
            let ay: Vec<f64> = ey.iter().map(|x| x.abs()).collect();
            println!("   R={r:3} n {:5}: x {:6.3} / {:6.3} / {:6.3} ; y {:6.3} / {:6.3} / {:6.3}", ex.len(), rms(&ex), pct(&ax, 0.99), absmax(&ex), rms(&ey), pct(&ay, 0.99), absmax(&ey));
        }
    }
}

pub fn classify(world: &World) {
    let rec = record(world, "level26", 0, 1);
    let cam = Cam::new(&rec);
    let (runs, _, _) = clean_runs(&rec);
    for &r in &[60u32, 144] {
        let mut groups: std::collections::BTreeMap<&str, Vec<f64>> = Default::default();
        for &(a, len) in &runs {
            if len < 8 { continue; }
            let out = sim(&rec, &cam, a, len, r, 0);
            for (i, s) in out.iter().enumerate() {
                let st = &rec.steps[a + i + 1];
                let real = from_step(st);
                let ex = (s.x - real.x) as f64 / 16777216.0;
                let on = i32::from(st.x) - i32::from(st.cam_used.0);
                let key = if on == 24 { "x pinned low" } else if on == 232 { "x pinned high" } else if st.held & 0x0C != 0 { "x free, keys" } else { "x free, no keys" };
                groups.entry(key).or_default().push(ex);
            }
        }
        for (k, v) in &groups {
            let ab: Vec<f64> = v.iter().map(|x| x.abs()).collect();
            println!("R={r}: {k}: n {} rms {:.3} p99 {:.3} max {:.3} mean {:+.3}", v.len(), rms(v), pct(&ab, 0.99), absmax(v), mean(v));
        }
    }
}

// ---------------- after the route's end: the vertical limits push the camera ----------------

/// player.md 9.1 "After the route's end": returns the camera push (route_my) and may move y (Q.24 px). cam: integer camera (cx, cy).
pub fn keep_end_y(s: &mut Sh, cam: (u16, u16)) -> i32 {
    let (cy, y) = (i32::from(cam.1), (s.y >> 24) as i32);
    let frac = s.y & 0xFFFFFF;
    let d = y - 16 - cy;
    let mut push = 0;
    if d < 0 {
        if (256..464).contains(&cy) {
            s.y = (i64::from(cy + 16) << 24) | frac;
        } else {
            push = i32::from((((d & 0xFF) >> 1) | 0x80) as u8 as i8);
        }
    } else if d >= 160 {
        if (576..768).contains(&cy) && cy & 0xFF >= 0x40 {
            s.y = (i64::from(cy + 176) << 24) | frac;
        } else {
            let excess = d - 160;
            push = (excess & 0xFF) >> 1;
            if push == 0 {
                let ny = cy + 176 - i32::from(excess == 0);
                s.y = (i64::from(ny) << 24) | frac;
            }
        }
    }
    push
}

/// Steps in the extended runs where the route has ended: does the f = 1 model give the same y and push?
pub fn check_end(world: &World) {
    for seed in 1..=2u64 {
        let rec = record(world, "level26", 4000, seed);
        let st = &rec.steps;
        let (mut n, mut ok_y, mut ok_push, mut pushes) = (0, 0, 0, 0);
        let mut hist = std::collections::BTreeMap::new();
        for k in 1..st.len() {
            if !st[k].end || st[k - 1].end != st[k].end || st[k].surface != 0 {
                continue;
            }
            let mut s = from_step(&st[k - 1]);
            tick(&mut s, ONE as i64, st[k].held);
            // x limits as before; y by the end rule
            let prev = (s.x, s.y);
            let cu = (i64::from(st[k].cam_used.0) << 24, i64::from(st[k].cam_used.1) << 24);
            // x only
            let mut sx = s;
            clamp(&mut sx, prev, ONE as i64, (cu.0, cu.1));
            s.x = sx.x;
            let push = keep_end_y(&mut s, st[k].cam_used);
            n += 1;
            let r = from_step(&st[k]);
            if s.y == r.y && s.x == r.x {
                ok_y += 1;
            }
            if push == st[k].my_push {
                ok_push += 1;
            }
            if st[k].my_push != 0 {
                pushes += 1;
                *hist.entry(st[k].my_push).or_insert(0usize) += 1;
            }
        }
        println!("seed {seed}: route-end steps checked {n}: x,y match {ok_y}, push match {ok_push}; steps with a camera push {pushes}: values {:?}", hist);
    }
}
