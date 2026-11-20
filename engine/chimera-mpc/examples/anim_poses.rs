//! Makes the PLAY page's moving-head poses from CHIMERA's own singing (see `src/anim.rs`).
//!
//!     cargo run --release -p chimera-mpc --example anim_poses -- <plugin dir> <K> <outlines.json>
//!
//! For each body (male, female, child) it sings a corpus of words through this crate's own
//! `Instrument` (the plugin's planner and voice, Lite quality, offline), records the visible
//! articulators after every 128-sample block (the same snapshot the plugin uses live), and
//! clusters them (k-means, deterministic) into K poses: pose 0 is the voice at rest (fixed), the
//! others the most-visited places of real articulation, transitions included. It writes
//! `src/anim_poses.rs` (the table the plugin matches against) and, to <outlines.json>, each pose
//! drawn by CHIMERA's anatomy (the skin's frames are rendered from it by
//! mpc-plugin-builder-sdk/plugins/Chimera Voice/skin/make_assets.py). Not drawn: larynx height
//! and the other articulators outside `anim::VIS` (at rest).
use chimera::anatomy::human::{BodyName, HumanArtic, HumanVocalTract, NEUTRAL};
use chimera_mpc::anim::{N_VIS, VIS};
use chimera_mpc::instrument::{DEFAULTS, Instrument, N_PARAMS, P_QUALITY, P_VOICE};
use std::fmt::Write as _;

#[path = "common/outline.rs"]
mod outline;

const FS: f64 = 44100.0;
const BLOCK: usize = 128;
const CORPUS: &str = "Hello there, I am a voice made of air. Sing to me, oh sweet melody. Blue moon, you saw me standing alone. \
Figaro, Figaro, la di da. Peter Piper picked a peck of pickled peppers. She sells sea shells by the shore. \
The quick brown fox jumps over the lazy dog. Ooh, aah, eee, oh, ay. Mama, papa, nana, wawa, yaya, lala, rara. \
Thank you very much, good night, my love, forever and ever.";

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The corpus sung by body `voice`: (samples after every block, the rest pose).
fn record(dir: &std::path::Path, voice: usize) -> (Vec<[f32; N_VIS]>, [f32; N_VIS]) {
    let mut p = DEFAULTS;
    p[P_VOICE] = voice as f64;
    p[P_QUALITY] = 3.0; // Lite, as on the MPC
    let p: [f64; N_PARAMS] = p;
    let inst = Instrument::new(FS, dir.to_path_buf(), &p, false);
    inst.set_typed(CORPUS);
    inst.settle();
    inst.set_animation(true);
    let mut buf = vec![0.0f32; BLOCK];
    let mut run = |inst: &Instrument, sec: f64, out: &mut Vec<[f32; N_VIS]>| {
        for _ in 0..(sec * FS / BLOCK as f64) as usize {
            inst.render(&mut buf);
            inst.pump();
            if let Some(v) = inst.artic_now() {
                out.push(v);
            }
        }
    };
    let mut rest = Vec::new();
    run(&inst, 0.6, &mut rest);
    let rest = *rest.last().expect("a snapshot");
    let mut samples = Vec::new();
    let base = [48u8, 60, 64][voice];
    let melody = [0u8, 2, 4, 5, 7, 5, 4, 2, 0, 7, 9, 7];
    let durs = [0.45, 0.3, 0.6, 0.25, 0.5, 0.35];
    // about twice through the corpus' syllables, with gaps (codas, rest) and some legato
    for i in 0..170usize {
        let n = base + melody[i % melody.len()];
        inst.midi(0x90, n, 96);
        inst.pump();
        let d = durs[i % durs.len()];
        if i % 5 == 4 {
            // legato: the next note starts before this one ends
            run(&inst, d, &mut samples);
            let n2 = base + melody[(i + 1) % melody.len()];
            inst.midi(0x90, n2, 96);
            inst.pump();
            inst.midi(0x80, n, 0);
            inst.pump();
            run(&inst, 0.3, &mut samples);
            inst.midi(0x80, n2, 0);
            inst.pump();
        } else {
            run(&inst, d, &mut samples);
            inst.midi(0x80, n, 0);
            inst.pump();
        }
        run(&inst, if i % 7 == 6 { 0.5 } else { 0.12 }, &mut samples);
    }
    (samples, rest)
}

fn dist(a: &[f32; N_VIS], b: &[f32; N_VIS], s: &[f32; N_VIS]) -> f32 {
    (0..N_VIS).map(|k| ((a[k] - b[k]) / s[k]).powi(2)).sum()
}

/// k-means with centroid 0 fixed at `rest`: the centroids, and per centroid its share of samples.
fn cluster(x: &[[f32; N_VIS]], rest: [f32; N_VIS], scale: &[f32; N_VIS], k: usize) -> (Vec<[f32; N_VIS]>, Vec<f64>, f64) {
    let mut rng = Lcg(0x5EED_C41A);
    let mut c = vec![rest];
    // k-means++
    while c.len() < k {
        let d: Vec<f32> = x.iter().map(|p| c.iter().map(|q| dist(p, q, scale)).fold(f32::INFINITY, f32::min)).collect();
        let total: f64 = d.iter().map(|v| *v as f64).sum();
        let mut r = rng.next() * total;
        let mut pick = x.len() - 1;
        for (i, v) in d.iter().enumerate() {
            r -= *v as f64;
            if r <= 0.0 {
                pick = i;
                break;
            }
        }
        c.push(x[pick]);
    }
    let mut assign = vec![0usize; x.len()];
    for _ in 0..60 {
        for (i, p) in x.iter().enumerate() {
            assign[i] = chimera_mpc::anim::nearest(&c, scale, p);
        }
        let mut sum = vec![[0.0f64; N_VIS]; k];
        let mut cnt = vec![0usize; k];
        for (i, p) in x.iter().enumerate() {
            cnt[assign[i]] += 1;
            for d in 0..N_VIS {
                sum[assign[i]][d] += p[d] as f64;
            }
        }
        for j in 1..k {
            if cnt[j] > 0 {
                c[j] = std::array::from_fn(|d| (sum[j][d] / cnt[j] as f64) as f32);
            } else {
                // an empty cluster: move it to the sample worst served
                let far = (0..x.len()).max_by(|&a, &b| dist(&x[a], &c[assign[a]], scale).total_cmp(&dist(&x[b], &c[assign[b]], scale))).unwrap();
                c[j] = x[far];
            }
        }
    }
    let mut cnt = vec![0usize; k];
    let mut err = 0.0f64;
    for p in x {
        let j = chimera_mpc::anim::nearest(&c, scale, p);
        cnt[j] += 1;
        err += (dist(p, &c[j], scale) as f64).sqrt();
    }
    let share = cnt.iter().map(|n| *n as f64 / x.len() as f64).collect();
    (c, share, err / x.len() as f64)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(args.get(1).expect("plugin dir (with Content/cmudict.txt.gz)"));
    let k: usize = args.get(2).map(|s| s.parse().expect("K")).unwrap_or(24);
    let json_path = args.get(3).expect("outlines.json");
    let bodies = [(BodyName::Male, "male"), (BodyName::Female, "female"), (BodyName::Child, "child")];
    let mut table = String::new();
    let mut scales = String::new();
    let mut bjson = Vec::new();
    for (bi, (b, bname)) in bodies.iter().enumerate() {
        let (x, rest) = record(&dir, bi);
        // each articulator's spread while singing (floored: a nearly still one mustn't dominate)
        let floor = [0.12f32, 0.12, 0.12, 0.12, 0.12, 0.08, 0.12, 0.05, 0.05, 0.08, 0.08, 0.05];
        let scale: [f32; N_VIS] = std::array::from_fn(|d| {
            let m = x.iter().map(|p| p[d] as f64).sum::<f64>() / x.len() as f64;
            let v = x.iter().map(|p| (p[d] as f64 - m).powi(2)).sum::<f64>() / x.len() as f64;
            (v.sqrt() as f32).max(floor[d])
        });
        let (mut c, share, err) = cluster(&x, rest, &scale, k);
        // poses 1.. ordered by jaw opening (frame order is cosmetic)
        let mut order: Vec<usize> = (1..k).collect();
        order.sort_by(|a, b| c[*a][0].total_cmp(&c[*b][0]));
        let mut sorted = vec![c[0]];
        let mut sh = vec![share[0]];
        for i in order {
            sorted.push(c[i]);
            sh.push(share[i]);
        }
        c = sorted;
        eprintln!("{bname}: {} samples ({:.1} s), {k} poses, mean scaled error {err:.3}, rest share {:.0}%, smallest pose {:.1}%",
                  x.len(), x.len() as f64 * BLOCK as f64 / FS, 100.0 * sh[0], 100.0 * sh.iter().cloned().fold(1.0, f64::min));
        let _ = writeln!(table, "    // {bname}: shares of the corpus' blocks {:?}", sh.iter().map(|s| (s * 1000.0).round() / 10.0).collect::<Vec<_>>());
        table.push_str("    [\n");
        for p in &c {
            let _ = writeln!(table, "        [{}],", p.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(", "));
        }
        table.push_str("    ],\n");
        let _ = writeln!(scales, "    [{}],", scale.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(", "));
        // drawings
        let body = b.body();
        let mut t = HumanVocalTract::new(body.clone());
        let mut pj = Vec::new();
        for (i, p) in c.iter().enumerate() {
            let mut a: HumanArtic = NEUTRAL;
            for (d, key) in VIS.iter().enumerate() {
                *a.get_mut(*key) = p[d] as f64;
            }
            let (lines, length) = outline::head_lines(&mut t, body.pharynx, &a);
            pj.push(format!("{{\"name\":\"p{i}\",\"length\":{length:.2},\"artic\":[{}],\"lines\":[{}]}}",
                            p.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(","), lines.join(",")));
        }
        bjson.push(format!("{{\"name\":\"{bname}\",\"poses\":[{}]}}", pj.join(",")));
    }
    let src = format!(
        "//! Generated by `cargo run --release -p chimera-mpc --example anim_poses` (do not edit): the\n\
         //! anatomy poses of the PLAY page's head, per body (male, female, child), in `anim::VIS` order.\n\
         //! Pose 0 is the voice at rest; the others are k-means centres of the articulators recorded\n\
         //! while this instrument sang a corpus (Lite), ordered by jaw opening. SCALE: each\n\
         //! articulator's spread in that recording (floored), the distance's units.\n\
         pub const K: usize = {k};\n\
         pub const POSES: [[[f32; crate::anim::N_VIS]; K]; 3] = [\n{table}];\n\
         pub const SCALE: [[f32; crate::anim::N_VIS]; 3] = [\n{scales}];\n");
    let out = concat!(env!("CARGO_MANIFEST_DIR"), "/src/anim_poses.rs");
    std::fs::write(out, src).expect("write anim_poses.rs");
    std::fs::write(json_path, format!("{{\"k\":{k},\"bodies\":[{}]}}", bjson.join(","))).expect("write outlines");
    eprintln!("wrote {out} and {json_path}");
}
