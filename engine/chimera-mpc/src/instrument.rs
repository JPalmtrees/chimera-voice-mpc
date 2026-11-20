//! The instrument: CHIMERA's human voice singing lyrics from MIDI notes, split across two threads.
//!
//! - The **audio thread** owns the voice. It renders, applies the voice commands the planner sends
//!   (gestures, releases, a rebuilt voice) and the knob settings that need no planning (source,
//!   pitch, volume), and forwards MIDI to the planner. It never allocates, locks or does I/O.
//! - The **planner thread** ([`Worker`]) runs the live singing controller ([`Singer`]), the
//!   engine's planners, lyrics loading (file I/O, G2P) and voice rebuilds (body or quality
//!   changes), all of which allocate. Retired voices are dropped there too.
//!
//! They talk through two wait-free queues ([`Ring`]); knob values are atomics any thread may set.
use crate::lyrics::{self, LyricSet};
use crate::ring::Ring;
use crate::singer::{Singer, Sink};
use chimera::anatomy::human::{ArticKey, ArticPatch, BodyMix};
use chimera::control::score::Gesture;
use chimera::engine::default_f0;
use chimera::phonetics::Syllable;
use chimera::phonetics::english::EnglishG2P;
use chimera::phonetics::mandarin::MandarinLexicon;
use chimera::phonetics::syllables::lyrics_to_syllables_with;
use crate::voice::Voice;
use chimera::voice::human_voice::{HumanVoice, SourcePatch};
use std::cell::UnsafeCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

// Parameter indices: the plugin's parameter order (permanent once released; append only).
/// How often the planner picks the head's pose (1.3.3): 12.5 times a second.
const ANIM_PERIOD: Duration = Duration::from_millis(80);

pub const P_VOLUME: usize = 0;
pub const P_VOICE: usize = 1;
pub const P_TRANSPOSE: usize = 2;
pub const P_FINE: usize = 3;
pub const P_BEND_RANGE: usize = 4;
pub const P_BREATH: usize = 5;
pub const P_BREATHY: usize = 6;
pub const P_VIB_DEPTH: usize = 7;
pub const P_VIB_RATE: usize = 8;
pub const P_PRESSED: usize = 9;
pub const P_BRIGHT: usize = 10;
pub const P_ROUGH: usize = 11;
pub const P_RING: usize = 12;
pub const P_LYRIC_SET: usize = 13;
pub const P_START: usize = 14;
pub const P_RESTART: usize = 15;
pub const P_PLAY_SYNC: usize = 16;
pub const P_VEL_SENS: usize = 17;
pub const P_QUALITY: usize = 18;
/// lyric source: 0 typed on the touchscreen (when there are any), 1 lyrics.txt
pub const P_SOURCE: usize = 19;
pub const N_PARAMS: usize = 20;

/// Defaults (plain values), in parameter order. Source defaults are the engine's `DEFAULT_SOURCE`
/// and the web app's Sing panel.
pub const DEFAULTS: [f64; N_PARAMS] = [0.0, 0.0, 0.0, 0.0, 2.0, 7.2, 0.15, 35.0, 6.0, 0.5, 0.25, 0.1, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, DEFAULT_QUALITY, 0.0];

/// Quality choice → (airway oversampling, control interval in the voice's samples, fast
/// arithmetic, voice rate as a fraction m/l of the host's).
/// - Eco: the airway simulated at 44.1 kHz, articulation updated every 2.9 ms, fast arithmetic.
/// - Std: the same at the engine's own control rate (every 1.45 ms).
/// - High: as the web app renders at 44.1 kHz (airway at 88.2 kHz), exact arithmetic.
/// - Lite (appended, so saved projects keep their choice): the airway at 3/4 of the host's rate
///   (33.075 kHz), articulation every 5.8 ms, resampled up (see `voice`); for the MPC One.
pub const QUALITY: [(usize, usize, bool, (usize, usize)); 4] = [(1, 128, true, (1, 1)), (1, 64, true, (1, 1)), (2, 64, false, (1, 1)), (1, 192, true, (3, 4))];
pub const QUALITY_NAMES: [&str; 4] = ["Eco", "Std", "High", "Lite"];
pub const Q_LITE: usize = 3;
pub const DEFAULT_QUALITY: f64 = 0.0;

pub const BODIES: [BodyMix; 3] = [BodyMix::MALE, BodyMix::FEMALE, BodyMix::CHILD];

/// Gestures one schedule command takes (the engine's `MAX_SCHEDULE`).
const MAX_SCHEDULE: usize = chimera::engine::MAX_SCHEDULE;

#[derive(Clone, Copy)]
enum Event {
    NoteOn(u8, u8),
    NoteOff(u8),
    AllOff,
    /// the host's transport started playing
    PlayStarted,
    /// a voice the audio thread no longer uses, to be dropped here
    Retire(*mut Voice),
}
unsafe impl Send for Event {}

#[derive(Clone, Copy)]
enum Cmd {
    Gesture(Gesture),
    /// schedule the gestures sent since the last commit, `lead` s after the voice clock
    Commit(f64),
    Release(f64),
    /// a new voice (owned: a leaked Box), and the index of its body
    Voice(*mut Voice),
}
unsafe impl Send for Cmd {}

/// What the screen shows (read from the UI thread, written by the planner).
#[derive(Default, Clone, Debug)]
pub struct Display {
    pub titles: Vec<String>,
    pub next: String,
    pub position: usize,
    pub count: usize,
    pub status: String,
    /// the touchscreen editor's status line, and its draft as lines around the cursor (each fits
    /// the host's 31-byte display strings)
    pub edit_status: String,
    pub edit_lines: [String; crate::editor::ROWS],
    /// the typed lyrics in use
    pub typed: String,
    /// the whole editor, as saved with the project
    pub editor_state: Vec<u8>,
    /// the singer is singing the typed lyrics (not a lyrics.txt set)
    pub typed_active: bool,
    /// the words sung before the next syllable, and after it (the performance page's lyric line)
    pub before: String,
    pub after: String,
    /// the lyrics' next line: the words after `after`
    pub upcoming: String,
    /// `next`, `before`, `after` and `position` are about the syllable sounding now (true), or the
    /// one the next key will sing (false: nothing sounds)
    pub singing: bool,
}

/// `s` cut to at most `max` bytes on a character boundary, keeping its end (`keep_end`) or its
/// start: the host shows at most 31 bytes of a parameter's text.
fn fit(s: String, max: usize, keep_end: bool) -> String {
    if s.len() <= max {
        return s;
    }
    if keep_end {
        let mut i = s.len() - max;
        while !s.is_char_boundary(i) {
            i += 1;
        }
        s[i..].to_string()
    } else {
        let mut i = max;
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        s[..i].to_string()
    }
}

pub struct Shared {
    params: [AtomicU64; N_PARAMS],
    stop: AtomicBool,
    events: Ring<Event>,
    cmds: Ring<Cmd>,
    pub display: Mutex<Display>,
    /// MIDI events dropped because the planner fell behind (diagnostic)
    pub dropped: AtomicU32,
    /// on-screen keyboard presses (from any thread: a spin flag guards the producer side)
    keys: Ring<u8>,
    key_lock: AtomicBool,
    /// typed lyrics restored from a project, for the planner to pick up
    typed_in: Mutex<Option<crate::editor::Editor>>,
    /// bumped whenever the editor's text or the typed lyrics change (the shell refreshes its label)
    pub text_version: AtomicU32,
    /// bumped whenever the next syllable or its context changes
    pub display_version: AtomicU32,
    /// bumped when the lyrics or the singer's place in them change by a deliberate act (Done, a
    /// restart, the start syllable, the source or set, a restored project, the transport starting,
    /// the dictionary arriving) and never by a note: the shell refreshes the PLAY page's lyric
    /// line on this, so the host hears from it per user action, not per note (1.3.1)
    pub jump_version: AtomicU32,
    /// the head drawing's mouth pose (`Singer::pose`), read by the audio thread
    pub pose: AtomicU32,
    /// 1.3.3, the moving head (see `anim`): on/off, the visible articulators after each block
    /// (written by the audio thread only while on), the pose shown and a counter of its changes
    pub anim_on: AtomicBool,
    pub artic: crate::anim::ArticSnapshot,
    pub anim_frame: AtomicU32,
    pub anim_version: AtomicU32,
    fs: f64,
    plugin_dir: PathBuf,
}

impl Shared {
    pub fn param(&self, i: usize) -> f64 {
        f64::from_bits(self.params[i].load(Ordering::Relaxed))
    }
    pub fn set_param(&self, i: usize, v: f64) {
        if i < N_PARAMS && v.is_finite() {
            self.params[i].store(v.to_bits(), Ordering::Relaxed);
        }
    }
    pub fn body_index(&self) -> usize {
        (self.param(P_VOICE).round().max(0.0) as usize).min(BODIES.len() - 1)
    }
    fn quality(&self) -> usize {
        (self.param(P_QUALITY).round().max(0.0) as usize).min(QUALITY.len() - 1)
    }
}

pub fn build_voice(fs: f64, body: usize, quality: usize) -> Box<Voice> {
    let b = BODIES[body];
    let (oversample, ctrl, fast, ratio) = QUALITY[quality];
    let mut v = HumanVoice::new(Voice::internal_rate(fs, ratio), b.body(), oversample);
    v.set_control_interval(ctrl);
    v.airway.net.fast_math = fast;
    if fast {
        v.airway.still_tolerance = 1e-4;
    }
    v.set_source(&SourcePatch::f0(default_f0(b)));
    v.telemetry = false;
    // warm up here, off the audio thread: whatever the first renders set up lazily is done now
    let mut v = Voice::new(v, ratio);
    let mut warm = [0.0f32; 256];
    v.render(&mut warm);
    Box::new(v)
}

/// The knob settings the audio thread applies to the voice directly.
#[derive(Clone, Copy, PartialEq)]
struct SourceKnobs {
    pressure: f64,
    breathiness: f64,
    vibrato_rate: f64,
    tension: f64,
    brightness: f64,
    roughness: f64,
    ring: f64,
}

struct Audio {
    voice: Box<Voice>,
    scratch: Vec<Gesture>,
    applied: Option<SourceKnobs>,
    /// pitch bend, −1..1
    bend: f64,
    gain: f64,
    was_playing: bool,
    /// a retired voice the event queue had no room for yet
    retire: Option<*mut Voice>,
}

pub struct Instrument {
    pub shared: Arc<Shared>,
    audio: UnsafeCell<Audio>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// without a thread (tests): the planner, stepped by hand
    pub manual: Option<Mutex<Worker>>,
}

// The audio half is only touched from the audio thread (`render`, `midi`, `transport`); the rest is
// atomics, queues and a mutex.
unsafe impl Sync for Instrument {}
unsafe impl Send for Instrument {}

impl Instrument {
    /// A new instrument at `fs`. `plugin_dir` is the plugin's folder (lyrics.txt and Content/).
    /// `threaded`: run the planner on its own thread (false: step it with `pump`, for tests).
    pub fn new(fs: f64, plugin_dir: PathBuf, params: &[f64; N_PARAMS], threaded: bool) -> Instrument {
        chimera::init();
        let shared = Arc::new(Shared {
            params: std::array::from_fn(|i| AtomicU64::new(params[i].to_bits())),
            stop: AtomicBool::new(false),
            events: Ring::new(1024),
            cmds: Ring::new(4096),
            display: Mutex::new(Display::default()),
            dropped: AtomicU32::new(0),
            keys: Ring::new(256),
            key_lock: AtomicBool::new(false),
            typed_in: Mutex::new(None),
            text_version: AtomicU32::new(0),
            display_version: AtomicU32::new(0),
            jump_version: AtomicU32::new(0),
            pose: AtomicU32::new(0),
            anim_on: AtomicBool::new(false),
            artic: crate::anim::ArticSnapshot::default(),
            anim_frame: AtomicU32::new(0),
            anim_version: AtomicU32::new(0),
            fs,
            plugin_dir,
        });
        let voice = build_voice(fs, shared.body_index(), shared.quality());
        let worker = Worker::new(shared.clone());
        let audio = Audio { voice, scratch: Vec::with_capacity(MAX_SCHEDULE), applied: None, bend: 0.0, gain: db_to_gain(params[P_VOLUME]), was_playing: false, retire: None };
        let (thread, manual) = if threaded {
            let th = std::thread::Builder::new().name("chimera-planner".into()).spawn(move || worker.run()).expect("the planner thread");
            (Some(th), None)
        } else {
            (None, Some(Mutex::new(worker)))
        };
        Instrument { shared, audio: UnsafeCell::new(audio), thread, manual }
    }

    /// Any thread: the moving head on or off (off: the snapshot stops, the pose goes back to rest).
    pub fn set_animation(&self, on: bool) {
        self.shared.anim_on.store(on, Ordering::Relaxed);
    }

    /// Tools and tests: the visible articulators after the last block rendered with the animation on.
    pub fn artic_now(&self) -> Option<[f32; crate::anim::N_VIS]> {
        self.shared.artic.read()
    }

    /// Tests: run the planner until it has nothing left to do.
    pub fn pump(&self) {
        if let Some(w) = &self.manual {
            let mut w = w.lock().unwrap();
            while w.step() {}
        }
    }

    /// Any thread, real-time safe: an on-screen keyboard press (see `editor` for the codes).
    /// False if it was dropped (the queue is full, or another thread held it for too long).
    pub fn key(&self, code: u8) -> bool {
        let sh = &self.shared;
        for _ in 0..64 {
            if sh.key_lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                let ok = sh.keys.push(code);
                sh.key_lock.store(false, Ordering::Release);
                return ok;
            }
            std::hint::spin_loop();
        }
        false
    }

    /// Non-real-time: the editor's state to save (a restore not yet picked up counts).
    pub fn editor_state(&self) -> Vec<u8> {
        let slot = self.shared.typed_in.lock().unwrap_or_else(|e| e.into_inner());
        match &*slot {
            Some(e) => e.to_bytes(),
            None => self.shared.display.lock().unwrap_or_else(|e| e.into_inner()).editor_state.clone(),
        }
    }

    /// Non-real-time: the editor restored from a saved project (`Editor::to_bytes`; any other
    /// bytes are taken as the lyrics' text). The screen's fingerprint follows at once.
    pub fn set_editor_state(&self, bytes: &[u8]) {
        let e = crate::editor::Editor::from_bytes(bytes).unwrap_or_else(|| {
            let mut e = crate::editor::Editor::default();
            e.set_applied(&String::from_utf8_lossy(bytes));
            e
        });
        self.restore(e);
    }

    /// Non-real-time: restore these lyrics as typed and sung.
    pub fn set_typed(&self, text: &str) {
        let mut e = crate::editor::Editor::default();
        e.set_applied(text);
        self.restore(e);
    }

    fn restore(&self, e: crate::editor::Editor) {
        let mut slot = self.shared.typed_in.lock().unwrap_or_else(|e| e.into_inner());
        self.shared.text_version.store(e.fingerprint(), Ordering::Release);
        *slot = Some(e);
    }

    /// Tests: pump until the dictionary has loaded too (as a session a moment after loading is).
    pub fn settle(&self) {
        if let Some(w) = &self.manual {
            let mut w = w.lock().unwrap();
            while w.step() || w.dict_rx.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn audio(&self) -> &mut Audio {
        unsafe { &mut *self.audio.get() }
    }

    /// Audio thread: a MIDI message (status, data1, data2).
    pub fn midi(&self, status: u8, d1: u8, d2: u8) {
        let ev = match status & 0xF0 {
            0x90 if d2 > 0 => Event::NoteOn(d1 & 0x7F, d2 & 0x7F),
            0x80 | 0x90 => Event::NoteOff(d1 & 0x7F),
            0xB0 if d1 == 120 || d1 == 123 => Event::AllOff,
            0xE0 => {
                let v = ((d1 as i32 & 0x7F) | ((d2 as i32 & 0x7F) << 7)) - 8192;
                self.audio().bend = (v as f64 / 8192.0).clamp(-1.0, 1.0);
                return;
            }
            _ => return,
        };
        if !self.shared.events.push(ev) {
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Audio thread: the host's transport state, once per block.
    pub fn transport(&self, playing: bool) {
        let a = self.audio();
        if playing && !a.was_playing && self.shared.param(P_PLAY_SYNC) >= 0.5 {
            self.shared.events.push(Event::PlayStarted);
        }
        if !playing && a.was_playing {
            self.shared.events.push(Event::AllOff);
        }
        a.was_playing = playing;
    }

    /// Audio thread: render `out.len()` mono samples.
    pub fn render(&self, out: &mut [f32]) {
        let sh = &*self.shared;
        let a = self.audio();
        if let Some(p) = a.retire {
            if sh.events.push(Event::Retire(p)) {
                a.retire = None;
            }
        }
        // the planner's commands, in order
        while let Some(c) = sh.cmds.pop() {
            match c {
                Cmd::Gesture(g) => {
                    if a.scratch.len() < MAX_SCHEDULE {
                        a.scratch.push(g);
                    }
                }
                Cmd::Commit(lead) => {
                    let off = a.voice.clock + lead;
                    for g in a.scratch.iter_mut() {
                        g.t0 += off;
                        g.t1 += off;
                    }
                    a.voice.schedule(&a.scratch);
                    a.scratch.clear();
                }
                Cmd::Release(tag) => a.voice.release(tag),
                Cmd::Voice(p) => {
                    let old = Box::into_raw(std::mem::replace(&mut a.voice, unsafe { Box::from_raw(p) }));
                    a.scratch.clear();
                    a.applied = None;
                    // an older retiree still waiting: queue it, or leak it (never free here)
                    if let Some(prev) = a.retire.take() {
                        let _ = sh.events.push(Event::Retire(prev));
                    }
                    if !sh.events.push(Event::Retire(old)) {
                        a.retire = Some(old);
                    }
                }
            }
        }
        // knobs that need no planning
        let k = SourceKnobs { pressure: sh.param(P_BREATH), breathiness: sh.param(P_BREATHY), vibrato_rate: sh.param(P_VIB_RATE), tension: sh.param(P_PRESSED), brightness: sh.param(P_BRIGHT), roughness: sh.param(P_ROUGH), ring: sh.param(P_RING) };
        if a.applied != Some(k) {
            let prev = a.applied;
            let mut s = SourcePatch::default();
            s.pressure = Some(k.pressure.clamp(1.0, 25.0));
            s.breathiness = Some(k.breathiness.clamp(0.0, 1.0));
            s.vibrato_rate = Some(k.vibrato_rate.clamp(2.0, 9.0));
            s.tension = Some(k.tension.clamp(0.0, 1.0));
            s.brightness = Some(k.brightness.clamp(0.0, 1.0));
            s.roughness = Some(k.roughness.clamp(0.0, 1.0));
            a.voice.set_source(&s);
            if prev.map(|p| p.ring) != Some(k.ring) {
                let mut ap = ArticPatch::default();
                ap.values[ArticKey::Epilarynx as usize] = Some(k.ring.clamp(0.0, 1.0));
                a.voice.set_articulation(&ap, None);
            }
            a.applied = Some(k);
        }
        let semis = sh.param(P_TRANSPOSE) + sh.param(P_FINE) / 100.0 + a.bend * sh.param(P_BEND_RANGE);
        a.voice.pitch_ratio = chimera::math::pow(2.0, semis / 12.0);
        a.voice.render(out);
        // the moving head: the articulators this block ended on (14 atomic stores; nothing when off)
        if sh.anim_on.load(Ordering::Relaxed) {
            sh.artic.write(&a.voice.artic.x);
        }
        // volume, smoothed per sample (a one-pole of about 10 ms)
        let target = db_to_gain(sh.param(P_VOLUME));
        let c = 1.0 - (-1.0 / (0.01 * sh.fs)).exp();
        for o in out.iter_mut() {
            a.gain += c * (target - a.gain);
            *o = (*o as f64 * a.gain) as f32;
        }
    }
}

impl Drop for Instrument {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // what is still queued: retired voices and a voice never picked up
        while let Some(e) = self.shared.events.pop() {
            if let Event::Retire(p) = e {
                drop(unsafe { Box::from_raw(p) });
            }
        }
        while let Some(c) = self.shared.cmds.pop() {
            if let Cmd::Voice(p) = c {
                drop(unsafe { Box::from_raw(p) });
            }
        }
        if let Some(p) = self.audio().retire.take() {
            drop(unsafe { Box::from_raw(p) });
        }
    }
}

pub fn db_to_gain(db: f64) -> f64 {
    if db <= -60.0 { 0.0 } else { 10f64.powf(db / 20.0) }
}

/// The planner's side of the command queue: waits (it may) when the audio thread is behind.
struct CmdSink<'a> {
    shared: &'a Shared,
}

impl CmdSink<'_> {
    fn send(&mut self, c: Cmd) {
        let mut spins = 0;
        while !self.shared.cmds.push(c) {
            if self.shared.stop.load(Ordering::Relaxed) {
                return;
            }
            spins += 1;
            if spins > 2000 {
                return; // the audio thread has stopped pulling (suspended): give up on this one
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Sink for CmdSink<'_> {
    fn release(&mut self, tag: f64) {
        self.send(Cmd::Release(tag));
    }
    fn schedule(&mut self, gestures: &[Gesture], lead: f64) {
        for g in gestures.iter().take(MAX_SCHEDULE) {
            self.send(Cmd::Gesture(*g));
        }
        self.send(Cmd::Commit(lead));
    }
}

/// The planner thread's state.
pub struct Worker {
    shared: Arc<Shared>,
    pub singer: Singer,
    g2p: EnglishG2P,
    zh: MandarinLexicon,
    zh_loaded: bool,
    dict_loaded: bool,
    sets: Vec<LyricSet>,
    lyrics_file: Option<(PathBuf, SystemTime)>,
    last_file_check: Option<Instant>,
    /// what the voice was built with: body index, quality
    built: (usize, usize),
    seen_set: f64,
    seen_start: f64,
    seen_restart: f64,
    started: bool,
    /// the pronouncing dictionary, loading on a helper thread (notes go on meanwhile, with the
    /// letter-to-sound rules)
    dict_rx: Option<std::sync::mpsc::Receiver<EnglishG2P>>,
    /// the dictionary loader: joined before the planner ends, so no thread of ours outlives
    /// `cm_free` (a host that unloads the library right after closing would crash it)
    dict_thread: Option<std::thread::JoinHandle<()>>,
    pub editor: crate::editor::Editor,
    seen_source: f64,
    /// this round moved the lyrics by a deliberate act (see `Shared::jump_version`)
    jumped: bool,
    /// when the head's pose was last chosen
    last_anim: Option<Instant>,
}

impl Worker {
    fn new(shared: Arc<Shared>) -> Worker {
        let built = (shared.body_index(), shared.quality());
        Worker {
            singer: Singer::new(BODIES[built.0]),
            g2p: EnglishG2P::new(),
            zh: MandarinLexicon::new(),
            zh_loaded: false,
            dict_loaded: false,
            sets: Vec::new(),
            lyrics_file: None,
            last_file_check: None,
            built,
            seen_set: f64::NAN,
            seen_start: f64::NAN,
            seen_restart: f64::NAN,
            started: false,
            dict_rx: None,
            dict_thread: None,
            editor: crate::editor::Editor::default(),
            seen_source: f64::NAN,
            jumped: false,
            last_anim: None,
            shared,
        }
    }

    fn run(mut self) {
        while !self.shared.stop.load(Ordering::Relaxed) {
            if !self.step() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if let Some(t) = self.dict_thread.take() {
            let _ = t.join();
        }
        // drop retired voices still queued
        while let Some(e) = self.shared.events.pop() {
            if let Event::Retire(p) = e {
                drop(unsafe { Box::from_raw(p) });
            }
        }
    }

    fn content(&self, name: &str) -> PathBuf {
        self.shared.plugin_dir.join("Content").join(name)
    }

    /// The selected set's syllables, read with whatever dictionaries are loaded.
    fn syllables_of_set(&mut self) -> Vec<Syllable> {
        let text = if self.typed_active() {
            self.editor.applied().to_string()
        } else {
            match self.file_set_text() {
                Some(t) => t,
                None => return Vec::new(),
            }
        };
        if !self.zh_loaded && text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c) || ('\u{3400}'..='\u{4DBF}').contains(&c)) {
            self.zh_loaded = true;
            if let Some(t) = lyrics::read_gz(&self.content("mandarin.txt.gz")) {
                self.zh.load(&t);
            }
        }
        lyrics_to_syllables_with(&text, &self.g2p, &self.zh)
    }

    /// Typed lyrics are sung when the source is "Typed" and there are some.
    fn typed_active(&self) -> bool {
        self.shared.param(P_SOURCE) < 0.5 && !self.editor.applied().trim().is_empty()
    }

    /// The selected lyrics.txt set's text.
    fn file_set_text(&self) -> Option<String> {
        let n = self.sets.len();
        if n == 0 {
            return None;
        }
        let i = (self.shared.param(P_LYRIC_SET).round().max(1.0) as usize - 1).min(n - 1);
        Some(self.sets[i].text.clone())
    }

    fn publish_editor(&self) {
        {
            let mut d = self.shared.display.lock().unwrap();
            d.edit_status = self.editor.status();
            let lines = self.editor.lines(crate::editor::LINE_BYTES, crate::editor::ROWS);
            for (slot, l) in d.edit_lines.iter_mut().zip(lines) {
                *slot = l;
            }
            d.typed = self.editor.applied().to_string();
            d.editor_state = self.editor.to_bytes();
            d.typed_active = self.typed_active();
        }
        self.shared.text_version.store(self.editor.fingerprint(), Ordering::Release);
    }

    fn start_index(&self) -> usize {
        (self.shared.param(P_START).round().max(1.0) as usize) - 1
    }

    fn apply_set(&mut self) {
        let syls = self.syllables_of_set();
        let start = self.start_index();
        self.singer.set_syllables(syls, start);
    }

    fn load_lyrics(&mut self) {
        let found = lyrics::find(&self.shared.plugin_dir);
        let mut status = String::new();
        self.sets = match &found {
            Some((p, _)) => match lyrics::read_text(p) {
                Some(t) => lyrics::parse(&t),
                None => {
                    status = "lyrics.txt unreadable".into();
                    Vec::new()
                }
            },
            None => {
                status = "no lyrics.txt".into();
                Vec::new()
            }
        };
        if status.is_empty() {
            status = format!("{} lyric set{}", self.sets.len(), if self.sets.len() == 1 { "" } else { "s" });
        }
        self.lyrics_file = found;
        self.apply_set();
        let mut d = self.shared.display.lock().unwrap();
        d.titles = self.sets.iter().map(|s| s.title.clone()).collect();
        d.status = status;
    }

    fn publish(&self) {
        let mut d = self.shared.display.lock().unwrap();
        self.shared.pose.store(self.singer.pose(), Ordering::Release);
        // the syllable sounding now while a note sings, else the next one (1.3.2: the screen
        // follows the singing; the shell decides how often to tell the host)
        let (at, singing) = self.singer.focus();
        let next = fit(self.singer.text_at(at), 30, false);
        let (before, after, upcoming) = self.singer.context_at(at, 18, 14, 26);
        let (before, after, upcoming) = (fit(before, 30, true), fit(after, 30, false), fit(upcoming, 30, false));
        let position = at + 1;
        let count = self.singer.syllables.len();
        if d.next != next || d.before != before || d.after != after || d.position != position || d.count != count
            || d.upcoming != upcoming || d.singing != singing
        {
            d.next = next;
            d.before = before;
            d.after = after;
            d.upcoming = upcoming;
            d.singing = singing;
            d.position = position;
            d.count = count;
            self.shared.display_version.fetch_add(1, Ordering::Release);
        }
    }

    /// One round of work; false when there was nothing to do.
    pub fn step(&mut self) -> bool {
        self.animate();
        let busy = self.step_inner();
        if busy && self.jumped {
            self.jumped = false;
            // after `publish`: the shell re-reads a display that is already up to date
            self.shared.jump_version.fetch_add(1, Ordering::Release);
        }
        busy
    }

    /// About 12 times a second: the head's pose nearest the articulators sounding now (rest
    /// while the animation is off). Publishes it only when it changes.
    fn animate(&mut self) {
        let now = Instant::now();
        if self.last_anim.is_some_and(|t| now.duration_since(t) < ANIM_PERIOD) {
            return;
        }
        self.last_anim = Some(now);
        let sh = &self.shared;
        let frame = if sh.anim_on.load(Ordering::Relaxed) {
            match sh.artic.read() {
                Some(v) => crate::anim::pose_for(sh.body_index(), &v),
                None => sh.anim_frame.load(Ordering::Relaxed),
            }
        } else {
            0
        };
        if frame != sh.anim_frame.load(Ordering::Relaxed) {
            sh.anim_frame.store(frame, Ordering::Release);
            sh.anim_version.fetch_add(1, Ordering::Release);
        }
    }

    fn step_inner(&mut self) -> bool {
        let mut busy = false;
        let sh = self.shared.clone();
        if !self.started {
            self.started = true;
            // lyrics at once (letter-to-sound for English), then again with the dictionary
            self.load_lyrics();
            self.seen_set = sh.param(P_LYRIC_SET);
            self.seen_start = sh.param(P_START);
            self.seen_restart = sh.param(P_RESTART);
            self.publish();
            let path = self.content("cmudict.txt.gz");
            let (tx, rx) = std::sync::mpsc::channel();
            let stop = sh.clone();
            let spawned = std::thread::Builder::new().name("chimera-dict".into()).spawn(move || {
                if let Some(t) = lyrics::read_gz(&path) {
                    // closed meanwhile: don't spend the parse (cm_free waits for this thread)
                    if stop.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let mut g = EnglishG2P::new();
                    g.load_dict(&t);
                    let _ = tx.send(g);
                }
            });
            if let Ok(t) = spawned {
                self.dict_rx = Some(rx);
                self.dict_thread = Some(t);
            }
            self.last_file_check = Some(Instant::now());
            self.publish();
            self.jumped = true;
            return true;
        }
        // the voice: rebuilt off the audio thread when the body or quality changes
        let want = (sh.body_index(), sh.quality());
        if want != self.built {
            let v = build_voice(sh.fs, want.0, want.1);
            let mut sink = CmdSink { shared: &sh };
            sink.send(Cmd::Voice(Box::into_raw(v)));
            self.built = want;
            self.singer.voice_replaced(BODIES[want.0]);
            busy = true;
        }
        self.singer.vibrato = sh.param(P_VIB_DEPTH).clamp(0.0, 200.0);
        self.singer.velocity_sens = sh.param(P_VEL_SENS).clamp(0.0, 1.0);
        // lyrics: another set, a new start position, a restart
        let set = sh.param(P_LYRIC_SET);
        if set != self.seen_set {
            self.seen_set = set;
            self.apply_set();
            self.jumped = true;
            busy = true;
        }
        let start = sh.param(P_START);
        let restart = sh.param(P_RESTART);
        // Restart is a toggle: every flip, either way, goes back to the start
        if start != self.seen_start || restart != self.seen_restart {
            self.seen_start = start;
            let i = self.start_index();
            self.singer.seek(i);
            self.jumped = true;
            busy = true;
        }
        self.seen_restart = restart;
        // the touchscreen editor: a restored project first (it replaces whatever was typed before
        // it, so presses still queued from then are dropped), then key presses in order
        let restored = {
            // the display's copy changes under the slot's lock, so a save reading the slot and
            // then the display never sees neither
            let mut slot = self.shared.typed_in.lock().unwrap_or_else(|e| e.into_inner());
            let t = slot.take();
            if let Some(e) = &t {
                let mut d = self.shared.display.lock().unwrap();
                d.typed = e.applied().to_string();
                d.editor_state = e.to_bytes();
            }
            t
        };
        if let Some(e) = restored {
            while sh.keys.pop().is_some() {}
            self.editor = e;
            self.apply_set();
            self.publish_editor();
            self.jumped = true;
            busy = true;
        }
        while let Some(code) = sh.keys.pop() {
            busy = true;
            match self.editor.key(code) {
                crate::editor::Outcome::Done => {
                    self.apply_set();
                    self.jumped = true;
                }
                crate::editor::Outcome::Load => {
                    if let Some(t) = self.file_set_text() {
                        self.editor.set_draft(&t);
                    }
                }
                _ => {}
            }
            self.publish_editor();
        }
        let source = sh.param(P_SOURCE);
        if source != self.seen_source {
            self.seen_source = source;
            self.apply_set();
            self.publish_editor();
            self.jumped = true;
            busy = true;
        }
        // MIDI and retired voices
        let mut sink = CmdSink { shared: &sh };
        while let Some(e) = sh.events.pop() {
            busy = true;
            match e {
                Event::NoteOn(n, v) => self.singer.note_on(n, v, &mut sink),
                Event::NoteOff(n) => self.singer.note_off(n, &mut sink),
                Event::AllOff => self.singer.all_off(&mut sink),
                Event::PlayStarted => {
                    let i = self.start_index();
                    self.singer.seek(i);
                    self.jumped = true;
                }
                Event::Retire(p) => drop(unsafe { Box::from_raw(p) }),
            }
        }
        // the dictionary has arrived: read the lyrics again with it (the cursor stays)
        if let Some(rx) = &self.dict_rx {
            match rx.try_recv() {
                Ok(g) => {
                    self.g2p = g;
                    self.dict_loaded = true;
                    self.dict_rx = None;
                    let cursor = self.singer.cursor;
                    let syls = self.syllables_of_set();
                    self.singer.set_syllables(syls, cursor);
                    self.jumped = true;
                    busy = true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.dict_rx = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        // lyrics.txt edited or swapped (another drive, a new file): read it again
        if self.last_file_check.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
            self.last_file_check = Some(Instant::now());
            let now = lyrics::find(&sh.plugin_dir);
            if now != self.lyrics_file {
                self.load_lyrics();
                self.jumped = true;
                busy = true;
            }
        }
        if busy {
            self.publish();
        }
        busy
    }
}
