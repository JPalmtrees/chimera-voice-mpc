//! The head drawing's outlines for one articulation, in body cm (shared by `head_outlines` and
//! `anim_poses`): CHIMERA's `HumanVocalTract` for the airway, tongue, lips, teeth, velum, jaw and
//! epiglottis; the face, cranium and nose after the web app's `src/ui/render/profile.ts`.
use chimera::anatomy::geom::Vec2;
use chimera::anatomy::human::{HumanArtic, HumanVocalTract};

fn rot(p: Vec2, c: Vec2, phi: f64) -> Vec2 {
    let (s, co) = phi.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    Vec2 { x: c.x + dx * co - dy * s, y: c.y + dx * s + dy * co }
}

fn line(out: &mut Vec<String>, kind: &str, smooth: bool, closed: bool, pts: &[Vec2]) {
    let p: Vec<String> = pts.iter().map(|v| format!("[{:.4},{:.4}]", v.x, v.y)).collect();
    out.push(format!("{{\"kind\":\"{kind}\",\"smooth\":{smooth},\"closed\":{closed},\"pts\":[{}]}}", p.join(",")));
}

/// The outlines (JSON objects) of `t`'s body (pharynx factor `ph`) articulated as `a`, and the
/// tract length (cm).
pub fn head_lines(t: &mut HumanVocalTract, ph: f64, a: &HumanArtic) -> (Vec<String>, f64) {
    let k = t.scale();
    let bp = |x: f64, y: f64| Vec2 { x: x * k, y: (if y < 7.0 { 7.0 - (7.0 - y) * ph } else { y }) * k };
    let f = t.compute_new(&a);
    let condyle = t.p(-2.5, 10.5);
    let far = t.p(6.95, 7.75);
    let d = ((far.x - condyle.x).powi(2) + (far.y - condyle.y).powi(2)).sqrt();
    let phi = -a.jaw.clamp(-0.2, 3.0) * k / d;
    let j = |x: f64, y: f64| rot(t.p(x, y), condyle, phi);
    let mut lines = Vec::new();
    let ul: Vec<Vec2> = f.upper_lip.iter().skip(7).copied().collect();
    let ll: Vec<Vec2> = f.lower_lip.iter().skip(7).copied().collect();
    // skin: neck front → chin (jaw-carried) → lower lip; upper lip → nose → forehead → cranium → nape
    let mut lower = vec![bp(2.3, -4.6), bp(2.38, -1.5), bp(2.52, 0.4), bp(3.0, 1.7), j(5.3, 3.0), j(7.25, 3.35), j(7.95, 4.6), j(7.55, 5.7)];
    if let Some(e) = ll.last() {
        lower.push(*e);
    }
    line(&mut lines, "skin", true, false, &lower);
    line(&mut lines, "lip", false, false, &ll);
    line(&mut lines, "lip", false, false, &ul);
    let mut face = vec![*ul.last().unwrap()];
    face.extend([bp(7.05, 10.6), bp(7.75, 10.78), bp(8.75, 11.05), bp(9.25, 11.65), bp(8.95, 12.6), bp(8.35, 13.9), bp(7.7, 15.0), bp(7.95, 16.1), bp(7.9, 18.1), bp(6.9, 20.5)]);
    line(&mut lines, "skin", true, false, &face);
    line(&mut lines, "skin", true, false, &[bp(6.9, 20.5), bp(3.0, 22.9), bp(-2.2, 22.7), bp(-6.9, 19.6), bp(-8.7, 15.4), bp(-8.3, 11.9), bp(-7.4, 8.9), bp(-6.8, 4.8), bp(-6.6, 0.0), bp(-6.6, -4.6)]);
    line(&mut lines, "nasal", true, true, &[bp(0.3, 11.2), bp(1.1, 10.25), bp(2.7, 10.75), bp(4.6, 10.85), bp(6.4, 10.65), bp(7.6, 10.85), bp(8.45, 11.3), bp(8.0, 12.2), bp(6.7, 13.4), bp(4.4, 13.9), bp(1.8, 13.6), bp(0.2, 12.6)]);
    line(&mut lines, "brain", true, false, &[bp(6.55, 16.3), bp(6.75, 18.4), bp(5.5, 20.1), bp(2.8, 21.4), bp(-1.9, 21.3), bp(-5.8, 18.8), bp(-7.1, 15.4), bp(-6.7, 13.1), bp(-4.4, 12.8), bp(-2.1, 13.9), bp(1.0, 14.3), bp(4.0, 14.7), bp(5.9, 15.4)]);
    let eye = bp(6.5, 15.4);
    let ring: Vec<Vec2> = (0..=24).map(|i| {
        let an = i as f64 / 24.0 * std::f64::consts::TAU;
        Vec2 { x: eye.x + an.cos() * 1.1 * k, y: eye.y + an.sin() * 0.95 * k }
    }).collect();
    line(&mut lines, "eye", false, true, &ring);
    // the airway itself
    let g = f.glottis;
    let outer: Vec<Vec2> = f.outer.iter().copied().filter(|p| p.y > g.y || p.x > 0.0).collect();
    line(&mut lines, "wall", false, false, &outer);
    line(&mut lines, "wall", false, false, &f.inner);
    line(&mut lines, "tongue", false, true, &f.tongue);
    line(&mut lines, "velum", false, false, &f.velum);
    line(&mut lines, "teeth", false, true, &f.upper_teeth);
    line(&mut lines, "teeth", false, true, &f.lower_teeth);
    line(&mut lines, "epiglottis", false, false, &f.epiglottis);
    let jb: Vec<Vec2> = f.jaw_bone.iter().copied().filter(|p| p.x > 0.8 * k).collect();
    line(&mut lines, "jaw", false, false, &jb);
    // vertebrae (as profile.ts: rounded boxes behind the pharyngeal wall)
    for i in 0..9 {
        let y0 = -4.4 + i as f64 * 1.75;
        if y0 > 10.5 {
            break;
        }
        let x0 = -1.2 - 0.025 * y0 - 2.25;
        let (p0, p1) = (t.p(x0, y0 + 1.4), t.p(x0 + 1.6, y0));
        line(&mut lines, "spine", false, true, &[Vec2 { x: p0.x, y: p0.y }, Vec2 { x: p1.x, y: p0.y }, Vec2 { x: p1.x, y: p1.y }, Vec2 { x: p0.x, y: p1.y }]);
    }
    (lines, f.length)
}
