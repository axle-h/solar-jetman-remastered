use exp::contact::*;
fn main() {
    let world = exp::world();
    let scens = scenarios(&world);
    let off = rules_from("corr+tp1+push+wfull+fxcd"); let on = rules_from(&std::env::args().nth(1).unwrap_or("corr+tp1+appr+push+wfull+fxcd".into()));
    println!("effect of the approach gate AT 30 Hz (R30 model, which equals the original with gate off): 12 s, free and thrust-pressed");
    for class in ["F", "S", "W", "C", "SC"] {
        let (mut ea, mut eb, mut fa, mut fb, mut same_first) = (0usize, 0usize, 0i64, 0i64, 0usize);
        let mut n = 0;
        let thr_of = |s: &Scen| -> (i64, i64) { match (s.class, s.tag.contains("wallL")) { ("F", _) | ("S", _) => (0, 16), ("C", _) | ("SC", _) => (0, -16), ("W", false) => (16, 0), _ => (-16, 0) } };
        for s in scens.iter().filter(|s| s.class == class) { for v in variants(class) {
            for thrust in [false, true] {
                let init = with_variant(s, v);
                let t = if thrust { thr_of(s) } else { (0, 0) };
                let a = run_thr(&world, s.level, off, 30, init, t, 4.0); let b = run_thr(&world, s.level, on, 30, init, t, 4.0);
                // compare only until the trajectories diverge: count events before the first difference
                let k = a.ev.iter().zip(&b.ev).take_while(|(x, y)| (x.t, x.kind, x.speed) == (y.t, y.kind, y.speed)).count();
                if k == a.ev.len() && a.ev.len() == b.ev.len() { same_first += 1; }
                ea += a.ev.len(); eb += b.ev.len(); fa += a.ev.iter().map(|e| e.loss).sum::<i64>(); fb += b.ev.iter().map(|e| e.loss).sum::<i64>(); n += 1;
            }
        } }
        println!("{class:3}: {n} runs, identical event streams in {same_first}; events gate-off {ea} vs gate-on {eb} ({:+.1}%), impact fuel {fa} vs {fb} ({:+.1}%)", 100.0 * (eb as f64 / ea as f64 - 1.0), 100.0 * (fb as f64 / fa.max(1) as f64 - 1.0));
    }
}
