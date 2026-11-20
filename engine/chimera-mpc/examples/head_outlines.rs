//! Dumps the midsagittal head outlines the MPC skin's head graphic is drawn from, computed by
//! CHIMERA's own anatomy (`HumanVocalTract`), for each body and a few vowel poses. The face,
//! cranium and nose follow the web app's `src/ui/render/profile.ts` (same control points).
//!
//!     cargo run --release -p chimera-mpc --example head_outlines > outlines.json
//!
//! Output: {"bodies": [{"name", "poses": [{"name", "lines": [{"kind", "smooth", "closed", "pts"}]}]}]},
//! in body cm (x forward, y up, glottis near the origin). `smooth` lines are Catmull-Rom control points.
use chimera::anatomy::human::{BodyMix, BodyName, HumanArtic, HumanVocalTract, NEUTRAL};
use chimera::phonetics::planner::vowel_artic;

#[path = "common/outline.rs"]
mod outline;

fn main() {
    let bodies = [(BodyName::Male, "male"), (BodyName::Female, "female"), (BodyName::Child, "child")];
    let poses: [(&str, Option<&str>); 6] = [("rest", None), ("a", Some("ɑ")), ("e", Some("ɛ")), ("i", Some("i")), ("o", Some("o")), ("u", Some("u"))];
    let mut bout = Vec::new();
    for (b, bname) in bodies {
        let body = b.body();
        let mix = match b {
            BodyName::Male => BodyMix::new(0.0, 0.0),
            BodyName::Female => BodyMix::new(1.0, 0.0),
            BodyName::Child => BodyMix::new(0.0, 1.0),
        };
        let mut t = HumanVocalTract::new(body.clone());
        let ph = body.pharynx;
        let mut pout = Vec::new();
        for (pname, ipa) in poses {
            let a: HumanArtic = match ipa {
                None => NEUTRAL,
                Some(v) => vowel_artic(v, mix).expect("vowel").apply_to(&NEUTRAL),
            };
            let (lines, length) = outline::head_lines(&mut t, ph, &a);
            pout.push(format!("{{\"name\":\"{pname}\",\"length\":{:.2},\"lines\":[{}]}}", length, lines.join(",")));
        }
        bout.push(format!("{{\"name\":\"{bname}\",\"poses\":[{}]}}", pout.join(",")));
    }
    println!("{{\"bodies\":[{}]}}", bout.join(","));
}
