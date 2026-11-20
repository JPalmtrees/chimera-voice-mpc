//! Sing mode's live path, ported from the web app's `src/ui/singing.ts` (singNoteOn/singNoteOff):
//! the lyrics are syllables with a cursor and each key sings the next syllable at its pitch. A key
//! pressed while the previous one is still down sings legato (the previous syllable's coda runs
//! straight into the next onset); a "-" in the lyrics holds the vowel onto a new pitch (melisma).
//! Releasing the sounding key while another is still down glides back to that key on the same
//! vowel. Key velocity sets the breath.
//!
//! It plans with the engine's own planners (`plan_note_on`, `plan_note_off`, `plan_melisma`), which
//! allocate, so it runs off the audio thread and hands the voice commands through a [`Sink`].
use chimera::anatomy::human::BodyMix;
use chimera::control::score::{ControlKey, Gesture};
use chimera::phonetics::Syllable;
use chimera::phonetics::planner::EXHALE_LABEL;
use chimera::phonetics::sing::{LegatoFrom, MelismaOptions, NoteOnOptions, plan_melisma, plan_note_off, plan_note_on};

/// The scheduling lead the web app's live keys use (s).
pub const LIVE_LEAD: f64 = 0.006;

/// Where the singer's commands go (the voice, in order).
pub trait Sink {
    /// End the open-ended gestures with this tag now.
    fn release(&mut self, tag: f64);
    /// Schedule gestures planned relative to "now", shifted by the voice clock + `lead`.
    fn schedule(&mut self, gestures: &[Gesture], lead: f64);
}

/// Sung when the lyrics are empty.
pub fn ah() -> Syllable {
    serde_json::from_str(r#"{"text":"ah","onset":[],"nucleus":{"ipa":"ɑ","stress":1},"coda":[],"wordEnd":true,"phraseEnd":false}"#).expect("the 'ah' syllable")
}

/// Breath level for a key velocity (1 at the web keys' default of 96).
pub fn breath_for(velocity: f64) -> f64 {
    0.62 + 0.4 * (velocity / 96.0).clamp(0.0, 1.0) + 0.12 * ((velocity - 96.0) / 31.0).max(0.0)
}

fn is_melisma(s: &Syllable) -> bool {
    s.melisma == Some(true)
}

struct Sounding {
    midi: f64,
    /// the syllable whose vowel is sounding (a melisma keeps its parent's, for the coda)
    syl: Syllable,
    tag: f64,
}

pub struct Singer {
    pub syllables: Vec<Syllable>,
    /// the syllable the next key sings
    pub cursor: usize,
    pub body: BodyMix,
    /// vibrato depth on held vowels (cents)
    pub vibrato: f64,
    /// 0: every key breathes as at velocity 96; 1: the web app's velocity → breath
    pub velocity_sens: f64,
    sounding: Option<Sounding>,
    /// the index of the syllable the sounding note sang (its parent, for a melisma mark): what the
    /// screen highlights while a note sounds
    sung: Option<usize>,
    /// keys down, oldest first
    keys: Vec<u8>,
    next_tag: f64,
    /// endings (glide, coda) that may still be playing: tag, and tag + 1 for the breath-out
    tails: Vec<f64>,
    exhales_live: bool,
    fallback: Syllable,
}

impl Singer {
    pub fn new(body: BodyMix) -> Self {
        Singer { syllables: Vec::new(), cursor: 0, body, vibrato: 35.0, velocity_sens: 1.0, sounding: None, sung: None, keys: Vec::with_capacity(128), next_tag: 2.0, tails: Vec::new(), exhales_live: false, fallback: ah() }
    }

    /// New lyrics; the cursor goes back to `start` (clamped).
    pub fn set_syllables(&mut self, syls: Vec<Syllable>, start: usize) {
        self.syllables = syls;
        self.seek(start);
    }

    pub fn seek(&mut self, i: usize) {
        self.cursor = if self.syllables.is_empty() { 0 } else { i.min(self.syllables.len() - 1) };
        // the lyrics moved: what sounds now (if anything) is no longer at a place in them
        self.sung = None;
    }

    /// What the screen shows: the syllable sounding now (index, true) while a note sings it, else
    /// the one the next key will sing (index, false). From the singer's own state, never a guess.
    pub fn focus(&self) -> (usize, bool) {
        match (self.sung, &self.sounding) {
            (Some(i), Some(_)) if i < self.syllables.len() => (i, true),
            _ => (self.cursor, false),
        }
    }

    /// Syllable `i`'s text, with a hyphen when its word goes on (the fallback "ah" when there are
    /// no lyrics).
    pub fn text_at(&self, i: usize) -> String {
        match self.syllables.get(i) {
            Some(s) => s.text.clone() + if s.word_end { "" } else { "-" },
            None => self.fallback.text.clone(),
        }
    }

    /// The syllable the next key sings (its text, with a hyphen when its word goes on).
    pub fn next_text(&self) -> String {
        match self.syllables.get(self.cursor) {
            Some(s) => s.text.clone() + if s.word_end { "" } else { "-" },
            None => self.fallback.text.clone(),
        }
    }

    /// The head drawing's mouth for what is sounding: 0 at rest (no note), else the vowel's nearest
    /// of the drawn poses: 1 a, 2 e, 3 i, 4 o, 5 u (by the nucleus's first IPA letter).
    pub fn pose(&self) -> u32 {
        let Some(s) = &self.sounding else { return 0 };
        match s.syl.nucleus.ipa.chars().next() {
            Some('i' | 'ɪ' | 'y' | 'j') => 3,
            Some('u' | 'ʊ' | 'w' | 'ɯ') => 5,
            Some('o' | 'ɔ' | 'ø' | 'ɤ') => 4,
            Some('a' | 'ɑ' | 'æ' | 'ʌ' | 'ɒ' | 'ɐ') => 1,
            _ => 2,
        }
    }

    /// The words around the next syllable, about `before_chars` characters before it (ending in a
    /// space at a word boundary) and `after_chars` after it, in whole syllables. Melisma marks are
    /// left out.
    pub fn context(&self, before_chars: usize, after_chars: usize) -> (String, String) {
        let (b, a, _) = self.context_at(self.cursor, before_chars, after_chars, 0);
        (b, a)
    }

    /// The words around syllable `at`: about `before_chars` before it, `after_chars` after it, and
    /// about `more_chars` following those (the next line of the lyrics), in whole syllables.
    pub fn context_at(&self, at: usize, before_chars: usize, after_chars: usize, more_chars: usize) -> (String, String, String) {
        let syl = |s: &Syllable| -> Option<String> {
            (!is_melisma(s)).then(|| s.text.clone() + if s.word_end { " " } else { "" })
        };
        let mut before = String::new();
        for s in self.syllables[..at.min(self.syllables.len())].iter().rev() {
            if before.chars().count() >= before_chars {
                break;
            }
            if let Some(t) = syl(s) {
                before.insert_str(0, &t);
            }
        }
        let mut after = String::new();
        if let Some(cur) = self.syllables.get(at) {
            if cur.word_end {
                after.push(' ');
            }
        }
        let mut more = String::new();
        for s in self.syllables.iter().skip(at + 1) {
            let Some(t) = syl(s) else { continue };
            // the after part ends at a word's end once it is long enough; the rest goes on below
            if after.chars().count() < after_chars || !after.ends_with(' ') && more.is_empty() {
                after.push_str(&t);
            } else if more.chars().count() < more_chars {
                more.push_str(&t);
            } else {
                break;
            }
        }
        (before, after.trim_end().to_string(), more.trim_end().to_string())
    }

    /// The voice was rebuilt (body or quality changed): nothing of ours sounds there any more.
    pub fn voice_replaced(&mut self, body: BodyMix) {
        self.body = body;
        self.sounding = None;
        self.sung = None;
        self.tails.clear();
        self.exhales_live = false;
        self.next_tag = 2.0;
    }

    /// The parent a melisma at index i continues.
    fn parent_of(&self, i: usize) -> Syllable {
        for k in (0..=i).rev() {
            if !is_melisma(&self.syllables[k]) {
                return self.syllables[k].clone();
            }
        }
        self.fallback.clone()
    }

    fn breath(&self, velocity: f64) -> f64 {
        1.0 + self.velocity_sens * (breath_for(velocity) - 1.0)
    }

    fn start_note(&mut self, syl: Syllable, midi: f64, velocity: f64, out: &mut impl Sink) {
        let tag = self.next_tag;
        self.next_tag += 2.0;
        // earlier codas may finish, but their breath-out must not open the folds on this onset
        if self.exhales_live {
            for t in &self.tails {
                out.release(t + 1.0);
            }
        }
        self.exhales_live = false;
        let from = self.sounding.take();
        if let Some(f) = &from {
            out.release(f.tag);
            out.release(f.tag + 1.0);
        }
        let legato_from = from.map(|f| LegatoFrom { syl: f.syl, midi: f.midi });
        let mut gs = plan_note_on(&syl, midi, &NoteOnOptions { body: self.body, vibrato: self.vibrato, tag, legato_from });
        let breath = self.breath(velocity);
        for g in gs.iter_mut() {
            if let Some(b) = g.targets.get(ControlKey::BREATH) {
                g.targets.set(ControlKey::BREATH, b * breath);
            }
        }
        out.schedule(&gs, LIVE_LEAD);
        self.sounding = Some(Sounding { midi, syl, tag });
    }

    /// The held vowel moves to a new pitch (melisma, or back to a key still down).
    fn glide(&mut self, midi: f64, out: &mut impl Sink) {
        let vibrato = self.vibrato;
        let s = self.sounding.as_mut().expect("a sounding note");
        out.release(s.tag + 1.0);
        out.schedule(&plan_melisma(midi, &MelismaOptions { vibrato, tag: s.tag }), LIVE_LEAD);
        s.midi = midi;
    }

    pub fn note_on(&mut self, note: u8, velocity: u8, out: &mut impl Sink) {
        self.keys.retain(|&k| k != note);
        self.keys.push(note);
        let midi = note as f64;
        let n = self.syllables.len();
        let (i, syl) = if n == 0 { (0, self.fallback.clone()) } else { (self.cursor % n, self.syllables[self.cursor % n].clone()) };
        // the syllable sung now: a melisma mark carries its parent's vowel on
        let mut sung = i;
        while n > 0 && sung > 0 && is_melisma(&self.syllables[sung]) {
            sung -= 1;
        }
        if is_melisma(&syl) && self.sounding.is_some() {
            self.glide(midi, out);
        } else if is_melisma(&syl) {
            // a melisma with nothing held: sing its vowel again, without the onset
            let mut p = self.parent_of(i);
            p.onset.clear();
            self.start_note(p, midi, velocity as f64, out);
        } else {
            self.start_note(syl, midi, velocity as f64, out);
        }
        if n > 0 {
            self.cursor = (i + 1) % n;
            self.sung = Some(sung);
        }
    }

    pub fn note_off(&mut self, note: u8, out: &mut impl Sink) {
        self.keys.retain(|&k| k != note);
        let midi = note as f64;
        match &self.sounding {
            Some(s) if s.midi == midi => {}
            _ => return,
        }
        if let Some(&k) = self.keys.last() {
            self.glide(k as f64, out);
            return;
        }
        let s = self.sounding.take().unwrap();
        self.sung = None;
        out.release(s.tag);
        out.release(s.tag + 1.0);
        // a melisma comes next: this vowel carries on there, and the coda waits for the chain's end
        if self.syllables.get(self.cursor).is_some_and(is_melisma) {
            return;
        }
        let mut gs = plan_note_off(&s.syl, midi, self.body);
        if gs.is_empty() {
            return;
        }
        let tag = self.next_tag;
        self.next_tag += 2.0;
        for g in gs.iter_mut() {
            *g = g.tag(if g.label_str() == EXHALE_LABEL { tag + 1.0 } else { tag });
        }
        out.schedule(&gs, LIVE_LEAD);
        self.tails.push(tag);
        if self.tails.len() > 8 {
            self.tails.remove(0); // long finished by now
        }
        self.exhales_live = true;
    }

    /// Every key up (all-notes-off, transport stop): end the sounding note with its coda.
    pub fn all_off(&mut self, out: &mut impl Sink) {
        let keys = std::mem::take(&mut self.keys);
        if let Some(s) = &self.sounding {
            let n = s.midi as u8;
            self.keys.push(n);
            self.note_off(n, out);
        }
        drop(keys);
        self.keys.clear();
    }
}
