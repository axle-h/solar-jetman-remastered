use exp::contact::*;
fn main() {
    let world = exp::world();
    let s = find_scenarios(&world);
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for x in &s { *by.entry(format!("{} {}", x.class, x.tag.split(' ').nth(1).unwrap())).or_default() += 1; }
    println!("{} scenarios", s.len());
    for (k, v) in by { println!("{k}: {v}"); }
}
