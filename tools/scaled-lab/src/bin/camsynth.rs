use exp::camera::*;
fn main() {
    let world = exp::world();
    for run in synthetic(&world) {
        let segs = to_segs(&run);
        let maxd = run.passes.iter().map(|p| p.delta.0.unsigned_abs().max(p.delta.1.unsigned_abs())).max().unwrap_or(0);
        let st: usize = segs.iter().map(|s| s.p.len()).sum();
        println!("{} passes {} segs {} steps {} maxd {}", run.name, run.passes.len(), segs.len(), st, maxd);
    }
}
