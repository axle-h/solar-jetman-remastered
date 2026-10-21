use exp::contact::*;
use solar_jetman::game::gameplay::input::Buttons;
fn main() {
    let world = exp::world();
    let mut rng = 12345u64;
    let mut next = move || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (rng >> 33) as u32 };
    let mut total = 0; let mut bad = 0; let mut contacts = 0;
    for level in [0usize, 1, 2, 3, 5, 8, 11, 14] {
        let lv = &world.levels[level];
        let (w, hh) = (lv.width as u32 * 256, ((lv.height as u32 - 1) / 2 + 1) * 256);
        for _case in 0..400 {
            let i = Init { x: (40 + next() % (w - 80)) as u16, y: (40 + next() % (hh - 80)) as u16, xf: next() as u8, yf: next() as u8,
                hs: (next() % 0x400) as u16, hl: next() & 1 == 1, vs: (next() % 0x400) as u16, vd: next() & 1 == 1 };
            let mut p = make_pod(&world, level, i);
            let mut m = M::from_init(i);
            let sim = Sim { world: &world, level: lv, rules: rules_from(&std::env::args().nth(1).unwrap_or_default()) };
            total += 1;
            for n in 0..150 {
                orig_step(&world, level, &mut p, n, Buttons(0));
                sim.tick(&mut m, ONE);
                let b = &p.body;
                if m.x < 0 { break; }
                let ok = (b.x as i32, b.x_fraction as i32, b.y as i32, b.y_fraction as i32) == (m.xw(), ((m.x >> 8) & 255) as i32, m.yw(), m.yfrac())
                    && b.h_speed as i64 == M::mag(m.vx) && b.v_speed as i64 == M::mag(m.vy)
                    && (b.h_speed == 0 || b.h_left == (m.vx < 0)) && (b.v_speed == 0 || b.v_down == (m.vy >= 0))
                    && p.fuel as i64 == m.fuel && (p.hit_pause as i64) == (m.hp >> 16) && p.surface == m.surface;
                if p.surface != 0 { contacts += 1; }
                if !ok { bad += 1; if bad < 6 { println!("MISMATCH L{level} {i:?} step {n}: orig x{} y{}.{:02x} h{}{} v{}{} fuel{:x} hp{} s{:x} | model x{} y{}.{:02x} h{:x} v{:x} fuel{:x} hp{} s{:x}", b.x,b.y,b.y_fraction,b.h_left,b.h_speed,b.v_down,b.v_speed,p.fuel,p.hit_pause,p.surface, m.xw(), m.yw(), m.yfrac(), m.vx>>16, m.vy>>16, m.fuel, m.hp>>16, m.surface); } break; }
            }
        }
    }
    println!("cases {total} mismatched {bad} contact-steps {contacts}");
}
