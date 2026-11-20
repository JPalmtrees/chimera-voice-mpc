//! Where the time goes at the MPC's rates: geometry (per control step) vs FDTD (per sample) vs glottis.
use chimera::anatomy::human::{BODY_MALE, NEUTRAL};
use chimera::voice::human_airway::HumanAirway;
use std::time::Instant;
fn main() {
    for m in [1usize, 2] {
        let fs = 44100.0 * m as f64;
        let mut air = HumanAirway::new(fs, BODY_MALE);
        let mut a = NEUTRAL;
        let ctrl = 64 * m;
        let n_ctrl = (fs * 4.0) as usize / ctrl;
        let t0 = Instant::now();
        for i in 0..n_ctrl {
            a.tb_x = 2.9 + 0.5 * ((i as f64) * 0.01).sin();
            air.apply(&a, ctrl as f64 / fs);
            air.net.begin_block(ctrl);
        }
        let geo = t0.elapsed().as_secs_f64();
        let net = &mut air.net;
        let n = (fs * 4.0) as usize;
        let t0 = Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            if i % ctrl == 0 {
                net.begin_block(ctrl);
            }
            acc += net.step();
        }
        let st = t0.elapsed().as_secs_f64();
        println!("{m}x: nodes {} branches {}; per 4 s: geometry {:.1} ms ({:.1}%), FDTD {:.1} ms ({:.1}%) [{acc:.3e}]", net.n_nodes, net.n_branches, geo * 1e3, geo / 40.0 * 1e3 / 1e1, st * 1e3, st / 40.0 * 1e3 / 1e1);
    }
}
