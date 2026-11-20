//! The voice the audio thread renders: CHIMERA's `HumanVoice`, either at the host's rate (Eco, Std,
//! High: unchanged) or, for the Lite quality, simulated at a lower internal rate and resampled up.
//!
//! Why a lower rate saves so much: the airway's sections must be at least one sound-travel step
//! long (c·T), so at 3/4 of the rate the tract has about 3/4 of the sections AND 3/4 of the time
//! steps: the per-sample simulation costs about (3/4)² ≈ 56% as much. The voice keeps everything
//! below the internal Nyquist (16.5 kHz at a 44.1 kHz host), which covers the voice and its
//! fricatives; the resampler is a 4:3 polyphase FIR (Kaiser, 32 taps a phase), whose images and
//! the band above 0.5·f_internal are attenuated by ~70 dB.
use chimera::voice::human_voice::HumanVoice;
use std::ops::{Deref, DerefMut};

/// Taps per polyphase branch.
const TAPS: usize = 32;
/// Largest output chunk rendered at once (bounds the scratch buffer; longer blocks loop).
const CHUNK: usize = 1024;

pub struct Voice {
    pub hv: HumanVoice,
    rs: Option<Resampler>,
}

impl Voice {
    /// `ratio` = (m, l): the voice runs at `fs_out · m / l` (m < l), or at `fs_out` when m == l.
    pub fn new(hv: HumanVoice, ratio: (usize, usize)) -> Voice {
        let rs = (ratio.0 != ratio.1).then(|| Resampler::new(ratio.0, ratio.1));
        Voice { hv, rs }
    }

    /// The rate the airway runs at for an output rate, with this ratio.
    pub fn internal_rate(fs_out: f64, ratio: (usize, usize)) -> f64 {
        fs_out * ratio.0 as f64 / ratio.1 as f64
    }

    /// Fill `out` at the host's rate. Real-time safe: no allocation, any block length.
    pub fn render(&mut self, out: &mut [f32]) {
        let Some(rs) = &mut self.rs else {
            self.hv.render(out);
            return;
        };
        for chunk in out.chunks_mut(CHUNK) {
            let need = (rs.frac + chunk.len() * rs.m) / rs.l;
            self.hv.render(&mut rs.input[..need]);
            rs.run(chunk, need);
        }
    }
}

impl Deref for Voice {
    type Target = HumanVoice;
    fn deref(&self) -> &HumanVoice {
        &self.hv
    }
}

impl DerefMut for Voice {
    fn deref_mut(&mut self) -> &mut HumanVoice {
        &mut self.hv
    }
}

/// Rational up-sampler by l/m (m < l). Output sample n sits at input time n·m/l: with `frac` the
/// position past the newest input in 1/l steps, y = Σ_j h[j·l + frac] · x[newest − j].
struct Resampler {
    m: usize,
    l: usize,
    /// prototype low-pass at l × the input rate, l·TAPS long, gain l
    h: Vec<f32>,
    /// input history twice over (so the last TAPS samples are always contiguous)
    hist: Vec<f32>,
    pos: usize,
    frac: usize,
    input: Vec<f32>,
}

impl Resampler {
    fn new(m: usize, l: usize) -> Resampler {
        assert!(m < l && l <= 8);
        let n = TAPS * l;
        // cut-off at 0.44 × the input rate (14.5 kHz for 33.075 kHz in); with this length and
        // window the transition is ~0.14 × the input rate wide, so the images (from 0.5 × the input
        // rate up) are attenuated and the band below ~12 kHz is flat
        let fc = 0.44 / l as f64; // cycles per prototype sample
        let beta = 7.0;
        let i0 = |x: f64| -> f64 {
            let (mut s, mut t) = (1.0, 1.0);
            for k in 1..30 {
                t *= (x / 2.0) * (x / 2.0) / (k * k) as f64;
                s += t;
            }
            s
        };
        let mid = (n - 1) as f64 / 2.0;
        let mut h: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64 - mid;
                let sinc = if x == 0.0 { 2.0 * fc } else { (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x) };
                let r = x / mid;
                sinc * i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0(beta)
            })
            .collect();
        // each polyphase branch sums to exactly 1 (no ripple at DC between output phases)
        for p in 0..l {
            let s: f64 = (0..TAPS).map(|j| h[j * l + p]).sum();
            for j in 0..TAPS {
                h[j * l + p] /= s;
            }
        }
        Resampler { m, l, h: h.iter().map(|x| *x as f32).collect(), hist: vec![0.0; 2 * TAPS], pos: 0, frac: 0, input: vec![0.0; CHUNK] }
    }

    fn run(&mut self, out: &mut [f32], have: usize) {
        let mut k = 0;
        for o in out.iter_mut() {
            self.frac += self.m;
            while self.frac >= self.l {
                self.frac -= self.l;
                let x = self.input[k];
                k += 1;
                self.pos = if self.pos == 0 { TAPS - 1 } else { self.pos - 1 };
                self.hist[self.pos] = x;
                self.hist[self.pos + TAPS] = x;
            }
            // newest sample at hist[pos], older ones after it
            let x = &self.hist[self.pos..self.pos + TAPS];
            let mut acc = 0.0f32;
            let mut i = self.frac;
            for xv in x {
                acc += self.h[i] * xv;
                i += self.l;
            }
            *o = acc;
        }
        debug_assert_eq!(k, have);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sine below the internal Nyquist comes through at its level, without images, and the
    /// result doesn't depend on how the output is split into blocks.
    #[test]
    fn resampler_passes_the_band_and_is_block_independent() {
        let (m, l) = (3, 4);
        let fs_in = 33075.0;
        let f = 5000.0;
        let run = |blocks: &[usize]| -> Vec<f32> {
            let mut r = Resampler::new(m, l);
            let mut t = 0usize;
            let mut out = vec![0.0f32; blocks.iter().sum()];
            let mut at = 0;
            for &b in blocks {
                let need = (r.frac + b * m) / l;
                for i in 0..need {
                    r.input[i] = (2.0 * std::f64::consts::PI * f * (t + i) as f64 / fs_in).sin() as f32;
                }
                t += need;
                r.run(&mut out[at..at + b], need);
                at += b;
            }
            out
        };
        // (blocks up to CHUNK: `Voice::render` splits longer ones)
        let a = run(&[1000, 1000, 1000, 1000, 410]);
        let b = run(&[1, 63, 128, 512, 7, 1000, 1024, 999, 676]);
        assert_eq!(a, b);
        // level: RMS of the settled part ≈ 1/√2
        let tail = &a[200..];
        let rms = (tail.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        assert!((rms - 0.7071).abs() < 0.005, "rms {rms}");
        // no image: correlate with the expected output tone and check the residual is small
        let fs_out = 44100.0;
        let (mut c, mut s) = (0.0, 0.0);
        for (i, x) in tail.iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * f * (i + 200) as f64 / fs_out;
            c += *x as f64 * w.cos();
            s += *x as f64 * w.sin();
        }
        let amp = 2.0 * (c * c + s * s).sqrt() / tail.len() as f64;
        let resid = rms * rms - amp * amp / 2.0;
        assert!(resid < 1e-5, "residual power {resid} (amp {amp})");
    }
}
