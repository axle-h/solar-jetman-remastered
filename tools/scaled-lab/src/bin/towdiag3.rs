use exp::tow::scaled::*;
use exp::tow::seg::*;
use solar_jetman::game::gameplay::tables::SIN;
fn main() {
    let world = exp::world();
    let segs = dataset(&world, false, true);
    let s = segs.iter().find(|s| s.name == "syn-L7-p0-m0@2").unwrap();
    for i in 74..84 {
        let r = &s.recs[i];
        let p = &r.pre_t;
        let a8 = (p.angle >> 8) as u8;
        let mut ro = Rope::from_real(p.angle, p.speed, p.sense, p.mass);
        let w0 = ro.speed24();
        let pam = u32::from(SIN[usize::from((2 * a8) & 0x7F)]) * ((8 * u32::from(r.g_a)) & 0xFF);
        let add = !(!r.g_down ^ (a8 >= 64) ^ p.sense);
        ro.pull(r.g_a, !r.g_down, 65536, Cap::Glitch);
        let w1 = ro.speed24();
        ro.torque(r.post_p.facing, true, 0, 65536, Cap::Glitch);
        let w2 = ro.speed24();
        println!("{i}: A8 {a8} sense {} W {w0:06x} pull {} {pam:04x} -> {w1:06x}; torque -> {w2:06x} (thrust {}) real post {:06x}", p.sense, if add {"add"} else {"sub"}, thrust_of(r), r.post_t.speed);
    }
}
