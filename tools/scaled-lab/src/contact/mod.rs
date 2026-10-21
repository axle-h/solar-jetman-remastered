//! contact experiment: oracle (real Pod) helpers.
pub mod obj;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::pod::{Context, Pod};
use solar_jetman::game::gameplay::tow::Tow;
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug, Default)]
pub struct Init { pub x: u16, pub y: u16, pub xf: u8, pub yf: u8, pub hs: u16, pub hl: bool, pub vs: u16, pub vd: bool }

pub fn make_pod(world: &World, level: usize, i: Init) -> Pod {
    let mut p = Pod::at_level_start(&world.levels[level]);
    p.body.x = i.x; p.body.y = i.y; p.body.x_fraction = i.xf; p.body.y_fraction = i.yf;
    p.body.h_speed = i.hs; p.body.h_left = i.hl; p.body.v_speed = i.vs; p.body.v_down = i.vd;
    p.invulnerable = 0; p.hit_pause = 0; p.fuel = 0xE000;
    p
}

pub fn ctx<'a>(world: &'a World, level: usize, frame: u8) -> Context<'a> {
    Context { world, level: &world.levels[level], frame, original_rules: true }
}

/// One original step (counter even in the A pass).
pub fn orig_step(world: &World, level: usize, p: &mut Pod, n: u32, held: Buttons) {
    let c = ctx(world, level, ((n * 2) & 0xFF) as u8);
    let mut tow = Tow::default();
    p.step(&c, held, Buttons(0), &mut tow);
}

use solar_jetman::game::gameplay::tables::{ATAN, SIN, TRIANGLE};
use solar_jetman::game::gameplay::terrain::{Surface, Window};
use solar_jetman::game::level::Level;

pub const ONE: i64 = 65536; // one logic step as Q16

/// Candidate switches.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// band uses per-tick displacement (else |v| whole px/step: too wide at fine ticks)
    pub band_scaled: bool,
    /// 1 px wall/ceiling pushes scaled by f
    pub push_scaled: bool,
    /// rest rule: floor contact with approach speed (8.8 px/step) below this -> rest (no bounce, no effects). 0 = off
    pub rest_thr: i64,
    /// motion integration correction a*f*(1-f)/2 (matches original samples at all rates)
    pub motion_corr: bool,
    /// respond (bounce/halve/effects) only when the pod approaches the surface
    pub approach: bool,
    /// right-face wall push resolves to the face (like the left face) instead of 1 px
    pub wall_full: bool,
    /// halve (slope responses) at most once per step of game time per sustained contact
    pub halve_cd: bool,
    /// sound/sparks/damage of a contact kind at most once per logic step of game time
    pub fx_cd: bool,
    /// thrust sign in the sample-exact correction: -1 = thrust one tick late (exact), 0 = ignore, +1 = immediate
    pub thr_corr: i64,
    /// speed cap in 8.8 (original: 0x400)
    pub vcap: i64,
    /// extra sink (fraction of min(v,1px)) retained at a flat-floor snap, emulating the original's detection overshoot
    pub sink_k: i64,
    /// a response to a non-approaching touch is allowed once per step of game time per kind (instead of never)
    pub appr_cd: bool,
}
impl Rules { pub fn naive() -> Rules { Rules { band_scaled: true, push_scaled: false, rest_thr: 0, motion_corr: false, approach: false, wall_full: false, halve_cd: false, fx_cd: false, thr_corr: 0, vcap: 0x400, sink_k: 0, appr_cd: false } } }

#[derive(Clone, Copy, Debug, Default)]
pub struct Ev { pub t: i64, pub kind: u8, pub dir: u8, pub speed: i64, pub loss: i64, pub dmg: bool, pub sound: bool, pub vx: i64, pub vy: i64, pub x: i64, pub y: i64 }

/// Pod physics at tick fraction f (Q16 of a 1/30 s step). Position Q16 px, speed u24 (px/step * 2^24), signed (+ right/down).
#[derive(Clone, Debug)]
pub struct M {
    pub x: i64, pub y: i64, pub vx: i64, pub vy: i64,
    pub ang: u8, pub fuel: i64, pub hp: i64, pub t: i64,
    pub ow: i32, pub h: i32,
    pub surface: u8,
    pub ev: Vec<Ev>,
    pub in_contact: bool,
    pub cur_dir: u8,
    pub f_cur: i64,
    pub last_fx: [i64; 8],
    pub suppressed: u32,
    pub next_resp: [i64; 8],
    /// constant thrust acceleration (8.8 px/step^2, x right / y down), applied after the terrain test
    pub thr: (i64, i64),
}
const U: i64 = 16; // u24 = s88 << 16

impl M {
    pub fn from_init(i: Init) -> M {
        let sx = i.hs as i64; let sy = i.vs as i64;
        M { x: ((i.x as i64) << 16) | ((i.xf as i64) << 8), y: ((i.y as i64) << 16) | ((i.yf as i64) << 8),
            vx: (if i.hl { -sx } else { sx }) << U, vy: (if i.vd { sy } else { -sy }) << U,
            ang: 0, fuel: 0xE000, hp: 0, t: 0, ow: 6, h: 6, surface: 0, ev: vec![], in_contact: false, cur_dir: 0, f_cur: 0, last_fx: [i64::MIN / 2; 8], suppressed: 0, next_resp: [i64::MIN / 2; 8], thr: (0, 0) }
    }
    pub fn xw(&self) -> i32 { (self.x >> 16) as i32 }
    pub fn yw(&self) -> i32 { (self.y >> 16) as i32 }
    pub fn yfrac(&self) -> i32 { ((self.y >> 8) & 0xFF) as i32 }
    pub fn vx88(&self) -> i64 { self.vx >> U } // signed floor for display; use mag helper below
    pub fn mag(v: i64) -> i64 { v.abs() >> U }
    /// movement angle from 8.8 magnitudes
    pub fn movement_angle(&self) -> u8 {
        let (mut h, mut v) = (Self::mag(self.vx) as u16, Self::mag(self.vy) as u16);
        for shifts in 1..=8 { h <<= 1; v <<= 1; if (h | v) & 0x0800 != 0 || shifts == 8 { break; } }
        let (h, v) = (usize::from(h >> 8), usize::from(v >> 8));
        let (right, down) = (self.vx >= 0, self.vy >= 0);
        if v == 0 { return if right { 0x40 } else { 0xC0 }; }
        if h == 0 { return if down { 0x80 } else { 0 }; }
        let e = if h == v { 0x20 } else if h > v {
            let e = ATAN[usize::from(TRIANGLE[h]) + v] / 2; if down { 0x40 - e } else { e }
        } else { let e = ATAN[usize::from(TRIANGLE[v]) + h] / 2; if down { e } else { 0x40 - e } };
        match (right, down) {
            (true, false) => 0x40u8.wrapping_sub(e), (true, true) => 0x80u8.wrapping_sub(e),
            (false, false) => e.wrapping_add(0xC0), (false, true) => e.wrapping_add(0x80),
        }
    }
}

/// integer sine of an angle in 1/256 turn, scaled by 128
fn isin(a: u8) -> i64 { let v = i64::from(SIN[usize::from(a & 0x7F)]); if a & 0x80 != 0 { -v } else { v } }
/// velocity component along the outward normal at angle a (0 = up, 0x40 = right), x right y down
fn vnormal(a: u8, vx: i64, vy: i64) -> i64 { (isin(a) * (vx >> 8) - isin(a.wrapping_add(0x40)) * (vy >> 8)) }

fn grav(level: &Level, y: i32) -> (bool, i64) {
    if (level.number as usize) < solar_jetman::game::level::PLANETS && y < 64 {
        let value = ((65 - y) * 2) as u8; return (true, (value / 4) as i64);
    }
    let g = i16::from(level.gravity as u8 as i8);
    (g >= 0, (g.unsigned_abs() / 4) as i64)
}

/// halve an u24 magnitude keeping the original's 8.8 floor
fn halve(v: i64) -> i64 { let m = v.abs() >> U; ((m >> 1) << U) * v.signum() }

pub struct Sim<'a> { pub world: &'a World, pub level: &'a Level, pub rules: Rules }

impl<'a> Sim<'a> {
    /// One tick of f (Q16 of a step). Returns surface flags.
    pub fn tick(&self, m: &mut M, f: i64) {
        m.ang = m.movement_angle();
        m.f_cur = f;
        // gravity
        let (down, a) = grav(self.level, m.yw());
        let add = a * f; // u24
        let g = if down { add } else { -add };
        let was = m.vy;
        m.vy += g;
        // same-direction cap at 3.00
        if (was >= 0) == down && (m.vy.abs() >> U) >> 8 >= 3 && (m.vy >= 0) == down { m.vy = 0x300i64 << U; if !down { m.vy = -m.vy; } }
        // pod cap 4.00 (>= $400)
        for v in [&mut m.vx, &mut m.vy] { if (v.abs() >> U) >= self.rules.vcap { *v = self.rules.vcap * v.signum() << U; } }
        // advance
        let dx = (m.vx.abs() * f >> 24) * m.vx.signum();
        let mut dy = (m.vy.abs() * f >> 24) * m.vy.signum();
        let mut dx = dx;
        if self.rules.motion_corr && f != ONE {
            // sample-exact integration: gravity acts before the move, thrust after it (one tick late)
            let k = f * (ONE - f) / ONE / 2; // f(1-f)/2 in Q16
            let g = if down { a } else { -a };
            dy += ((g + self.rules.thr_corr * m.thr.1) << 8) * k / ONE;
            dx += (self.rules.thr_corr * m.thr.0 << 8) * k / ONE;
        }
        m.x += dx; m.y += dy;
        if m.y < (16 << 16) { m.y = (16 << 16) | (m.y & 0xFFFF); m.vy = 0; }
        let bottom_page = (self.level.height.saturating_sub(1) >> 1) as i64;
        if (m.y >> 24) > bottom_page { m.y = (bottom_page << 24) | (0xFF << 16); m.vy = 0; }
        self.terrain(m, f);
        if m.thr != (0, 0) {
            for (v, t) in [(&mut m.vx, m.thr.0), (&mut m.vy, m.thr.1)] {
                *v += t * f;
                if (v.abs() >> U) >> 8 >= 3 && t != 0 { *v = 0x300i64 * v.signum() << U; }
            }
        }
        // hit pause counts down (after the test)
        if m.hp > 0 { m.hp = (m.hp - f).max(0); }
        m.t += f;
    }

    /// Whether the touch of `kind` gets the full response (reflection, halving, effects).
    fn gate(&self, m: &mut M, kind: u8, approaching: bool) -> bool {
        if !self.rules.approach { return true; }
        let now = m.t + m.f_cur;
        let ok = approaching || (self.rules.appr_cd && now >= m.next_resp[kind as usize]);
        if ok && self.rules.appr_cd {
            let n = m.next_resp[kind as usize];
            m.next_resp[kind as usize] = if now - n >= ONE { now + ONE } else { n + ONE };
        }
        ok
    }

    fn impact(&self, m: &mut M, amount: u16, kind: u8, speed: i64) {
        if self.rules.fx_cd {
            let now = m.t + m.f_cur;
            if now - m.last_fx[kind as usize] < ONE { m.suppressed += 1; return; }
            m.last_fx[kind as usize] = now;
        }
        let mut ev = Ev { t: m.t + m.f_cur, kind, dir: m.cur_dir, speed, sound: true, vx: m.vx, vy: m.vy, x: m.x, y: m.y, ..Default::default() };
        if m.fuel > 0 && m.hp <= 0 && !self.level.is_warp_level() {
            let loss = amount as i64 * 4;
            m.fuel = (m.fuel - loss).max(0); m.hp = 48 * ONE; ev.loss = loss; ev.dmg = true;
        }
        m.ev.push(ev);
    }

    fn slope_response(&self, m: &mut M, a: u8, _ceiling: bool, kind: u8) {
        let s = m.ang;
        let signed = |value: u8| i32::from(value as i8);
        let clamp = |value: i32| value.clamp(-0x38, 0x38) as u8;
        let s_new = if a < 0x80 && s >= 0x80 { a.wrapping_add(clamp(signed(a.wrapping_sub(s).wrapping_sub(0x81)))) }
            else if s >= 0x80 && s <= a { a.wrapping_add(clamp(signed(a.wrapping_sub(s).wrapping_sub(0x80)))) }
            else { a.wrapping_sub(clamp(signed(s.wrapping_add(0x80).wrapping_sub(a)))) };
        m.ang = s_new;
        let hs = M::mag(m.vx) as u16; let vs = M::mag(m.vy) as u16;
        let sum = hs.wrapping_add(vs);
        self.impact(m, sum.wrapping_mul(2), kind, sum as i64);
        let k = u32::from(((u32::from(sum) * 16) >> 8) as u8);
        let rebuild = |sine: u8| { let value = ((u32::from(sine) * k) >> 3) as u16; if value >> 8 >= 8 { 0x0700 | (value & 0xFF) } else { value } };
        let h = rebuild(SIN[usize::from(s_new & 0x7F)]);
        let v = rebuild(SIN[usize::from(0x40u8.wrapping_sub(s_new) & 0x7F)]);
        let (hl, vd) = match s_new >> 6 { 0 => (false, false), 1 => (false, true), 2 => (true, true), _ => (true, false) };
        let small = h >> 8 == 0 && v >> 8 == 0 && (h & 0xFF) + (v & 0xFF) <= 0x80;
        let (h, v) = if !small { (h >> 1, v >> 1) } else { (h, v) };
        m.vx = (h as i64) << U; if hl { m.vx = -m.vx; }
        m.vy = (v as i64) << U; if !vd { m.vy = -m.vy; }
    }

    fn terrain(&self, m: &mut M, f: i64) {
        m.surface = 0;
        let (xw, yw) = (m.xw(), m.yw());
        let window = Window::collect(self.world, self.level, xw as u16, yw as u16);
        let ow = m.ow; let h = m.h;
        let px = (xw & 15) + 16 + ow / 2;
        let py = (yw & 15) + 20;
        // vertical displacement of this tick, whole px and fraction byte
        let disp_q16 = if self.rules.band_scaled { m.vy.abs() * f >> 24 } else { m.vy.abs() >> 16 << 8 };
        let vwhole = (disp_q16 >> 16) as i32;
        let b = py + h;
        let t = b - vwhole - 3;
        let mut bottom = b;
        let vy_in = M::mag(m.vy);
        if let Some(Surface::Flat { y, .. }) = window.lists[0].iter().copied().find(
            |s| matches!(*s, Surface::Flat { x0, width, y } if x0 <= px && px <= x0 + width && t <= y && y < b)) {
            m.y += ((y - bottom) as i64) << 16; bottom = y; m.surface |= 1;
            let down = m.vy >= 0;
            let approach = vy_in;
            if !self.gate(m, 1, m.vy > 0) {
                // separating or still: position only
            } else if self.rules.rest_thr > 0 && approach < self.rules.rest_thr && down {
                // rest: no bounce, no effect
                m.vy = 0;
            } else {
                m.vy = -m.vy; // direction reverse
                self.impact(m, (vy_in as u16).wrapping_mul(4), 1, vy_in);
                m.vy = halve(m.vy);
                if self.rules.sink_k != 0 && f != ONE {
                    // sink = k/100 * min(v,1px) in Q16 px, kept inside the same pixel row
                    let v_q16 = (vy_in << 8).min(ONE); // 8.8 -> Q16 px
                    let want = v_q16 * self.rules.sink_k / 100;
                    let room = (ONE - 1) - (m.y & 0xFFFF);
                    m.y += want.min(room).max(0);
                }
            }
            let _ = down;
        }
        let yfrac_bits = ((m.y >> 8) & 0xFF) as i32;
        let sp_frac = ((disp_q16 >> 8) & 0xFF) as i32;
        let t_slope = t + if yfrac_bits >= sp_frac { 2 } else { 1 };
        'floors: for direction in 1..=6u8 {
            for surface in &window.lists[usize::from(direction)] {
                let Surface::Floor { y_low, x0, width, .. } = *surface else { continue };
                if y_low < t_slope || px < x0 || px > x0 + width { continue; }
                let line = surface.line_height(direction, px);
                if line > b { continue; }
                m.cur_dir = direction;
                m.y += ((line - bottom) as i64) << 16;
                m.surface |= match direction { 1 => 2, 2 => 4, 3 | 5 => 8, _ => 16 };
                const CONTACT: [u8; 6] = [0x15, 0xEB, 0x20, 0xE0, 0x2A, 0xD6];
                let ca = CONTACT[usize::from(direction) - 1];
                let appr = vnormal(ca, m.vx, m.vy) < 0;
                if self.gate(m, 2, appr) { self.slope_response(m, ca, false, 2); }
                break 'floors;
            }
        }
        // walls
        let (wall_top, wall_bottom) = (py - h, py + h);
        let overlaps = |y0: i32, height: i32| y0 < wall_bottom && y0 + height >= wall_top;
        let right_face = window.lists[7].iter().find_map(|s| match *s { Surface::Wall { x, y0, height } if overlaps(y0, height) && x <= px && px < x + 8 => Some(x), _ => None });
        let left_face = window.lists[8].iter().find_map(|s| match *s { Surface::Wall { x, y0, height } if overlaps(y0, height) && x < px + ow / 2 && px + ow / 2 <= x + 8 => Some(x), _ => None });
        if right_face.is_some() || left_face.is_some() {
            let hs = M::mag(m.vx);
            let approaching = if right_face.is_some() { m.vx < 0 } else { m.vx > 0 };
            let respond = self.gate(m, 3, approaching);
            m.cur_dir = if right_face.is_some() { 7 } else { 8 };
            if let Some(x) = right_face {
                if self.rules.wall_full {
                    // blend: the original's 1 px at f = 1, the depth to the face as f -> 0
                    let d = ((x + 8 - px).max(0) as i64) << 16;
                    let w = { let a = f * f / ONE; a * a / ONE }; // (f/ONE)^4
                    m.x += w + (ONE - w) * d / ONE;
                } else { m.x += if self.rules.push_scaled && !respond { f } else { ONE }; }
                if respond { m.vx = m.vx.abs(); }
            } else if let Some(x) = left_face {
                let d = (x - (px + ow / 2)) as i64;
                m.x += d << 16;
                if respond { m.vx = -m.vx.abs(); }
            }
            m.surface |= 0x20;
            if respond {
                self.impact(m, (hs as u16).wrapping_mul(4), 3, hs);
                m.vx = halve(m.vx);
            }
        }
        // ceilings
        let top = py - h;
        let reach = top + vwhole + 6;
        let flat = window.lists[9].iter().find_map(|s| match *s { Surface::Flat { x0, width, y } if x0 <= px && px <= x0 + width && top <= y && y <= reach => Some(y), _ => None });
        if let Some(y) = flat {
            m.y += ((y + 1 - top) as i64) << 16;
            let vs = M::mag(m.vy);
            if self.gate(m, 4, m.vy < 0) {
                self.impact(m, (vs as u16).wrapping_mul(4), 4, vs);
                m.vy = halve(m.vy).abs();
            }
        } else {
            'c: for direction in 10..=15u8 {
                for surface in &window.lists[usize::from(direction)] {
                    let Surface::Ceiling { y_top, x0, width, .. } = *surface else { continue };
                    if y_top > reach || px < x0 || px > x0 + width { continue; }
                    let line = surface.line_height(direction, px);
                    if line < top { continue; }
                    m.cur_dir = direction;
                    let push: i64 = if direction % 2 == 0 { 1 } else { -1 };
                    const CONTACT: [u8; 6] = [0x60, 0xA0, 0x55, 0xAA, 0x6A, 0x95];
                    let ca = CONTACT[usize::from(direction) - 10];
                    let appr = vnormal(ca, m.vx, m.vy) < 0;
                    let respond = self.gate(m, 5, appr);
                    m.x += push * if self.rules.push_scaled && !respond { f } else { ONE };
                    m.y += ((line + 1 - top) as i64) << 16;
                    m.surface |= 0x20;
                    if respond { self.slope_response(m, ca, true, 5); }
                    break 'c;
                }
            }
        }
    }
}

pub const RATES: [i64; 10] = [30, 60, 75, 90, 100, 120, 144, 165, 200, 137];
pub fn fq(rate: i64) -> i64 { (ONE * 30 + rate / 2) / rate }

pub struct Trace { pub ev: Vec<Ev>, pub fuel_at: Vec<i64>, /* per 1/30 s sample (fuel) */ pub y_at: Vec<f64>, pub apex: Vec<f64> }

/// Run the model for `secs` of real time at rate; samples at each whole 1/30 s boundary reached.
pub fn trace(world: &World, level: usize, rules: Rules, rate: i64, init: Init, secs: f64) -> Trace {
    let sim = Sim { world, level: &world.levels[level], rules };
    let mut m = M::from_init(init);
    let f = fq(rate);
    let ticks = (secs * rate as f64) as i64;
    let mut fuel_at = vec![]; let mut y_at = vec![]; let mut next_s = ONE; // sample when t >= next
    let mut apex = vec![]; let mut min_y = f64::MAX; let mut last_ev = 0usize; let mut had = false;
    for _ in 0..ticks {
        sim.tick(&mut m, f);
        let yy = m.y as f64 / 65536.0;
        if yy < min_y { min_y = yy; }
        if m.ev.len() > last_ev { if had { apex.push(min_y); } had = true; min_y = f64::MAX; last_ev = m.ev.len(); }
        while m.t >= next_s { fuel_at.push(m.fuel); y_at.push(yy); next_s += ONE; }
    }
    Trace { ev: m.ev, fuel_at, y_at, apex }
}
pub fn ms(t: i64) -> f64 { t as f64 / 65536.0 * 1000.0 / 30.0 }

// ---------- scenarios ----------
#[derive(Clone, Debug)]
pub struct Scen { pub class: &'static str, pub level: usize, pub init: Init, pub tag: String }

/// First event (kind, dir, steps) when run at the original rate with the model.
pub fn first_event(world: &World, level: usize, init: Init, max_steps: usize) -> Option<(u8, u8, usize)> {
    let sim = Sim { world, level: &world.levels[level], rules: Rules::naive() };
    let mut m = M::from_init(init);
    for n in 0..max_steps {
        sim.tick(&mut m, ONE);
        if let Some(e) = m.ev.first() { return Some((e.kind, e.dir, n + 1)); }
        if m.surface != 0 { return None; }
    }
    None
}

pub fn find_scenarios(world: &World) -> Vec<Scen> {
    let mut out = vec![];
    for level in 0..world.levels.len() {
        let lv = &world.levels[level];
        if lv.is_warp_level() { continue; }
        let w = lv.width as u32 * 256; let hh = ((lv.height as u32 - 1) / 2 + 1) * 256;
        let planet = lv.gravity != 0;
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        let mut tryit = |out: &mut Vec<Scen>, class: &'static str, init: Init, want: &dyn Fn(u8, u8) -> Option<String>, cap: usize| {
            if let Some((k, d, n)) = first_event(world, level, init, 90) {
                if n < 6 { return; }
                if let Some(key) = want(k, d) {
                    let c = counts.entry(format!("{class}{key}")).or_default();
                    if *c < cap { *c += 1; out.push(Scen { class, level, init, tag: format!("L{level} {key} ({},{})", init.x, init.y) }); }
                }
            }
        };
        let mut x = 40u32; let mut ix = 0;
        while x < w - 40 {
            let mut y = 40u32; let mut iy = 0;
            while y < hh - 40 {
                let (xx, yy) = (x as u16, y as u16);
                if planet {
                    tryit(&mut out, "F", Init { x: xx, y: yy, vd: true, ..Default::default() }, &|k, _| if k == 1 { Some("flat".into()) } else { None }, 3);
                    tryit(&mut out, "S", Init { x: xx, y: yy, vd: true, ..Default::default() }, &|k, d| if k == 2 { Some(format!("slope{d}")) } else { None }, 1);
                    tryit(&mut out, "SC", Init { x: xx, y: yy, vd: false, vs: 0x100, ..Default::default() }, &|k, d| if k == 5 { Some(format!("cslope{d}")) } else { None }, 1);
                }
                if (ix + iy) % 2 == 0 {
                    let robust = |hl: bool, d: u8| [0x40u16, 0x200, 0x3F0].iter().all(|&hs| matches!(first_event(world, level, Init { x: xx, y: yy, hs, hl, ..Default::default() }, 120), Some((3, dd, _)) if dd == d));
                    if robust(false, 8) { tryit(&mut out, "W", Init { x: xx, y: yy, hs: 0x100, hl: false, ..Default::default() }, &|k, d| if k == 3 && d == 8 { Some("wallR".into()) } else { None }, 2); }
                    if robust(true, 7) { tryit(&mut out, "W", Init { x: xx, y: yy, hs: 0x100, hl: true, ..Default::default() }, &|k, d| if k == 3 && d == 7 { Some("wallL".into()) } else { None }, 2); }
                    tryit(&mut out, "C", Init { x: xx, y: yy, vs: 0x100, vd: false, ..Default::default() }, &|k, _| if k == 4 { Some("flatceil".into()) } else { None }, 2);
                }
                y += 19; iy += 1;
            }
            x += 23; ix += 1;
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Smp { pub x: f64, pub y: f64, pub vx: f64, pub vy: f64, pub fuel: i64 }
pub struct Tr { pub ev: Vec<Ev>, pub s: Vec<Smp>, pub apex: Vec<f64> }

/// Run and sample at every whole logic step (position extrapolated back from the first tick at/after the boundary).
pub fn run(world: &World, level: usize, rules: Rules, rate: i64, init: Init, secs: f64) -> Tr {
    let sim = Sim { world, level: &world.levels[level], rules };
    let mut m = M::from_init(init);
    let f = fq(rate);
    let nsteps = (secs * 30.0) as usize;
    let mut s = vec![Smp { x: m.x as f64 / 65536.0, y: m.y as f64 / 65536.0, vx: m.vx as f64 / 16777216.0 * 256.0, vy: m.vy as f64 / 16777216.0 * 256.0, fuel: m.fuel }];
    let mut next = ONE;
    let (mut apex, mut nev, mut miny, mut yev) = (vec![], 0usize, f64::MAX, 0.0f64);
    while s.len() <= nsteps {
        sim.tick(&mut m, f);
        let yy = m.y as f64 / 65536.0;
        if m.ev.len() > nev { if nev > 0 { apex.push(yev - miny); } nev = m.ev.len(); yev = yy; miny = yy; } else if yy < miny { miny = yy; }
        while m.t >= next {
            let back = (m.t - next) as f64 / 65536.0; // steps
            let vx = m.vx as f64 / 16777216.0; let vy = m.vy as f64 / 16777216.0; // px/step
            s.push(Smp { x: m.x as f64 / 65536.0 - vx * back, y: m.y as f64 / 65536.0 - vy * back, vx: vx * 256.0, vy: vy * 256.0, fuel: m.fuel });
            next += ONE;
        }
    }
    Tr { ev: m.ev, s, apex }
}

pub fn save_scens(path: &str, v: &[Scen]) {
    let mut t = String::new();
    for s in v { let i = s.init; t += &format!("{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", s.class, s.level, i.x, i.y, i.xf, i.yf, i.hs, i.hl as u8, i.vs, i.vd as u8, s.tag, 0); }
    std::fs::write(path, t).unwrap();
}
pub fn load_scens(path: &str) -> Option<Vec<Scen>> {
    let t = std::fs::read_to_string(path).ok()?;
    let mut out = vec![];
    for l in t.lines() {
        let f: Vec<&str> = l.split('|').collect();
        let class: &'static str = match f[0] { "F" => "F", "S" => "S", "SC" => "SC", "W" => "W", _ => "C" };
        let n = |i: usize| f[i].parse::<u32>().unwrap();
        out.push(Scen { class, level: n(1) as usize, init: Init { x: n(2) as u16, y: n(3) as u16, xf: n(4) as u8, yf: n(5) as u8, hs: n(6) as u16, hl: n(7) == 1, vs: n(8) as u16, vd: n(9) == 1 }, tag: f[10].to_string() });
    }
    Some(out)
}
pub fn scenarios(world: &World) -> Vec<Scen> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/scens.txt");
    if let Some(v) = load_scens(p) { return v; }
    let v = find_scenarios(world); save_scens(p, &v); v
}

/// Speed variants per class: (hs, vs, vd) overrides.
pub fn variants(class: &str) -> Vec<(u16, u16, bool)> {
    match class {
        "F" => [0u16, 0x80, 0x100, 0x200, 0x300, 0x3F0].iter().map(|&v| (0, v, true)).collect(),
        "S" => vec![(0, 0, true), (0, 0x100, true), (0, 0x300, true), (0x100, 0, true), (0x100, 0x200, true)],
        "SC" => vec![(0, 0x100, false), (0, 0x300, false)],
        "W" => [0x40u16, 0x100, 0x200, 0x300, 0x3F0].iter().map(|&v| (v, 0, true)).collect(),
        _ => [0x80u16, 0x100, 0x200, 0x300, 0x3F0].iter().map(|&v| (0, v, false)).collect(),
    }
}
pub fn with_variant(s: &Scen, v: (u16, u16, bool)) -> Init {
    let mut i = s.init;
    if s.class == "W" { i.hs = v.0; } else { if s.class == "S" || s.class == "SC" { i.hs = v.0.max(0); } i.vs = v.1; i.vd = v.2; }
    if s.class == "S" && v.0 == 0 { i.hs = 0; }
    i
}

pub fn rules_from(which: &str) -> Rules {
    let mut r = Rules::naive();
    r.motion_corr = which.contains("corr"); r.push_scaled = which.contains("push"); r.approach = which.contains("appr");
    r.wall_full = which.contains("wfull"); r.appr_cd = which.contains("acd"); if let Some(i) = which.find("sink") { r.sink_k = which[i + 4..].split('+').next().unwrap().parse().unwrap_or(0); } if which.contains("tm1") { r.thr_corr = -1; } if which.contains("tp1") { r.thr_corr = 1; } r.fx_cd = which.contains("fxcd"); r.halve_cd = which.contains("hcd");
    if let Some(i) = which.find("rest") { r.rest_thr = which[i + 4..].split('+').next().unwrap().parse().unwrap_or(0); }
    r
}

pub fn run_thr(world: &World, level: usize, rules: Rules, rate: i64, init: Init, thr: (i64, i64), secs: f64) -> Tr {
    let sim = Sim { world, level: &world.levels[level], rules };
    let mut m = M::from_init(init); m.thr = thr;
    let f = fq(rate);
    let nsteps = (secs * 30.0) as usize;
    let mut s = vec![Smp { x: m.x as f64 / 65536.0, y: m.y as f64 / 65536.0, vx: 0.0, vy: 0.0, fuel: m.fuel }];
    let mut next = ONE;
    while s.len() <= nsteps {
        sim.tick(&mut m, f);
        while m.t >= next {
            let back = (m.t - next) as f64 / 65536.0;
            let vx = m.vx as f64 / 16777216.0; let vy = m.vy as f64 / 16777216.0;
            s.push(Smp { x: m.x as f64 / 65536.0 - vx * back, y: m.y as f64 / 65536.0 - vy * back, vx: vx * 256.0, vy: vy * 256.0, fuel: m.fuel });
            next += ONE;
        }
    }
    Tr { ev: m.ev, s, apex: vec![] }
}

pub fn replay_full(name: &str) -> solar_jetman::tools::replay::Replay {
    let path = format!("{}/../../tools/mesen/replays/{name}.replay", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("replay");
    solar_jetman::tools::replay::parse_replay(&text).expect("parse")
}
