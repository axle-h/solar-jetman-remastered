const CIRCLE: [u8; 128] = [
    0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0D, 0x0E, 0x10, 0x12, 0x13, 0x15, 0x17, 0x18, 0x1A, 0x1B,
    0x1D, 0x1E, 0x20, 0x21, 0x22, 0x24, 0x25, 0x26, 0x27, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x34, 0x35, 0x36, 0x37, 0x37, 0x38, 0x39, 0x39, 0x3A, 0x3A, 0x3B,
    0x3B, 0x3C, 0x3C, 0x3C, 0x3D, 0x3D, 0x3D, 0x3E, 0x3E, 0x3E, 0x3F, 0x3F, 0x3F, 0x3F, 0x3F, 0x3F,
    0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x3F, 0x3F, 0x3F, 0x3E, 0x3E, 0x3E, 0x3D, 0x3D,
    0x3C, 0x3C, 0x3B, 0x3B, 0x3A, 0x39, 0x39, 0x38, 0x37, 0x36, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31,
    0x30, 0x2F, 0x2E, 0x2D, 0x2C, 0x2B, 0x29, 0x28, 0x27, 0x26, 0x24, 0x23, 0x22, 0x20, 0x1F, 0x1D,
    0x1C, 0x1A, 0x19, 0x17, 0x15, 0x14, 0x12, 0x10, 0x0F, 0x0D, 0x0B, 0x09, 0x07, 0x05, 0x03, 0x01,
];
fn pos(a: u8) -> (i32, i32) {
    let (q, i) = (a / 64, a % 64);
    let j = usize::from(if q % 2 == 1 { 63 - i } else { i });
    let along = i32::from(CIRCLE[j]); let across = i32::from(CIRCLE[j + 64]);
    (if q < 2 { along } else { -along }, if q == 0 || q == 3 { -across } else { across })
}
fn main() {
    // validate against the real engine: formation parts: x - home == pos(phase) where phase = x_fraction + 16*(kind-first)
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    let (mut ok, mut bad) = (0u64, 0u64);
    for r in &runs { for f in &r.frames { for s in 23..31 { let o = &f.slots[s];
        if (o.state == 0xA9 || o.state == 0xAA) && o.work6 & 1 != 0 {
            let first = if o.state == 0xA9 { 0x23u8 } else { 0x33 };
            let a = o.x_fraction.wrapping_add(16u8.wrapping_mul(o.kind.wrapping_sub(first)));
            let (dx, dy) = pos(a);
            let (hx, hy) = (i32::from(o.speed_x), i32::from(o.speed_y));
            if i32::from(o.x) == hx + dx && i32::from(o.y) == hy + dy { ok += 1 } else { bad += 1 }
        }}}}
    println!("table model vs real formation parts: exact {ok}, mismatches {bad}");
    // path metrics
    let mut maxstep: f64 = 0.0; let mut sum = 0.0;
    for a in 0..=255u8 { let (x0, y0) = pos(a); let (x1, y1) = pos(a.wrapping_add(1)); let d = (((x1 - x0).pow(2) + (y1 - y0).pow(2)) as f64).sqrt(); maxstep = maxstep.max(d); sum += d; }
    println!("per-step displacement: max {maxstep:.2} px, mean {:.2} px", sum / 256.0);
    // smooth reference: Catmull-Rom through the 256 integer samples; chord error of the linear interpolation
    let p = |a: i32| pos(a.rem_euclid(256) as u8);
    let (mut maxe, mut sq, mut n) = (0f64, 0f64, 0f64);
    for a in 0..256 { for sub in 1..10 { let u = sub as f64 / 10.0;
        let (p0, p1, p2, p3) = (p(a - 1), p(a), p(a + 1), p(a + 2));
        let cr = |c0: i32, c1: i32, c2: i32, c3: i32| 0.5 * ((2 * c1) as f64 + (-c0 + c2) as f64 * u + (2 * c0 - 5 * c1 + 4 * c2 - c3) as f64 * u * u + (-c0 + 3 * c1 - 3 * c2 + c3) as f64 * u * u * u);
        let (sx, sy) = (cr(p0.0, p1.0, p2.0, p3.0), cr(p0.1, p1.1, p2.1, p3.1));
        let (lx, ly) = (p1.0 as f64 + (p2.0 - p1.0) as f64 * u, p1.1 as f64 + (p2.1 - p1.1) as f64 * u);
        let e = ((sx - lx).powi(2) + (sy - ly).powi(2)).sqrt(); maxe = maxe.max(e); sq += e * e; n += 1.0; } }
    println!("linear interpolation vs Catmull-Rom of the table: max {maxe:.3} px rms {:.3} px", (sq / n).sqrt());
    // table vs ideal circle radius 64 (centre offset 0.5)
    let (mut maxq, mut rr) = (0f64, 0f64);
    for a in 0..256 { let (x, y) = pos(a as u8); let th = (a as f64 + 0.5) * std::f64::consts::TAU / 256.0; let _ = th;
        let r = ((x as f64).powi(2) + (y as f64).powi(2)).sqrt(); rr = rr.max(r); maxq = maxq.max((r - 64.0).abs()); }
    println!("table radius: max {rr:.1} px; deviation from a 64 px circle up to {maxq:.1} px (the table is not a circle; it is the original's path)");
    // phase advance per tick: fractional phase error of integer-tick accumulators over 1 hour in Q16 vs Q32 game clock
    for &rate in &[60i64, 144, 200, 137] {
        let f16 = (65536 * 30 + rate / 2) / rate; let ticks = rate * 3600;
        let drift_steps = (f16 as f64 * ticks as f64 / 65536.0) - 30.0 * 3600.0;
        println!("R={rate}: Q16 tick accumulator drift after 1 h = {:.1} steps ({:.0} ms); phase error {:.1} table units (256 per lap)", drift_steps, drift_steps / 30.0 * 1000.0, drift_steps);
    }
}
