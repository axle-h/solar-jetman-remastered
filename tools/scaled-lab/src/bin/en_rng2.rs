fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    let mut n = 0u64; let mut c = [0u64; 8];
    let mut hist16 = [0u64; 16]; let mut h64a = [0u64; 64]; let mut h64b = [0u64; 64]; let mut h4 = [0u64; 4];
    for r in &runs { for f in &r.frames { n += 1; let (o, r1, r2) = (f.rng, f.r1, f.r2);
        if r1 % 64 == 0x17 { c[0] += 1; } if (r1 >> 1) % 64 == 0x17 { c[1] += 1; } if o & 0x3E == 0x16 { c[2] += 1; } if r1 % 64 == 0x16 { c[3] += 1; } if o % 4 == 0 { c[4] += 1; } if o % 8 == 0 { c[5] += 1; } if r1 % 8 == 3 { c[6] += 1; }
        hist16[(o % 16) as usize] += 1; h64a[(r1 % 64) as usize] += 1; h64b[(r2 % 64) as usize] += 1; h4[(o % 4) as usize] += 1; } }
    println!("{n} frames");
    for (nm, p, i) in [("R1%64==0x17", 1.0/64.0, 0), ("(R1>>1)%64==0x17", 1.0/64.0, 1), ("R&0x3E==0x16", 1.0/32.0, 2), ("R1%64==0x16", 1.0/64.0, 3), ("R%4==0", 0.25, 4), ("R%8==0", 0.125, 5), ("R1%8==3", 0.125, 6)] {
        println!("{nm}: {:.5} ideal {:.5} (z={:.1})", c[i] as f64 / n as f64, p, (c[i] as f64 - p * n as f64) / (p * (1.0 - p) * n as f64).sqrt());
    }
    let chi = |h: &[u64]| { let e = n as f64 / h.len() as f64; h.iter().map(|&x| (x as f64 - e).powi(2) / e).sum::<f64>() };
    println!("chi2 R%16 (15 dof) {:.1}; R1%64 (63 dof) {:.1}; R2%64 {:.1}; R%4 (3 dof) {:.1}", chi(&hist16), chi(&h64a), chi(&h64b), chi(&h4));
}
