//! Candidate integrators: state in fixed point, one `tick` per physics tick of fraction `f` (Q16).
//!
//! Speeds are kept as the original keeps them: a magnitude in 1/256 px per step (here Q.24, so the
//! sub-unit part is mid-step ramp) plus the thrust sub-fraction byte `acc` (here Q.24, < 1 unit) that
//! only ever feeds the magnitude through carries and never moves the pod itself.
use super::rec::{Rec, Thrust, thrust_of};

pub const ONE: u32 = 65536;
const UNIT: i64 = 0x10000; // 1/256 px per step in Q.24

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct S {
    /// position, Q.24 px
    pub x: i64,
    pub y: i64,
    /// speed magnitude, Q.24 px per step; sub-fraction accumulator < UNIT
    pub hm: i64,
    pub hacc: i64,
    pub hl: bool,
    pub vm: i64,
    pub vacc: i64,
    pub vd: bool,
    /// time left before another reversal kick is allowed (Q16 steps)
    pub kick_wait: i64,
}

impl S {
    pub fn from_rec(r: &Rec) -> S {
        S {
            x: (i64::from(r.x) << 24) | (i64::from(r.xf) << 16),
            y: (i64::from(r.y) << 24) | (i64::from(r.yf) << 16),
            hm: i64::from(r.h) << 16,
            hacc: i64::from(r.ah) << 8,
            hl: r.hl,
            vm: i64::from(r.v) << 16,
            vacc: i64::from(r.av) << 8,
            vd: r.vd,
            kick_wait: 0,
        }
    }
    /// signed speeds (right/down positive for h, up positive for v) including the accumulator
    pub fn signed(&self) -> (i64, i64) {
        let h = self.hm + self.hacc;
        let v = self.vm + self.vacc;
        (if self.hl { -h } else { h }, if self.vd { -v } else { v })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// thrust of the step after the move (original order), current interval's input
    After,
    /// thrust before the move, input of the previous step (matches the original's samples)
    BeforeLag,
    /// thrust before the move, current step's input (one step earlier than the original)
    BeforeNow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bump {
    /// the reversal's extra step of acceleration, full size once per crossing
    Keep,
    /// scaled by f
    Scale,
    Drop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryH {
    Actual,
    Zero,
    Mean,
}
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub order: Order,
    pub corr: bool,
    pub bump: Bump,
    pub carry_v: bool,
    pub carry_h: CarryH,
    /// decide the vertical reversal against the speed the pod had at the start of the 30 Hz interval
    pub phase_kick: bool,
    /// at most one reversal kick per 1/30 s
    pub cooldown: bool,
    /// the thrust cap sits one full step of opposing gravity below 3.00 (the original's displacement
    /// speed at the cap is the capped speed minus gravity)
    pub cap_bias: bool,
    /// fractional bits kept of the position and of the speeds after each tick (24 = full)
    pub pos_bits: u32,
    pub spd_bits: u32,
}

impl Opts {
    pub fn exact(order: Order) -> Opts {
        Opts { order, corr: true, bump: Bump::Keep, carry_v: true, carry_h: CarryH::Actual, phase_kick: false, cooldown: false, cap_bias: false, pos_bits: 24, spd_bits: 24 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TickIn {
    pub kill: bool,
    pub g_down: bool,
    pub g_a: u16,
    pub thrust: Option<(Thrust, u8, u8)>,
    /// time since the last 30 Hz grid line at the start of the tick, Q16 of a step
    pub phase: u32,
}

pub fn thrust_in(r: &Rec) -> Option<(Thrust, u8, u8)> {
    thrust_of(r).map(|t| (t, r.g_byte, r.cam))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ev {
    pub kick_v: bool,
    pub cross_h: bool,
    pub grav: bool,
    pub cap: bool,
    pub kill_sat: bool,
}

fn cap3(m: &mut i64) -> bool {
    if *m >= 3 << 24 {
        let hit = *m != 3 << 24;
        *m = 3 << 24;
        return hit;
    }
    false
}

/// Thrust along one axis. Returns (crossed zero, cap hit).
#[allow(clippy::too_many_arguments)]
fn thrust_axis(mag: &mut i64, acc: &mut i64, dir: &mut bool, want: bool, a_q8: u16, carry: u8, f: u32, vertical: bool, o: &Opts, gz: i64, wait: i64, bias: i64) -> (bool, bool) {
    let du = i64::from(a_q8) * i64::from(f);
    let dacc = (i64::from(carry) * i64::from(f)) >> 8;
    if *dir == want {
        *mag += du;
        *acc += dacc;
        if *acc >= UNIT {
            *acc -= UNIT;
            *mag += UNIT;
        }
        if bias > 0 && vertical {
            let lim = (3 << 24) - bias;
            let hit = *mag > lim;
            if hit {
                *mag = lim;
            }
            return (false, hit);
        }
        return (false, cap3(mag));
    }
    let mut borrow = 0;
    *acc -= dacc;
    if *acc < 0 {
        *acc += UNIT;
        *mag -= UNIT;
        borrow = 1;
    }
    // opposing: the speed shrinks; through zero the leftover reflects (player.md 4.6). The original's extra
    // step of acceleration (vertical only) comes with the reversal, decided against the speed the pod had
    // before this thrust (gz = gravity along the thrust since the interval began, 0 unless phase-locked).
    let mut vs = du - *mag;
    let kick = vertical && *mag + gz <= du && (!o.cooldown || wait <= 0);
    let mut hit = false;
    if kick {
        *acc = i64::from(carry) << 8;
        let bump = (i64::from(a_q8) + borrow) * UNIT;
        vs += match o.bump {
            Bump::Keep => bump,
            Bump::Scale => (bump * i64::from(f)) >> 16,
            Bump::Drop => 0,
        };
    } else if !vertical && vs >= 0 {
        *acc = 0;
    }
    if vs >= 0 {
        *mag = vs;
        *dir = want;
    } else {
        *mag = -vs;
    }
    if kick {
        hit = cap3(mag);
    }
    (kick, hit)
}

pub fn apply_thrust(s: &mut S, f: u32, t: &(Thrust, u8, u8), o: &Opts, gz: i64, bias: i64) -> Ev {
    let wait = s.kick_wait;
    let (th, cv, ch) = *t;
    let cv = if o.carry_v { cv } else { 0 };
    let ch = match o.carry_h {
        CarryH::Actual => ch,
        CarryH::Zero => 0,
        CarryH::Mean => 69,
    };
    let (kv, cap_v) = thrust_axis(&mut s.vm, &mut s.vacc, &mut s.vd, th.down, th.av, cv, f, true, o, gz, wait, bias);
    if kv {
        s.kick_wait = i64::from(ONE);
    }
    let (mut kh, mut cap_h) = (false, false);
    if th.ah != 0 {
        (kh, cap_h) = thrust_axis(&mut s.hm, &mut s.hacc, &mut s.hl, th.left, th.ah, ch, f, false, o, 0, 0, 0);
    }
    Ev { kick_v: kv, cross_h: kh, cap: cap_v || cap_h, ..Default::default() }
}

fn disp(s: &S) -> (i64, i64) {
    (if s.hl { -s.hm } else { s.hm }, if s.vd { s.vm } else { -s.vm })
}

pub fn tick(s: &mut S, f: u32, inp: &TickIn, o: &Opts) -> Ev {
    let mut ev = Ev::default();
    let (vx0, vy0) = disp(s);
    s.kick_wait -= i64::from(f);
    if o.order != Order::After {
        if let Some(t) = &inp.thrust {
            let e = apply_thrust(s, f, t, o, gz_of(s, inp, t, o), if o.cap_bias && inp.g_down != t.0.down { (i64::from(inp.g_a) * UNIT * i64::from(ONE - f)) >> 16 } else { 0 });
            ev.kick_v |= e.kick_v;
            ev.cross_h |= e.cross_h;
            ev.cap |= e.cap;
        }
    }
    if inp.kill {
        let dk = 0x20 * i64::from(f);
        for m in [&mut s.hm, &mut s.vm] {
            if *m > dk {
                *m -= dk;
            } else {
                *m = 0;
                ev.kill_sat = true;
            }
        }
    }
    // gravity
    let da = i64::from(inp.g_a) * i64::from(f);
    if s.vd == inp.g_down {
        s.vm += da;
        ev.cap |= cap3(&mut s.vm);
    } else if s.vm > da {
        s.vm -= da;
    } else {
        s.vm = da - s.vm;
        s.vd = inp.g_down;
        ev.grav = true;
    }
    for m in [&mut s.hm, &mut s.vm] {
        if *m >= 4 << 24 {
            *m = 4 << 24;
        }
    }
    let (vx1, vy1) = disp(s);
    let k = i64::from(ONE - f);
    let mut dx = (vx1 * i64::from(f)) >> 16;
    let mut dy = (vy1 * i64::from(f)) >> 16;
    if o.corr {
        dx += ((vx1 - vx0) * k) >> 17;
        dy += ((vy1 - vy0) * k) >> 17;
    }
    s.x += dx;
    s.y += dy;
    if o.pos_bits < 24 {
        let sh = 24 - o.pos_bits;
        let r = |v: i64| ((v + (1 << (sh - 1))) >> sh) << sh;
        s.x = r(s.x);
        s.y = r(s.y);
    }
    if o.spd_bits < 24 {
        let sh = 24 - o.spd_bits;
        let r = |v: i64| ((v + (1 << (sh - 1))) >> sh) << sh;
        s.hm = r(s.hm);
        s.vm = r(s.vm);
        s.hacc = r(s.hacc).min(UNIT - 1);
        s.vacc = r(s.vacc).min(UNIT - 1);
    }
    if o.order == Order::After {
        if let Some(t) = &inp.thrust {
            let e = apply_thrust(s, f, t, o, gz_of(s, inp, t, o), if o.cap_bias && inp.g_down != t.0.down { (i64::from(inp.g_a) * UNIT * i64::from(ONE - f)) >> 16 } else { 0 });
            ev.kick_v |= e.kick_v;
            ev.cross_h |= e.cross_h;
            ev.cap |= e.cap;
        }
    }
    ev
}

/// Time source: tick ends at exact cumulative times (units of 1/65536 step).
pub struct Ticker {
    pub t: u64,
    anchor: u64,
    j: u64,
    rate: u64,
    sched: Vec<(u64, u32)>,
}
impl Ticker {
    pub fn new(sched: &[(u64, u32)]) -> Ticker {
        Ticker { t: 0, anchor: 0, j: 0, rate: u64::from(sched[0].1), sched: sched.to_vec() }
    }
    pub fn next_end(&mut self) -> u64 {
        let want = self.sched.iter().filter(|(st, _)| *st <= self.t).last().map_or(self.rate, |&(_, r)| u64::from(r));
        if want != self.rate {
            self.anchor = self.t;
            self.j = 0;
            self.rate = want;
        }
        let n = 65536u64 * 30;
        self.anchor + ((self.j + 1) * n + self.rate / 2) / self.rate
    }
    pub fn advance(&mut self, end: u64) {
        self.t = end;
        self.j += 1;
    }
}

/// Gravity along the thrust direction delivered since the 30 Hz grid line (Q.24), for the phase-locked
/// reversal decision (0 when not used).
fn gz_of(_s: &S, inp: &TickIn, t: &(Thrust, u8, u8), o: &Opts) -> i64 {
    if !o.phase_kick {
        return 0;
    }
    let along = inp.g_down == t.0.down;
    let g = i64::from(inp.g_a) * i64::from(inp.phase); // Q.24: units * Q16 phase
    if along { g } else { -g }
}
