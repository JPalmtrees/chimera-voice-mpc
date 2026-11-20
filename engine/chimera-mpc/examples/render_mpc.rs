//! Render a fixed sung phrase per quality and voice, and compare against an earlier render:
//! `cargo run --release -p chimera-mpc --example render_mpc -- <out_dir> [baseline_dir]`.
//! Writes `<quality>-<voice>.wav` (32-bit float, 44.1 kHz). With a baseline it prints, per file,
//! the sample-exact difference (max |Δ|, SNR) and the long-term spectral difference in 1/3-octave
//! bands from 100 Hz to 16 kHz (mean and worst band, dB), which also compares renders that differ
//! in the airway's internal rate.
use chimera::analysis::fft::{Fft, hann};
use chimera_mpc::instrument::*;
use std::path::Path;

const FS: f64 = 44100.0;

fn render(quality: f64, voice: f64, dir: &Path) -> Vec<f32> {
    let mut p = DEFAULTS;
    p[P_QUALITY] = quality;
    p[P_VOICE] = voice;
    let inst = Instrument::new(FS, dir.to_path_buf(), &p, false);
    inst.settle();
    let mut out = Vec::new();
    let mut buf = [0.0f32; 128];
    // a phrase with legato, fricatives, plosives and a held note
    let notes: [(u8, f64, f64); 10] = [(0, 0.35, 0.3), (4, 0.35, 0.3), (7, 0.5, 0.45), (12, 0.25, 0.3), (7, 0.25, 0.2), (5, 0.35, 0.3), (4, 0.35, 0.3), (2, 0.3, 0.25), (0, 1.2, 1.0), (0, 0.6, 0.0)];
    for &(n, len, hold) in &notes {
        let note = 48 + n + 12 * voice as u8;
        inst.midi(0x90, note, 100);
        inst.pump();
        let blocks = (len * FS / 128.0) as usize;
        let off = (hold * FS / 128.0) as usize;
        for b in 0..blocks {
            if b == off {
                inst.midi(0x80, note, 0);
                inst.pump();
            }
            inst.render(&mut buf);
            out.extend_from_slice(&buf);
            inst.pump();
        }
    }
    out
}

fn write_wav(path: &Path, x: &[f32]) {
    let mut b = Vec::with_capacity(44 + x.len() * 4);
    let data = (x.len() * 4) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(FS as u32).to_le_bytes());
    b.extend_from_slice(&(FS as u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for v in x {
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

fn read_wav(path: &Path) -> Option<Vec<f32>> {
    let b = std::fs::read(path).ok()?;
    Some(b[44..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect())
}

/// Long-term average power in 1/3-octave bands, 100 Hz .. 16 kHz (dB).
fn bands(x: &[f32]) -> Vec<f64> {
    let n = 4096;
    let fft = Fft::new(n);
    let w = hann(n);
    let mut acc = vec![0.0; n / 2 + 1];
    let mut frames = 0;
    let mut i = 0;
    while i + n <= x.len() {
        let mut re: Vec<f64> = (0..n).map(|k| x[i + k] as f64 * w[k]).collect();
        let mut im = vec![0.0; n];
        fft.transform(&mut re, &mut im);
        for k in 0..=n / 2 {
            acc[k] += re[k] * re[k] + im[k] * im[k];
        }
        frames += 1;
        i += n / 2;
    }
    let mut out = Vec::new();
    let mut f = 100.0f64;
    while f < 16000.0 {
        let (lo, hi) = (f / 2f64.powf(1.0 / 6.0), f * 2f64.powf(1.0 / 6.0));
        let (klo, khi) = ((lo / FS * n as f64) as usize, ((hi / FS * n as f64) as usize).max((lo / FS * n as f64) as usize + 1));
        let e: f64 = acc[klo..khi].iter().sum::<f64>() / frames.max(1) as f64;
        out.push(10.0 * (e + 1e-20).log10());
        f *= 2f64.powf(1.0 / 3.0);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = Path::new(&args[1]);
    std::fs::create_dir_all(out_dir).unwrap();
    let base = args.get(2).map(Path::new);
    let only: Option<usize> = std::env::var("QUALITY").ok().and_then(|q| q.parse().ok());
    let dir = std::env::temp_dir().join("chimera-mpc-render/VST2 Plugins/Gator Audio - VST - Chimera Voice");
    std::fs::create_dir_all(dir.join("Content")).unwrap();
    std::fs::write(dir.join("Content/lyrics.txt"), "Hello there, I am a voice made of air. Sing it slowly, softly, everywhere.").unwrap();
    for (qi, q) in QUALITY_NAMES.iter().enumerate() {
        if only.is_some_and(|o| o != qi) {
            continue;
        }
        for (vi, v) in ["male", "female"].iter().enumerate() {
            let x = render(qi as f64, vi as f64, &dir);
            let name = format!("{q}-{v}.wav");
            write_wav(&out_dir.join(&name), &x);
            let peak = x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rms = (x.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
            print!("{name}: peak {peak:.3}, rms {:.1} dBFS", 20.0 * rms.log10());
            // compare against the same name in the baseline, or (REF=<name>) a chosen reference
            let ref_name = std::env::var("REF").map(|r| format!("{r}-{v}.wav")).unwrap_or(name.clone());
            if let Some(y) = base.and_then(|b| read_wav(&b.join(&ref_name))) {
                let n = x.len().min(y.len());
                let (mut d2, mut s2, mut dmax) = (0.0f64, 0.0f64, 0.0f32);
                for i in 0..n {
                    let d = x[i] - y[i];
                    d2 += (d as f64).powi(2);
                    s2 += (y[i] as f64).powi(2);
                    dmax = dmax.max(d.abs());
                }
                let (bx, by) = (bands(&x[..n]), bands(&y[..n]));
                let diffs: Vec<f64> = bx.iter().zip(&by).map(|(a, b)| (a - b).abs()).collect();
                let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
                let worst = diffs.iter().cloned().fold(0.0, f64::max);
                if std::env::var("BANDS").is_ok() {
                    let d: Vec<String> = bx.iter().zip(&by).map(|(a, b)| format!("{:+.1}", a - b)).collect();
                    println!("
  bands 100 Hz..16 kHz (1/3 oct, dB vs ref): {}", d.join(" "));
                }
                print!("  vs {ref_name}: max|d| {dmax:.2e}, SNR {:.1} dB, bands mean {mean:.2} dB worst {worst:.2} dB", 10.0 * (s2 / d2.max(1e-30)).log10());
            }
            println!();
        }
    }
}
