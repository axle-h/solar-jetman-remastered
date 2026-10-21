//! Bang-bang homing controllers: the original step and the scaled candidates. Units: positions Q24 px,
//! speeds Q24 px/step (so the original's 1/256 unit = 65536), f = Q16 fraction of a step.
use solar_jetman::game::gameplay::objects::Object;

pub const U: i64 = 65536; // original 1/256 px in Q24
pub const CAP3: i64 = 3 * 256 * U; // 3.00 px/step

#[derive(Clone, Copy, Debug, Default)]
pub struct Ax { pub pos: i64, pub mag: i64, pub neg: bool }
impl Ax {
    pub fn whole(&self) -> i32 { (self.pos >> 24) as i32 }
    pub fn vel(&self) -> i64 { if self.neg { -self.mag } else { self.mag } }
}
#[derive(Clone, Copy, Debug)]
pub struct Par { pub a: i64, pub max: i64 } // original units (1/256 px/step^2, 1/256 px/step)

/// The original `home_x`/`home_y` on an axis: decision from whole-pixel difference, speed change, move.
pub fn orig_axis(ax: &mut Ax, target: i32, p: Par) {
    let diff = i32::from((target - ax.whole()) as i16);
    let toward = if diff >= 0 { !ax.neg } else { ax.neg };
    let a = p.a * U;
    if toward {
        ax.mag += a;
        if ax.mag >> 24 >= 3 { ax.mag = CAP3; }
    } else {
        ax.mag = (ax.mag - a).max(0);
        if ax.mag == 0 { ax.neg = !ax.neg; }
    }
    if ax.mag >= p.max * U { ax.mag = p.max * U; }
    ax.pos += if ax.neg { -ax.mag } else { ax.mag };
}
/// Velocity update only (the grid-decision candidate): same as orig_axis without the move.
pub fn orig_vel(ax: &mut Ax, target: i32, p: Par) {
    let keep = ax.pos; orig_axis(ax, target, p); ax.pos = keep;
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Turn { Drop, Reflect }
/// Per-tick scaled step of fraction f (Q16): accel a*f, S-motion move.
pub fn tick_axis(ax: &mut Ax, target: i32, p: Par, f: i64, turn: Turn) {
    let diff = i32::from((target - ax.whole()) as i16);
    let toward = if diff >= 0 { !ax.neg } else { ax.neg };
    let a = p.a * U * f >> 16;
    let v0 = ax.vel();
    if toward {
        ax.mag += a;
        if ax.mag >> 24 >= 3 { ax.mag = CAP3; }
    } else {
        let m = ax.mag - a;
        if m > 0 { ax.mag = m; }
        else { ax.neg = !ax.neg; ax.mag = if turn == Turn::Reflect { -m } else { 0 }; }
    }
    if ax.mag >= p.max * U { ax.mag = p.max * U; }
    let v1 = ax.vel();
    ax.pos += (f * v1 >> 16) + ((v1 - v0) * (65536 - f) >> 17);
}
/// Per-tick step with the target side latched at the grid (side_pos: target is at or beyond the object in +direction).
pub fn latch_axis(ax: &mut Ax, side_pos: bool, p: Par, f: i64) {
    let toward = if side_pos { !ax.neg } else { ax.neg };
    let a = p.a * U * f >> 16;
    let v0 = ax.vel();
    if toward { ax.mag += a; if ax.mag >> 24 >= 3 { ax.mag = CAP3; } }
    else { let m = ax.mag - a; if m > 0 { ax.mag = m; } else { ax.neg = !ax.neg; ax.mag = 0; } }
    if ax.mag >= p.max * U { ax.mag = p.max * U; }
    let v1 = ax.vel();
    ax.pos += (f * v1 >> 16) + ((v1 - v0) * (65536 - f) >> 17);
}
/// Free move at the current speed for fraction f (grid-decision candidate).
pub fn coast_axis(ax: &mut Ax, f: i64) { ax.pos += f * ax.vel() >> 16; }

pub fn ax_x(o: &Object) -> Ax { Ax { pos: (i64::from(o.x) << 24) | (i64::from(o.x_fraction) << 16), mag: i64::from(o.speed_x) * U, neg: o.left() } }
pub fn ax_y(o: &Object) -> Ax { Ax { pos: (i64::from(o.y) << 24) | (i64::from(o.y_fraction) << 16), mag: i64::from(o.speed_y) * U, neg: !o.down() } }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Var { Drop, Reflect, GridStale, GridPred, LatchStale, LatchPred, LatchOracle, GridOracle }

/// One recorded stretch of a homing object: the object at step 0, pod positions after each step, truth after each step.
pub struct Ep {
    pub obj0: Object,
    pub pod: Vec<(f64, f64)>,      // len W+1: pod at game time 0..=W (px)
    pub truth: Vec<Object>,        // len W+1
    pub px: Vec<Par>, pub py: Vec<Par>, // params for step k=1..=W (index k-1)
}

fn podat(pod: &[(f64, f64)], t: f64) -> (f64, f64) {
    let n = pod.len() - 1;
    let t = t.clamp(0.0, n as f64);
    let i = (t.floor() as usize).min(n - 1);
    let u = t - i as f64;
    (pod[i].0 + (pod[i + 1].0 - pod[i].0) * u, pod[i].1 + (pod[i + 1].1 - pod[i].1) * u)
}

/// Runs the variant at rate R with first-tick phase `phase` (0..1 of a tick); returns (x,y) in px at grid times 0..=W.
thread_local! { pub static TICKS: std::cell::RefCell<Vec<(f64, f64, f64)>> = const { std::cell::RefCell::new(Vec::new()) }; }
pub fn run_ep(ep: &Ep, rate: i64, phase: f64, var: Var) -> Vec<(f64, f64)> {
    let w = ep.truth.len() - 1;
    let mut x = ax_x(&ep.obj0);
    let mut y = ax_y(&ep.obj0);
    let q = |a: &Ax| a.pos as f64 / 16777216.0;
    let mut out = vec![(q(&x), q(&y))];
    let fq = (65536 * 30 + rate / 2) / rate; // Q16
    let ft = fq as f64 / 65536.0;
    let mut t = 0.0f64; // game time at tick start
    let mut tq: i64 = 0;
    let mut next_k = 1usize; // next grid sample to record
    let mut ev_k = 0usize;   // grid event (decision) count done: step ev_k+1 pending
    let mut first = true;
    let mut side = (true, true);
    let (mut px_prev, mut py_prev) = (q(&x), q(&y));
    let mut tprev = 0.0;
    while next_k <= w {
        let (this_f, this_ft) = if first { let ff = ((fq as f64 * phase) as i64).max(1); (ff, ff as f64 / 65536.0) } else { (fq, ft) };
        first = false;
        let t1 = t + this_ft;
        let stepk = (t.floor() as usize + 1).min(w); // the step this tick lies in (1-based)
        let (tx, ty) = podat(&ep.pod, t1);
        let (txw, tyw) = (tx.floor() as i32, ty.floor() as i32);
        match var {
            Var::LatchStale | Var::LatchPred | Var::LatchOracle => {
                while ev_k < w && (ev_k as f64) <= t + 1e-9 {
                    let (cx, cy) = podat(&ep.pod, ev_k as f64);
                    let (mut ax, mut ay) = (cx, cy);
                    if var == Var::LatchPred && ev_k >= 1 { let (bx, by) = podat(&ep.pod, ev_k as f64 - 1.0); ax = cx + (cx - bx); ay = cy + (cy - by); }
                    if var == Var::LatchOracle { let (ox, oy) = podat(&ep.pod, ev_k as f64 + 1.0); ax = ox; ay = oy; }
                    side = (i32::from((ax.floor() as i32 - x.whole()) as i16) >= 0, i32::from((ay.floor() as i32 - y.whole()) as i16) >= 0);
                    ev_k += 1;
                }
                latch_axis(&mut x, side.0, ep.px[stepk - 1], this_f);
                latch_axis(&mut y, side.1, ep.py[stepk - 1], this_f);
            }
            Var::Drop | Var::Reflect => {
                let turn = if var == Var::Drop { Turn::Drop } else { Turn::Reflect };
                tick_axis(&mut x, txw, ep.px[stepk - 1], this_f, turn);
                tick_axis(&mut y, tyw, ep.py[stepk - 1], this_f, turn);
            }
            Var::GridStale | Var::GridPred | Var::GridOracle => {
                // fire the decisions for every step whose start (k-1) is at or before this tick's start
                while ev_k < w && (ev_k as f64) <= t + 1e-9 {
                    let (cx, cy) = podat(&ep.pod, ev_k as f64);
                    let (mut ax, mut ay) = (cx, cy);
                    if var == Var::GridOracle { let (ox, oy) = podat(&ep.pod, ev_k as f64 + 1.0); ax = ox; ay = oy; }
                    if var == Var::GridPred && ev_k >= 1 { let (bx, by) = podat(&ep.pod, ev_k as f64 - 1.0); ax = cx + (cx - bx); ay = cy + (cy - by); }
                    orig_vel(&mut x, ax.floor() as i32, ep.px[ev_k]);
                    orig_vel(&mut y, ay.floor() as i32, ep.py[ev_k]);
                    ev_k += 1;
                }
                // the velocity of step ev_k holds from game time ev_k-1: events fired above are for steps starting at or before t;
                // split the tick if the next event lies strictly inside it
                let nb = ev_k as f64; // next step start
                if nb < t1 - 1e-9 && ev_k < w {
                    let f1 = ((nb - t) * 65536.0).round() as i64;
                    coast_axis(&mut x, f1); coast_axis(&mut y, f1);
                    // fire the event now (decision at its exact time)
                    let (cx, cy) = podat(&ep.pod, nb);
                    let (mut ax, mut ay) = (cx, cy);
                    if var == Var::GridOracle { let (ox, oy) = podat(&ep.pod, nb + 1.0); ax = ox; ay = oy; }
                    if var == Var::GridPred && ev_k >= 1 { let (bx, by) = podat(&ep.pod, nb - 1.0); ax = cx + (cx - bx); ay = cy + (cy - by); }
                    orig_vel(&mut x, ax.floor() as i32, ep.px[ev_k]); orig_vel(&mut y, ay.floor() as i32, ep.py[ev_k]);
                    ev_k += 1;
                    coast_axis(&mut x, this_f - f1); coast_axis(&mut y, this_f - f1);
                } else {
                coast_axis(&mut x, this_f);
                coast_axis(&mut y, this_f); }
            }
        }
        tq += this_f;
        let _ = tq;
        t = t1;
        let (cx, cy) = (q(&x), q(&y));
        TICKS.with(|v| v.borrow_mut().push((t, cx, cy)));
        while next_k <= w && (next_k as f64) <= t + 1e-9 {
            let u = (next_k as f64 - tprev) / (t - tprev);
            out.push((px_prev + (cx - px_prev) * u, py_prev + (cy - py_prev) * u));
            next_k += 1;
        }
        px_prev = cx; py_prev = cy; tprev = t;
    }
    out
}
