use solar_jetman::game::gameplay::objects::Object;
fn max_speed(m: u8) -> u16 { u16::from(m & 7) << 8 | u16::from(m & 0xF8) }
fn same(a: &Object, b: &Object) -> bool { (a.x,a.x_fraction,a.y,a.y_fraction,a.speed_x,a.speed_y,a.flags_x&1,a.flags_y&1)==(b.x,b.x_fraction,b.y,b.y_fraction,b.speed_x,b.speed_y,b.flags_x&1,b.flags_y&1) }
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    // (name, state, predictor)
    type P = fn(&mut Object, (u16,u16), &Object);
    let mine: P = |o,p,_| { let c = u16::from(o.index % 32); let max = 0x100 + (c%8)*0x20 + if c>=16 {0x100} else {0}; o.home_x(p.0,8,max); o.home_y(p.1,8,max); };
    let sat: P = |o,p,_| { o.home_x(p.0,3,max_speed(0xC0)); o.home_y(p.1,0x10,max_speed(0xC2)); };
    let sph: P = |o,p,_| { o.home_x(p.0,8,max_speed(4)); o.home_y(p.1,0x18,max_speed(4)); };
    let orb: P = |o,p,_| { o.home_x(p.0,2,0x80); o.home_y(p.1,2,0x80); };
    let list: [(&str,u8,P);4] = [("mine",0x1E,mine),("sat",0xB9,sat),("sphere",0xAF,sph),("orb",0x23,orb)];
    for (name, st, pred) in list {
        let mut res = [[0u32;2];2]; // [pod_offset][ok]
        for r in &runs { for i in 0..r.frames.len()-1 { for s in 3..31 {
            let a = &r.frames[i].slots[s]; let b = &r.frames[i+1].slots[s];
            if a.state != st || b.state != st { continue; }
            if st == 0x23 && a.index >= 16 { continue; }
            if st == 0xAF && !(a.work6 >= 63 && a.frame >= 0xA1) { continue; }
            for (k, pf) in [(0usize, &r.frames[i]), (1, &r.frames[i+1])] {
                let mut o = *a; pred(&mut o, (pf.px, pf.py), a);
                res[k][same(&o, b) as usize] += 1;
            }
        }}}
        println!("{name}: pod-before: ok {} bad {} | pod-after: ok {} bad {}", res[0][1], res[0][0], res[1][1], res[1][0]);
    }
}
