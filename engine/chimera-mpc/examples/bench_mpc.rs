//! Real-time cost of the MPC instrument singing (render only, planner stepped outside the timing):
//! `cargo run --release -p chimera-mpc --example bench_mpc [held]` (QUALITY=<index> for one mode).
//! Each case runs three times; the fastest run is reported (the others are noise from the OS).
use chimera_mpc::instrument::*;
use std::time::{Duration, Instant};

fn main() {
    let dir = std::env::temp_dir().join("chimera-mpc-bench/VST2 Plugins/Gator Audio - VST - Chimera Voice");
    std::fs::create_dir_all(dir.join("Content")).unwrap();
    std::fs::write(dir.join("Content/lyrics.txt"), "Hello there, I am a voice made of air. Sing it slowly, softly, everywhere.").unwrap();
    for (qi, name) in QUALITY_NAMES.iter().enumerate() {
        let q = qi as f64;
        if std::env::var("QUALITY").is_ok_and(|o| o != qi.to_string()) {
            continue;
        }
        for voice in [0.0, 1.0] {
            let mut p = DEFAULTS;
            p[P_QUALITY] = q;
            p[P_VOICE] = voice;
            let mut best = Duration::MAX;
            let mut samples = 0usize;
            for _run in 0..3 {
            let inst = Instrument::new(44100.0, dir.clone(), &p, false);
            inst.settle();
            let mut buf = [0.0f32; 128];
            let mut t = Duration::ZERO;
            samples = 0;
            let held = std::env::args().any(|a| a == "held");
            let (notes, len, off) = if held { (3, 3.0, 1000) } else { (16, 0.4, 120) };
            for k in 0..notes {
                let note = 48 + [0, 4, 7, 12, 7, 4, 2, 0][k % 8] + 12 * voice as u8;
                inst.midi(0x90, note, 100);
                inst.pump();
                for b in 0..(len * 44100.0 / 128.0) as usize {
                    if b == off {
                        inst.midi(0x80, note, 0);
                        inst.pump();
                    }
                    let t0 = Instant::now();
                    inst.render(&mut buf);
                    t += t0.elapsed();
                    samples += 128;
                    inst.pump();
                }
            }
            best = best.min(t);
            }
            let t = best;
            let rt = samples as f64 / 44100.0;
            println!("quality {name} voice {}: {:.1}% of one core ({:.3} ms per 128-sample block)", ["male", "female"][voice as usize], 100.0 * t.as_secs_f64() / rt, 1000.0 * t.as_secs_f64() / (samples / 128) as f64);
        }
    }
}

