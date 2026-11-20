//! The PLAY page's moving head (1.3.3): which pre-drawn pose of CHIMERA's anatomy is closest to the
//! articulators the voice is using right now.
//!
//! The original CHIMERA app streams the voice's 17 articulator values (the gestural score's output
//! after the articulators' own dynamics, the same values that shape the simulated airway) to the
//! screen and redraws the midsagittal anatomy from them (`HumanVocalTract::compute_new`) at 60 fps.
//! The MPC's skin can't draw lines; it can only show a frame of a pre-drawn film strip. So:
//!
//! - offline, `examples/anim_poses.rs` sings a corpus through this very instrument, records the
//!   visible articulators, clusters them into `anim_poses::K` poses per body (pose 0: at rest) and
//!   draws each with CHIMERA's anatomy (the skin's frames are made from that);
//! - live, the audio thread copies the visible articulators into an `ArticSnapshot` after each
//!   block (a few atomic stores, no lock, no allocation; only while the animation is on), and the
//!   planner thread, about 12 times a second, finds the nearest pose (`nearest`) and publishes
//!   its number. The plugin shows that frame (one host call per change, rate-limited there).
//!
//! What is not shown: the larynx height, epilarynx, ventricular folds, the lateral channel and
//! the dorsum (clicks): they are drawn at rest (the larynx would make the moving region twice as
//! tall). Between poses there is no blending (the skin can't): the nearest pose is shown.
use chimera::anatomy::human::{ArticKey, HumanArtic};
use std::sync::atomic::{AtomicU32, Ordering, fence};

/// The articulators the drawing shows, in this order everywhere (snapshot, pose tables).
pub const VIS: [ArticKey; N_VIS] = [
    ArticKey::Jaw,
    ArticKey::TbX,
    ArticKey::TbY,
    ArticKey::TtX,
    ArticKey::TtY,
    ArticKey::TtCurl,
    ArticKey::Root,
    ArticKey::LipAp,
    ArticKey::LipProt,
    ArticKey::LipWidth,
    ArticKey::Labiodental,
    ArticKey::Velum,
];
pub const N_VIS: usize = 12;

/// The visible articulators, written by the audio thread (one writer) and read by any thread: a
/// sequence lock over relaxed atomics, so the writer never waits and a reader never sees a torn
/// mixture (it retries, or gives up and keeps its last pose).
pub struct ArticSnapshot {
    seq: AtomicU32,
    v: [AtomicU32; N_VIS],
}

impl Default for ArticSnapshot {
    fn default() -> Self {
        ArticSnapshot { seq: AtomicU32::new(0), v: std::array::from_fn(|_| AtomicU32::new(0)) }
    }
}

impl ArticSnapshot {
    /// Audio thread only (the single writer): real-time safe, 14 atomic stores.
    pub fn write(&self, a: &HumanArtic) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for (slot, k) in self.v.iter().zip(VIS) {
            slot.store((a.get(k) as f32).to_bits(), Ordering::Relaxed);
        }
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// Any thread: the last complete snapshot (None before the first one, or if the writer kept
    /// overtaking this reader).
    pub fn read(&self) -> Option<[f32; N_VIS]> {
        for _ in 0..8 {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 == 0 || s0 & 1 == 1 {
                if s0 == 0 {
                    return None;
                }
                std::hint::spin_loop();
                continue;
            }
            let v: [f32; N_VIS] = std::array::from_fn(|i| f32::from_bits(self.v[i].load(Ordering::Relaxed)));
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s0 {
                return Some(v);
            }
        }
        None
    }
}

/// The pose (index into `poses`) nearest to `a`, each articulator's difference divided by
/// `scale` (its spread while singing, so a centimetre of jaw and a unit of lip spread compare).
pub fn nearest(poses: &[[f32; N_VIS]], scale: &[f32; N_VIS], a: &[f32; N_VIS]) -> usize {
    let mut best = (f32::INFINITY, 0usize);
    for (i, p) in poses.iter().enumerate() {
        let d: f32 = (0..N_VIS).map(|k| ((p[k] - a[k]) / scale[k]).powi(2)).sum();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// The pose for body `body` (0 male, 1 female, 2 child) nearest to `a`.
pub fn pose_for(body: usize, a: &[f32; N_VIS]) -> u32 {
    let b = body.min(crate::anim_poses::POSES.len() - 1);
    nearest(&crate::anim_poses::POSES[b], &crate::anim_poses::SCALE[b], a) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera::anatomy::human::NEUTRAL;

    #[test]
    fn snapshot_round_trips_and_starts_empty() {
        let s = ArticSnapshot::default();
        assert!(s.read().is_none());
        let mut a = NEUTRAL;
        a.jaw = 1.25;
        a.lip_ap = -0.1;
        s.write(&a);
        let v = s.read().unwrap();
        assert_eq!(v[0], 1.25);
        assert_eq!(v[7], -0.1);
        assert_eq!(v[1], NEUTRAL.tb_x as f32);
    }

    #[test]
    fn snapshot_is_never_torn_across_threads() {
        let s = std::sync::Arc::new(ArticSnapshot::default());
        let w = s.clone();
        let t = std::thread::spawn(move || {
            for i in 0..200_000u32 {
                let x = (i % 1000) as f64;
                let mut a = NEUTRAL;
                for k in VIS {
                    *a.get_mut(k) = x;
                }
                w.write(&a);
            }
        });
        let mut seen = 0;
        while !t.is_finished() {
            if let Some(v) = s.read() {
                assert!(v.iter().all(|x| *x == v[0]), "torn: {v:?}");
                seen += 1;
            }
        }
        t.join().unwrap();
        assert!(seen > 0);
    }

    #[test]
    fn nearest_picks_the_closest_scaled_pose() {
        let mut poses = [[0.0f32; N_VIS]; 3];
        poses[1][0] = 1.0;
        poses[2][7] = 1.0;
        let mut scale = [1.0f32; N_VIS];
        let mut a = [0.0f32; N_VIS];
        a[0] = 0.6;
        assert_eq!(nearest(&poses, &scale, &a), 1);
        a[0] = 0.4;
        assert_eq!(nearest(&poses, &scale, &a), 0);
        // a lip difference counts ten times more when lips vary ten times less: 0.06 of lip is
        // nearer the pose 0.1 away (0.04 → 0.16 scaled²) than the pose at 0 (0.06 → 0.36)
        poses[2][7] = 0.1;
        a = [0.0; N_VIS];
        a[7] = 0.06;
        assert_eq!(nearest(&poses, &scale, &a), 2, "unscaled, a lip difference of 0.06 is small");
        scale[7] = 0.1;
        assert_eq!(nearest(&poses, &scale, &a), 2);
        a[7] = 0.04;
        assert_eq!(nearest(&poses, &scale, &a), 0);
    }

    #[test]
    fn the_rest_pose_is_pose_zero_for_every_body() {
        let rest: [f32; N_VIS] = std::array::from_fn(|i| NEUTRAL.get(VIS[i]) as f32);
        for b in 0..3 {
            assert_eq!(crate::anim_poses::POSES[b][0], crate::anim_poses::POSES[b][0]);
            assert_eq!(pose_for(b, &crate::anim_poses::POSES[b][0]), 0, "body {b}");
            let _ = rest;
        }
    }
}
