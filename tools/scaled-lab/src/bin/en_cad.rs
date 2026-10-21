// Cadence on the Q32 game-time clock: events k/30 s (k = whole steps) fire on the first tick at/after game_time >= k<<32.
fn main() {
    let rates = [30i64, 60, 75, 90, 100, 120, 144, 165, 200, 137];
    println!("event lateness vs ideal k/30 s, and N-step cadences (staggered gate (slot+k)%N==0), 600 s windows, 16 random tick phases");
    println!("rate | late mean/max ms | cadence N=4 (133ms): mean int, sd, min..max ms | N=32 (1067ms) | N=128 (4267ms) | events in 600 s N=32: orig / scaled min..max | life 192 steps (6400 ms): sd, range ms | drone timer 64 ticks at 1..3 per step: shots/s vs orig");
    for &r in &rates {
        let f32q: u64 = ((1u128 << 32) * 30 / r as u128) as u64; // floor: rate may drift; use rounding
        let f32q = f32q + (((1u128 << 32) * 30 % r as u128) * 2 >= r as u128) as u64;
        let mut late_sum = 0.0; let mut late_max: f64 = 0.0; let mut late_n = 0.0;
        let mut res: Vec<(f64, f64, f64, f64)> = vec![]; // per N: mean sd min max
        let mut counts: Vec<u64> = vec![]; let mut life_dev: Vec<f64> = vec![];
        let mut ints: [Vec<f64>; 3] = [vec![], vec![], vec![]];
        for p in 0..16 {
            let phase = (f32q as f64 * (p as f64 + 0.5) / 16.0) as u64; // game_time at first tick
            let mut gt: u64 = phase; let mut k_next: u64 = 1;
            let mut times: Vec<f64> = vec![]; // ms of event k fired
            let total_steps = 600 * 30;
            while k_next <= total_steps {
                gt += f32q;
                while (k_next << 32) <= gt && k_next <= total_steps {
                    // tick time in ms: ticks occur at (tick index)/R -> use gt (steps) as time proxy: late = (gt - k<<32)/2^32 steps
                    let late_steps = (gt - (k_next << 32)) as f64 / 4294967296.0;
                    late_sum += late_steps * 1000.0 / 30.0; late_n += 1.0; late_max = late_max.max(late_steps * 1000.0 / 30.0);
                    times.push((k_next as f64 + late_steps) * 1000.0 / 30.0);
                    k_next += 1;
                }
            }
            for (ni, &n) in [4usize, 32, 128].iter().enumerate() {
                let sl = (p * 5) % n; // slot stagger
                let ev: Vec<f64> = (1..=total_steps as usize).filter(|k| (k + sl) % n == 0).map(|k| times[k - 1]).collect();
                for w in ev.windows(2) { ints[ni].push(w[1] - w[0]); }
                if n == 32 { counts.push(ev.len() as u64); }
            }
            // life 192 steps between events k and k+192 for creations at all k
            for k in (1..(total_steps as usize - 200)).step_by(7) { life_dev.push(times[k + 191] - times[k - 1] - 6400.0); }
        }
        for v in ints.iter() { let m = v.iter().sum::<f64>() / v.len() as f64; let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt(); let mn = v.iter().cloned().fold(1e9, f64::min); let mx = v.iter().cloned().fold(0.0, f64::max); res.push((m, sd, mn, mx)); }
        let ldm = life_dev.iter().cloned().fold(1e9, f64::min); let ldx = life_dev.iter().cloned().fold(-1e9, f64::max);
        let lsd = (life_dev.iter().map(|x| x * x).sum::<f64>() / life_dev.len() as f64).sqrt();
        let exp32 = 600 * 30 / 32;
        println!("{r:3} | {:.1}/{:.1} | N4: {:.2} sd {:.2} {:.1}..{:.1} | N32: {:.2} sd {:.2} {:.1}..{:.1} | N128: {:.2} sd {:.2} {:.1}..{:.1} | N32 events {}/{}..{} | life dev rms {:.2} range {:.1}..{:.1}",
            late_sum / late_n, late_max, res[0].0, res[0].1, res[0].2, res[0].3, res[1].0, res[1].1, res[1].2, res[1].3, res[2].0, res[2].1, res[2].2, res[2].3, exp32, counts.iter().min().unwrap(), counts.iter().max().unwrap(), lsd, ldm, ldx);
    }
}
