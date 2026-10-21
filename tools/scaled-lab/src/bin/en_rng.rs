use solar_jetman::game::gameplay::rng::Rng;
struct X(u64);
impl X { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } fn below(&mut self, n: u64) -> u64 { self.next() % n }
         fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 } }
// continuous player: segments of (len in steps, buttons)
fn make_player(seed: u64, steps: usize, idle: bool) -> Vec<(f64, u8)> {
    let mut r = X(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
    let mut v = vec![]; let mut t = 0.0;
    while t < steps as f64 + 50.0 {
        let len = 3.0 + r.unit() * 40.0;
        let mut b = 0u8;
        if !idle {
            if r.below(100) < 70 { b |= 0x40; }
            match r.below(4) { 0 => b |= 2, 1 => b |= 1, _ => {} }
            if r.below(100) < 20 { b |= 0x80; }
        }
        v.push((t, b)); t += len;
    }
    v
}
fn held_at(p: &[(f64, u8)], t: f64, hint: &mut usize) -> u8 { while *hint + 1 < p.len() && p[*hint + 1].0 <= t { *hint += 1; } p[*hint].1 }

type Gate = (&'static str, f64, fn(u64, &mut u8, u8, u8) -> bool); // name, expected p per step, (k, out(mut), r1, r2) -> event
fn gates() -> Vec<Gate> {
    vec![
        ("generator every step (R+=$61), window", 1.0 / 128.0, |k, o, _, _| { *o = o.wrapping_add(0x61); *o % 64 == 0x1F && (2 * k >> 8) & 1 == 0 }),
        ("dome lift  k%32==0: R1%8==3", 1.0 / 8.0 / 32.0, |k, _, r1, _| k % 32 == 0 && r1 % 8 == 3),
        ("dome open  k%16==0: R%8==0", 1.0 / 8.0 / 16.0, |k, o, _, _| { if k % 16 == 0 && *o % 8 == 0 { *o = o.wrapping_add(0x49); true } else { false } }),
        ("dome drone R1%64==0x17", 1.0 / 64.0, |_, _, r1, _| r1 % 64 == 0x17),
        ("dome flame (R1>>1)%64==0x17", 1.0 / 64.0, |_, _, r1, _| (r1 >> 1) % 64 == 0x17),
        ("satellite R&0x3E==0x16", 1.0 / 32.0, |_, o, _, _| *o & 0x3E == 0x16),
        ("walker k%4==0: R%4==0", 1.0 / 16.0, |k, o, _, _| k % 4 == 0 && *o % 4 == 0),
        ("machine22 R1%64==0x16", 1.0 / 64.0, |_, _, r1, _| r1 % 64 == 0x16),
        ("orb spawner (1/128 steps *1/4 * R bit7 clear)", 1.0 / 128.0 / 4.0 / 2.0, |k, o, _, _| k % 128 == 0 && (2 * k >> 8) & 3 == 3 && *o & 0x80 == 0),
    ]
}
#[derive(Clone, Copy)]
enum Mode { Orig, Scaled(i64) }
fn simulate(mode: Mode, p: &[(f64, u8)], steps: u64, g: &Gate) -> (f64, f64, u64) {
    let mut rng = Rng { out: 0x78, high: 0xC9, low: 0x03 };
    let mut job: u8 = 8; let mut counter: u32 = 0;
    let mut hint = 0usize;
    let (mut ev, mut prev, mut both, mut prev_cnt) = (0u64, false, 0u64, 0u64);
    let mut last_h = 0u8;
    let mut pend_press = 0u8;
    // tick bookkeeping for Scaled
    let rate = match mode { Mode::Scaled(r) => r, _ => 30 };
    let ft = 30.0 / rate as f64; // steps per tick
    let mut tick = 0u64; let mut frame_done: u64 = 0; // frames processed
    let mut held_cur = 0u8;
    // sample the player at ticks; at frame events use the latest tick sample
    let total_frames = steps * 2;
    let mut k: u64 = 0;
    let mut ph = 0.37 * ft; // tick phase
    while k < steps {
        // sweep at step k happens when frame 2k is about to be processed (counter even)
        match mode {
            Mode::Orig => {
                let h = held_at(p, k as f64, &mut hint); let pr = h & !last_h; last_h = h;
                let mut o = rng.out;
                let hit = (g.2)(k, &mut o, rng.high, rng.low); rng.out = o;
                ev += hit as u64; if hit && prev { both += 1; } prev = hit; prev_cnt += prev as u64;
                for _ in 0..2 { job = (job + 1) & 7; let c = counter.to_le_bytes(); rng.step([h, pr, 0, 0, c[0], c[1], c[2], job]); counter += 1; }
            }
            Mode::Scaled(_) => {
                // advance ticks until the frame events 2k and 2k+1 are due; sweep at event 2k (before the frame step)
                for fr in [2 * k, 2 * k + 1] {
                    let due = fr as f64 * 0.5; // game time in steps
                    // ticks continue until tick time >= due
                    while ph + tick as f64 * ft < due { tick += 1; held_cur = held_at(p, ph + tick as f64 * ft, &mut hint); pend_press |= held_cur & !last_h; last_h = held_cur; }
                    if fr % 2 == 0 {
                        let mut o = rng.out;
                        let hit = (g.2)(k, &mut o, rng.high, rng.low); rng.out = o;
                        ev += hit as u64; if hit && prev { both += 1; } prev = hit; prev_cnt += prev as u64;
                    }
                    job = (job + 1) & 7; let c = counter.to_le_bytes();
                    rng.step([held_cur, pend_press, 0, 0, c[0], c[1], c[2], job]); counter += 1; pend_press = 0;
                    frame_done += 1;
                }
            }
        }
        k += 1;
    }
    let _ = (total_frames, frame_done);
    let pe = ev as f64 / steps as f64;
    let cond = if prev_cnt > 0 { both as f64 / prev_cnt as f64 } else { 0.0 };
    (pe, cond, ev)
}
fn main() {
    let steps: u64 = 3_000_000;
    let gs = gates();
    for (idle, label) in [(true, "idle input"), (false, "gameplay-like input")] {
        println!("=== {label}");
        let player = make_player(7, steps as usize, idle);
        for g in &gs {
            let mut line = format!("{:48} ideal {:.5} |", g.0, g.1);
            for mode in [Mode::Orig, Mode::Scaled(60), Mode::Scaled(144), Mode::Scaled(200)] {
                let (pe, cond, ev) = simulate(mode, &player, steps, g);
                let name = match mode { Mode::Orig => "orig".to_string(), Mode::Scaled(r) => format!("R{r}") };
                line += &format!(" {name} {pe:.5} (n={ev}, P(ev|prev)={cond:.3})");
            }
            println!("{line}");
        }
    }
}
