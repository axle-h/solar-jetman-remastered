//! Shared helpers: rates, the Q32 game clock, replay runner with per-pass hooks, statistics.
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::{Gameplay, Seed, rng::Rng};
use solar_jetman::game::level::World;

pub const RATES: [u32; 10] = [30, 60, 75, 90, 100, 120, 137, 144, 165, 200];
/// ticks of R: Q32 steps per tick
pub fn f32q(r: u32) -> u64 {
    // ceil: N steps at an integral number of ticks always reach the deadline (drift < 2^-32 step per tick)
    ((1u64 << 32) * 30).div_ceil(u64::from(r))
}
/// Q16 steps per tick
pub fn f16(r: u32) -> u32 {
    (65536 * 30 + r / 2) / r
}

pub fn replay_game(world: &World, name: &str) -> (Gameplay, solar_jetman::tools::replay::Replay) {
    let path = format!("{}/../../tools/mesen/replays/{name}.replay", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("replay");
    let r = solar_jetman::tools::replay::parse_replay(&text).expect("parse");
    let seed = Seed {
        rng: Rng { out: 0x78, high: 0xC9, low: 0x03 },
        equipment: 0,
        weapon: r.weapon,
        device: r.device,
        bullet_upgrade: r.bullet_upgrade,
        weapon_gauge: r.weapon_gauge,
        planet_flags: r.flags,
        ..Default::default()
    };
    let level = r.level;
    let mut g = Gameplay::new(world, level, seed);
    g.pod.equipment.owned = r.equipment;
    if level != 26 && level < 13 {
        (g.objects.slots[1].flags_x, g.objects.slots[1].flags_y) = (1, 1);
    }
    (g, r)
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

pub struct Xs(pub u64);
impl Xs {
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

pub fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() - 1) as f64 * p).round() as usize]
}
pub fn rms(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}
pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}
pub fn absmax(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |a, &b| a.max(b.abs()))
}
pub fn _unused(_: Buttons) {}
