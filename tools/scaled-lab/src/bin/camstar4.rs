use exp::camera::*;
fn main() {
    let world = exp::world();
    for r in synthetic(&world) { if r.name != "synth-l0-2-0" { continue; }
        let segs = to_segs(&r); let s = &segs[0];
        let (o, _) = orig_stars(s);
        let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
        let t = run_cand(s, 60.0, 0.0, c);
        let sc = scaled_stars(s, &t, false);
        let mut j = 0;
        for u in 4..o.len() {
            while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; }
            if o[u].mode != sc[j].mode { 
                for uu in u.saturating_sub(12)..u+4 { println!("u{uu} orig mode {} sky(step {}) {} | pod sky seq {:?}", o[uu].mode, uu/2, s.sky[uu/2], &s.sky[(uu/2).saturating_sub(3)..(uu/2+3).min(s.sky.len())]); }
                let mut jj = j; while jj>0 && t[jj].u > u as f64 - 14.0 { jj -= 1; }
                for k in jj..=j { println!("  scaled tick u{:.1} mode {}", t[k].u, sc[k].mode); }
                break; }
        }
    }
}
