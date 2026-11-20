//! Control-rate cost split: tract geometry (compute) vs the whole airway apply.
use chimera::anatomy::human::{BODY_MALE, NEUTRAL};
use chimera::voice::human_airway::HumanAirway;
use std::time::Instant;
fn main() {
    let mut air = HumanAirway::new(44100.0, BODY_MALE);
    let mut a = NEUTRAL;
    let n = 20000;
    let mut f = air.tract.compute_new(&a);
    let t0 = Instant::now();
    for i in 0..n {
        a.tb_x = 2.9 + 0.5 * ((i as f64) * 0.01).sin();
        air.tract.compute(&a, &mut f);
    }
    let comp = t0.elapsed().as_secs_f64() / n as f64;
    let (mut ph, mut or) = (vec![0.0; air.n_ph], vec![0.0; air.n_or]);
    let (mut mp, mut mo) = (ph.clone(), or.clone());
    let t0 = Instant::now();
    for _ in 0..n {
        air.tract.resample(&a, &f, &mut ph, &mut or, Some(&mut mp), Some(&mut mo));
    }
    let res = t0.elapsed().as_secs_f64() / n as f64;
    let t0 = Instant::now();
    for i in 0..n {
        a.tb_x = 2.9 + 0.5 * ((i as f64) * 0.01).sin();
        air.apply(&a, 64.0 / 44100.0);
        air.net.begin_block(64);
    }
    let all = t0.elapsed().as_secs_f64() / n as f64;
    println!("per control step: tract.compute {:.1} us, resample {:.1} us, apply+begin_block {:.1} us", comp * 1e6, res * 1e6, all * 1e6);
}
