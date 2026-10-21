//! Recording of real pod steps and the free-flight segments in them.
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::pod::STATE_POD;
use solar_jetman::game::gameplay::tables::THRUST;
use solar_jetman::game::gameplay::{Gameplay, Seed};
use solar_jetman::game::level::{PLANETS, World};

/// Pod state after one step plus what that step's rules saw.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rec {
    pub held: u8,
    pub facing: u8,
    pub x: u16,
    pub xf: u8,
    pub y: u16,
    pub yf: u8,
    pub h: u16,
    pub hl: bool,
    pub v: u16,
    pub vd: bool,
    pub ah: u8,
    pub av: u8,
    pub fuel: u16,
    pub device: u8,
    pub owned: u8,
    /// gravity of the step: pulls down, magnitude per step (1/256), the byte (thrust carry-in)
    pub g_down: bool,
    pub g_a: u16,
    pub g_byte: u8,
    /// camera carry byte the horizontal thrust saw
    pub cam: u8,
    pub free: bool,
}

#[derive(Clone, Debug)]
pub struct Seg {
    pub name: String,
    pub start: Rec,
    pub steps: Vec<Rec>,
}

fn snapshot(g: &Gameplay) -> Rec {
    let p = &g.pod;
    let b = &p.body;
    Rec {
        held: 0,
        facing: p.facing,
        x: b.x,
        xf: b.x_fraction,
        y: b.y,
        yf: b.y_fraction,
        h: b.h_speed,
        hl: b.h_left,
        v: b.v_speed,
        vd: b.v_down,
        ah: p.accumulator_h,
        av: p.accumulator_v,
        fuel: p.fuel,
        device: p.equipment.device,
        owned: p.equipment.owned,
        ..Default::default()
    }
}

fn gravity_of(world: &World, g: &Gameplay) -> (bool, u16, u8) {
    let level = &world.levels[g.level()];
    let y = g.pod.body.y;
    if usize::from(level.number) < PLANETS && y < 64 {
        let value = ((65 - y) * 2) as u8;
        return (true, u16::from(value) / 4, value);
    }
    let byte = if g.pod.gravity_override != 0 { g.pod.gravity_override } else { level.gravity as u8 };
    let gg = i16::from(byte as i8);
    (gg >= 0, gg.unsigned_abs() / 4, byte)
}

/// Runs `inputs` on a game and returns the free-flight segments (>= `min` steps).
pub fn run(world: &World, g: Gameplay, inputs: &[Buttons], name: &str, min: usize, pre: &mut dyn FnMut(&mut Gameplay, usize)) -> Vec<Seg> {
    run_as(world, g, inputs, name, min, pre, false)
}

pub fn run_as(world: &World, mut g: Gameplay, inputs: &[Buttons], name: &str, min: usize, pre: &mut dyn FnMut(&mut Gameplay, usize), jetman: bool) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut prev = snapshot(&g);
    let mut cur: Option<Seg> = None;
    for (i, &b) in inputs.iter().enumerate() {
        pre(&mut g, i + 1);
        {
            let s2 = snapshot(&g);
            if (s2.x, s2.y, s2.h, s2.v, s2.xf, s2.yf) != (prev.x, prev.y, prev.h, prev.v, prev.xf, prev.yf) {
                if let Some(s) = cur.take() {
                    if s.steps.len() >= min {
                        segs.push(s);
                    }
                }
                prev = s2;
            }
        }
        if !g.next_is_a_pass() {
            break;
        }
        let (down, a, byte) = gravity_of(world, &g);
        let cam = g.pod.camera_carry;
        let pre_state = g.pod.state;
        let pre_fuel = g.pod.fuel;
        let pre_y = g.pod.body.y;
        let pre_anti = g.pod.antigravity;
        let pre_tow = g.tow.holding();
        let lvl = g.level();
        g.step(world, b);
        if g.level() != lvl {
            break;
        }
        let mut r = snapshot(&g);
        r.held = b.0;
        r.g_down = down;
        r.g_a = a;
        r.g_byte = byte;
        r.cam = cam;
        let p = &g.pod;
        r.free = if jetman {
            pre_state == 2 && p.state == 2 && p.surface == 0 && pre_fuel > 0 && p.body.y >= 24 && pre_y >= 24 && p.events.is_empty() && !pre_anti
        } else {
            pre_state == STATE_POD
            && p.state == STATE_POD
            && p.surface == 0
            && !g.tow.active()
            && !g.tow.holding()
            && !p.antigravity
            && !pre_anti
            && !pre_tow
            && pre_y >= 24
            && p.body.y >= 24
            && pre_fuel >= 40
            && p.events.is_empty()
            && !p.shield
        };
        if r.free {
            match &mut cur {
                Some(s) => s.steps.push(r),
                None => cur = Some(Seg { name: name.to_string(), start: prev, steps: vec![r] }),
            }
        } else if let Some(s) = cur.take() {
            if s.steps.len() >= min {
                segs.push(s);
            }
        }
        prev = r;
    }
    if let Some(s) = cur.take() {
        if s.steps.len() >= min {
            segs.push(s);
        }
    }
    segs
}

pub fn accel(table: u8, boost: bool, doubled: bool) -> u16 {
    let t = u16::from(table);
    match (boost, doubled) {
        (false, false) => (t / 2) / 2,
        (false, true) => (t / 2) * 2,
        (true, false) => t,
        (true, true) => t * 2,
    }
}

/// Thrust parameters of a step: None when B not held (or fuel short).
#[derive(Clone, Copy, Debug)]
pub struct Thrust {
    pub av: u16,
    pub ah: u16,
    pub down: bool,
    pub left: bool,
}

pub fn thrust_of(r: &Rec) -> Option<Thrust> {
    if r.held & Buttons::B == 0 {
        return None;
    }
    let boost = r.owned & 0x40 != 0 && r.held & Buttons::SELECT != 0;
    let doubled = r.device == 11;
    let f = usize::from(r.facing);
    let (tv, th) = (THRUST[f & 15], THRUST[(f + 8) & 15]);
    Some(Thrust {
        av: accel(tv, boost, doubled),
        ah: accel(th, boost, doubled),
        down: (9..=23).contains(&f),
        left: (17..=31).contains(&f),
    })
}

pub fn cost_of(r: &Rec) -> u16 {
    let boost = r.owned & 0x40 != 0 && r.held & Buttons::SELECT != 0;
    let eff = r.device == 10;
    match (boost, eff) {
        (false, false) => 2,
        (false, true) => 1,
        (true, false) => 16,
        (true, true) => 8,
    }
}

pub fn _seed(equipment: u8, device: u8) -> Seed {
    use solar_jetman::game::gameplay::rng::Rng;
    Seed { rng: Rng { out: 0x78, high: 0xC9, low: 0x03 }, equipment, device, ..Default::default() }
}
