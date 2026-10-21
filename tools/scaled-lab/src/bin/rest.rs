//! The pod rest switch (`docs/scaled.md`, "Pod rest"), simulated with the real Scaled engine:
//! the upright pod dropped onto the flat-floor sites of scens.txt at several speeds and rates,
//! for each candidate threshold. Reports how often it comes to rest, after how many bounces and
//! how long, what the landing costs in fuel, and how far a sideways speed carries it once at rest.
//! `cargo run --release --bin rest`
use exp::contact::scenarios;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::Seed;
use solar_jetman::game::scaled::Scaled;

const PX: u32 = 1 << 24;
const RATES: [u32; 3] = [30, 60, 144];
/// Thresholds in px per step: None is the Original's hop.
const THRESHOLDS: [Option<f64>; 5] = [None, Some(1.0), Some(1.5), Some(2.0), Some(2.5)];
const DROPS: [f64; 7] = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0];

/// Landing speeds at the first contact, the bins' upper edges in px per step.
const BINS: [f64; 6] = [0.5, 1.0, 1.5, 2.0, 2.5, 9.0];

#[derive(Default, Clone)]
struct Bin {
    runs: u32,
    rested: u32,
    bounces: u32,
    rest_ms: f64,
    /// Fuel from the first contact on, and the hop's drain in the last two seconds.
    fuel: f64,
    tail: f64,
}

fn main() {
    let world = exp::world();
    let sites: Vec<_> = scenarios(&world)
        .into_iter()
        .filter(|s| s.class == "F")
        .collect();
    println!(
        "{} flat-floor sites, drops {:?} px/step; from the first floor contact, 8 s; bins by \
         the speed at that contact (px/step, upper edges {:?})",
        sites.len(),
        DROPS,
        BINS
    );
    for threshold in THRESHOLDS {
        for rate in RATES {
            let mut bins = vec![Bin::default(); BINS.len()];
            let (mut slid, mut slid_runs) = (0.0, 0u32);
            for site in &sites {
                for &drop in &DROPS {
                    for sideways in [0.0, 0.5] {
                        let mut game = Scaled::new(&world, site.level, Seed::default());
                        let (x, y) = (site.init.x, site.init.y);
                        game.warp(x, y, x.saturating_sub(128), y.saturating_sub(80));
                        game.pod.body.v_mag = (drop * f64::from(PX)) as u32;
                        game.pod.body.v_down = true;
                        game.pod.body.h_mag = (sideways * f64::from(PX)) as u32;
                        game.pod.invulnerable = 0;
                        game.pod.hit_pause = 0;
                        game.set_pod_rest(threshold.map(|t| (t * f64::from(PX)) as u32));
                        let mut first: Option<(u32, f64, u16)> = None;
                        let (mut bounces, mut was_down) = (0, true);
                        let (mut rested_at, mut rest_x) = (None, 0u32);
                        let (mut tail_from, mut whole) = (None, false);
                        let mut n = 0;
                        loop {
                            for o in game.objects.slots.iter_mut().skip(3) {
                                o.state = 0;
                            }
                            let (speed, fuel) = (game.pod.body.v_mag, game.pod.fuel);
                            game.tick(&world, Buttons(0), Buttons(0), rate);
                            n += 1;
                            if game.transition().is_some() || n > 20 * rate {
                                break;
                            }
                            if first.is_none() && game.pod.surface & 1 != 0 {
                                first = Some((n, f64::from(speed) / f64::from(PX), fuel));
                            }
                            let Some((start, ..)) = first else { continue };
                            let down = game.pod.body.v_down;
                            if was_down && !down && game.pod.surface & 1 != 0 {
                                bounces += 1;
                            }
                            was_down = down;
                            if game.pod.resting && rested_at.is_none() {
                                rested_at = Some(n - start);
                                rest_x = game.pod.body.x_q16();
                            }
                            if n - start == 6 * rate {
                                tail_from = Some(game.pod.fuel);
                            }
                            if n - start >= 8 * rate {
                                whole = true;
                                break;
                            }
                        }
                        let Some((_, speed, fuel)) = first else { continue };
                        // Only runs that stayed on the level for the whole 8 s.
                        let (true, Some(tail_from)) = (whole, tail_from) else { continue };
                        let bin = &mut bins[BINS.iter().position(|&b| speed < b).unwrap_or(5)];
                        bin.runs += 1;
                        bin.fuel += f64::from(fuel - game.pod.fuel);
                        bin.tail += f64::from(tail_from - game.pod.fuel) / 2.0;
                        if let Some(after) = rested_at {
                            bin.rested += 1;
                            bin.bounces += bounces;
                            bin.rest_ms += f64::from(after) * 1000.0 / f64::from(rate);
                            if sideways > 0.0 {
                                let moved = game.pod.body.x_q16().wrapping_sub(rest_x) as i32;
                                slid += (f64::from(moved) / 65536.0).abs();
                                slid_runs += 1;
                            }
                        }
                    }
                }
            }
            println!(
                "threshold {:>4} rate {:>3}: slid after rest {:.1} px",
                threshold.map_or("off".to_owned(), |t| format!("{t:.1}")),
                rate,
                slid / f64::from(slid_runs.max(1))
            );
            for (bin, edge) in bins.iter().zip(BINS) {
                let rested = bin.rested.max(1) as f64;
                println!(
                    "    landing < {edge:.1}: {:>3} runs, rested {:>5.1}%, bounces before {:.2}, \
                     after {:>4.0} ms, fuel in 8 s {:>5.0}, drain at the end {:>4.0} units/s",
                    bin.runs,
                    100.0 * f64::from(bin.rested) / f64::from(bin.runs.max(1)),
                    f64::from(bin.bounces) / rested,
                    bin.rest_ms / rested,
                    bin.fuel / f64::from(bin.runs.max(1)),
                    bin.tail / f64::from(bin.runs.max(1)),
                );
            }
        }
    }
    // Leaving: at rest, B lifts the pod off on the first tick that sees it.
    let site = &sites[0];
    let mut game = Scaled::new(&world, site.level, Seed::default());
    game.warp(site.init.x, site.init.y, site.init.x - 128, site.init.y - 80);
    game.set_pod_rest(Some(2 * PX));
    for _ in 0..600 {
        game.tick(&world, Buttons(0), Buttons(0), 144);
    }
    let y = game.pod.body.y;
    let resting = game.pod.resting;
    for _ in 0..30 {
        game.tick(&world, Buttons(Buttons::B), Buttons(Buttons::B), 144);
    }
    println!(
        "leaving: resting {resting} at y {y}; after 30 ticks of thrust resting {} at y {}",
        game.pod.resting, game.pod.body.y
    );
}
