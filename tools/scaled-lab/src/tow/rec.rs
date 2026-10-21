//! Recording of real steps with the tow state before and after.
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::pod::STATE_POD;
use solar_jetman::game::gameplay::rng::Rng;
use solar_jetman::game::gameplay::{Gameplay, Seed};
use solar_jetman::game::level::{PLANETS, World};

#[derive(Clone, Copy, Debug, Default)]
pub struct T {
    pub slot: usize,
    pub mass: u8,
    pub angle: u16,
    pub speed: u32,
    pub sense: bool,
    pub spin: u8,
    pub active: bool,
    pub taken_in: bool,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct P {
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
    pub facing: u8,
    pub fuel: u16,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Rec {
    pub step: usize,
    pub held: u8,
    pub pre_t: T,
    pub post_t: T,
    pub pre_p: P,
    pub post_p: P,
    /// gravity: down?, a = |g|>>2
    pub g_down: bool,
    pub g_a: u16,
    pub anti: bool,
    pub state: u8,
    pub surface: u8,
    pub item: (u16, u16),
    pub scratch: u8,
    pub spin_pts: bool,
    pub spin_after: u8,
}

pub fn tsnap(g: &Gameplay) -> T {
    let t = &g.tow;
    T { slot: t.slot, mass: t.mass, angle: t.angle, speed: t.speed, sense: t.sense, spin: t.spin, active: t.active(), taken_in: t.taken_in }
}
pub fn psnap(g: &Gameplay) -> P {
    let p = &g.pod;
    let b = &p.body;
    P { x: b.x, xf: b.x_fraction, y: b.y, yf: b.y_fraction, h: b.h_speed, hl: b.h_left, v: b.v_speed, vd: b.v_down, ah: p.accumulator_h, av: p.accumulator_v, facing: p.facing, fuel: p.fuel }
}

pub fn gravity_of(world: &World, g: &Gameplay) -> (bool, u16) {
    let level = &world.levels[g.level()];
    let y = g.pod.body.y;
    if usize::from(level.number) < PLANETS && y < 64 {
        let value = ((65 - y) * 2) as u8;
        return (true, u16::from(value) / 4);
    }
    let byte = if g.pod.gravity_override != 0 { g.pod.gravity_override } else { level.gravity as u8 };
    let gg = i16::from(byte as i8);
    (gg >= 0, gg.unsigned_abs() / 4)
}

pub fn seed(equipment: u8, device: u8) -> Seed {
    Seed { rng: Rng { out: 0x78, high: 0xC9, low: 0x03 }, equipment, device, ..Default::default() }
}

/// Runs a game over inputs (with a per-step hook before the step) recording every A-step.
pub fn run(world: &World, mut g: Gameplay, inputs: &[Buttons], hook: &mut dyn FnMut(&mut Gameplay, usize)) -> Vec<Rec> {
    let mut out = Vec::new();
    for (i, &b) in inputs.iter().enumerate() {
        hook(&mut g, i + 1);
        if !g.next_is_a_pass() {
            break;
        }
        let lvl = g.level();
        let (down, a) = gravity_of(world, &g);
        let pre_t = tsnap(&g);
        let pre_p = psnap(&g);
        let anti = g.pod.antigravity;
        g.step(world, b);
        if g.level() != lvl {
            break;
        }
        out.push(Rec {
            step: i + 1,
            held: b.0,
            pre_t,
            post_t: tsnap(&g),
            pre_p,
            post_p: psnap(&g),
            g_down: down,
            g_a: a,
            anti,
            state: g.pod.state,
            surface: g.pod.surface,
            item: if pre_t.slot != 0 { (g.objects.slots[pre_t.slot].x, g.objects.slots[pre_t.slot].y) } else { (0, 0) },
            scratch: 20 + (g.pod.body.y & 15) as u8,
            ..Default::default()
        });
        let _ = STATE_POD;
    }
    out
}

pub fn game_for(world: &World, r: &solar_jetman::tools::replay::Replay, original: bool) -> Gameplay {
    use solar_jetman::game::gameplay::{PodLeftovers, Seed};
    let _ = PodLeftovers::default();
    let sd = Seed { weapon: r.weapon, bullet_upgrade: r.bullet_upgrade, weapon_gauge: r.weapon_gauge, planet_flags: r.flags, ..seed(r.equipment, r.device) };
    let mut g = Gameplay::new(world, r.level, sd);
    g.set_original_rules(original);
    g.pod.equipment.owned = r.equipment;
    if r.demo.is_none() && r.level < 13 {
        (g.objects.slots[1].flags_x, g.objects.slots[1].flags_y) = (1, 1);
    }
    g
}

pub fn setups(g: &mut Gameplay, r: &solar_jetman::tools::replay::Replay, step: usize) {
    if r.keep_fuel.is_some_and(|from| step >= from) {
        g.pod.fuel = 0xE000;
    }
    if let Some(&fuel) = r.fuel_pokes.get(&step) {
        g.pod.fuel = fuel;
    }
    if let Some(&[x, y, cx, cy]) = r.warps.get(&step) {
        g.warp(x, y, cx, cy);
    }
    for &(slot, state, x, y, frame, index) in r.objs.get(&step).into_iter().flatten() {
        g.set_object(slot, state, x, y, frame, index);
    }
    if let Some(&spin) = r.spins.get(&step) {
        g.tow.spin = spin;
    }
    for &(index, kind, x, y) in r.items.get(&step).into_iter().flatten() {
        g.set_item(index, kind, x, y);
    }
}

pub fn replay_recs(world: &World, name: &str) -> Option<Vec<Rec>> {
    replay_recs_with(world, name, true)
}
pub fn replay_recs_with(world: &World, name: &str, original: bool) -> Option<Vec<Rec>> {
    let path = format!("{}/../../tools/mesen/replays/{name}.replay", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).ok()?;
    let r = solar_jetman::tools::replay::parse_replay(&text).ok()?;
    if r.demo.is_some() || r.level >= 26 {
        return None;
    }
    let g = game_for(world, &r, original);
    let rr = r.clone();
    Some(run(world, g, &r.steps, &mut |g, s| setups(g, &rr, s)))
}

pub struct XR(pub u64);
impl XR {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A synthetic tow: level `level`, item `kind` hung 24 px below the pod's start, `mode` 0 random
/// bursts, 1 pumping controller. Returns the records.
pub fn synth_tow(world: &World, level: usize, kind: u8, pick: usize, mode: u32, seed: u64, steps: usize, original: bool) -> Vec<Rec> {
    let mut g = Gameplay::new(world, level, seed_for(0, 0));
    g.set_original_rules(original);
    // the first free-standing towable item of the level
    let it = g.objects.items.iter().filter(|e| matches!(e.kind, 3..=10 | 12..=16)).nth(pick).copied();
    let it = it.expect("item");
    let (x, y) = (it.x, it.y.saturating_sub(26));
    let (cx, cy) = (x.saturating_sub(120), y.saturating_sub(100));
    let _ = kind;
    g.warp(x, y, cx, cy);
    g.pod.fuel = 0xE000;
    let mut r = XR(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
    let mut inputs: Vec<Buttons> = vec![];
    let mut out = vec![];
    let mut cur = 0u8;
    let mut left = 0u32;
    for i in 0..steps {
        g.pod.fuel = 0xE000;
        let held = if i < 8 {
            0
        } else if mode == 0 {
            if left == 0 {
                left = 3 + r.below(40) as u32;
                cur = 0;
                if r.below(100) < 60 { cur |= Buttons::B; }
                match r.below(4) { 0 => cur |= Buttons::LEFT, 1 => cur |= Buttons::RIGHT, _ => {} }
            }
            left -= 1;
            cur
        } else if mode == 2 {
            // torque pump: choose the facing (f-1, f, f+1) whose torque most increases W
            let f0 = g.pod.facing;
            let mut best = (i64::MIN, 0u8);
            for (d, btn) in [(0i32, 0u8), (-1, Buttons::LEFT), (1, Buttons::RIGHT)] {
                let fac = ((i32::from(f0) + d).rem_euclid(32)) as u8;
                let mut t = g.tow;
                let before = t.speed as i64;
                let sense0 = t.sense;
                t.torque(fac, 0);
                let after = if t.sense != sense0 { -(t.speed as i64) } else { t.speed as i64 };
                let gain = after - before;
                if gain > best.0 { best = (gain, btn); }
            }
            let mut b = best.1;
            if best.0 > 0 { b |= Buttons::B; }
            b
        } else {
            // pumping: face sideways (rotate toward facing 8 (right) or 24 (left)), thrust while rope moves with it
            let a = g.tow.angle8();
            let w_dir = if g.tow.sense { -1i32 } else { 1 };
            let fast = g.tow.speed >> 8 > 0x40;
            let want_right = (w_dir > 0) == (a < 64) || !fast;
            let target = if want_right { 8u8 } else { 24u8 };
            let f = g.pod.facing;
            let d = (i32::from(target) - i32::from(f)).rem_euclid(32);
            let mut b = 0u8;
            if d != 0 { b |= if d < 16 { Buttons::RIGHT } else { Buttons::LEFT }; }
            if d == 0 || r.below(100) < 10 { b |= Buttons::B; }
            b
        };
        let _ = &mut inputs;
        if !g.next_is_a_pass() { break; }
        let lvl = g.level();
        let (down, a) = gravity_of(world, &g);
        let pre_t = tsnap(&g);
        let pre_p = psnap(&g);
        let anti = g.pod.antigravity;
        g.step(world, Buttons(held));
        if g.level() != lvl { break; }
        out.push(Rec { step: i + 1, held, pre_t, post_t: tsnap(&g), pre_p, post_p: psnap(&g), g_down: down, g_a: a, anti, state: g.pod.state, surface: g.pod.surface, item: (0, 0), scratch: 20 + (g.pod.body.y & 15) as u8, ..Default::default() });
    }
    out
}
pub fn seed_for(e: u8, d: u8) -> Seed { seed(e, d) }
