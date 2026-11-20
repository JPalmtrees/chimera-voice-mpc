//! The MPC instrument end to end, with the planner stepped by hand (deterministic).
use chimera_mpc::instrument::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Counts allocations made while `COUNTING` is on (this thread's renders only run then).
struct Counting;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;
/// tests share the counter: one at a time
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const FS: f64 = 44100.0;

/// A plugin folder laid out as on the MPC, with lyrics.txt in Content/ and the dictionary.
fn plugin_dir(name: &str, lyrics: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("chimera-mpc-test-{name}-{}", std::process::id()));
    let dir = root.join("VST2 Plugins").join("Gator Audio - VST - Chimera Voice");
    std::fs::create_dir_all(dir.join("Content")).unwrap();
    std::fs::write(dir.join("Content").join("lyrics.txt"), lyrics).unwrap();
    let dict = concat!(env!("CARGO_MANIFEST_DIR"), "/../../public/data/cmudict.txt.gz");
    std::fs::copy(dict, dir.join("Content").join("cmudict.txt.gz")).unwrap();
    dir
}

fn params(quality: f64, voice: f64) -> [f64; N_PARAMS] {
    let mut p = DEFAULTS;
    p[P_QUALITY] = quality;
    p[P_VOICE] = voice;
    p
}

/// Render `sec` seconds in host-sized blocks; the RMS.
fn run(inst: &Instrument, sec: f64, block: usize, out: &mut Vec<f32>) -> f64 {
    let n = (sec * FS) as usize;
    let mut buf = vec![0.0f32; block];
    let mut acc = 0.0;
    let mut i = 0;
    while i < n {
        let m = block.min(n - i);
        inst.render(&mut buf[..m]);
        for &x in &buf[..m] {
            assert!(x.is_finite(), "NaN/inf in the output");
            acc += (x as f64) * (x as f64);
        }
        out.extend_from_slice(&buf[..m]);
        inst.pump();
        i += m;
    }
    (acc / n as f64).sqrt()
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

fn sing_phrase(quality: f64, voice: f64) -> (Vec<f32>, Vec<String>) {
    let dir = plugin_dir(&format!("phrase{quality}{voice}"), "# Test\nHello there, I am a voice made of air.\n");
    let inst = Instrument::new(FS, dir, &params(quality, voice), false);
    inst.settle();
    let mut out = Vec::new();
    let mut sung = Vec::new();
    run(&inst, 0.1, 128, &mut out);
    let base = if voice == 0.0 { 50 } else { 62 };
    for (k, step) in [0, 2, 4, 5, 7, 5, 4, 2].iter().enumerate() {
        sung.push(inst.shared.display.lock().unwrap().next.clone());
        inst.midi(0x90, (base + step) as u8, 100);
        inst.pump();
        run(&inst, 0.35, if k % 2 == 0 { 128 } else { 96 }, &mut out);
        inst.midi(0x80, (base + step) as u8, 0);
        inst.pump();
        run(&inst, 0.08, 64, &mut out);
    }
    run(&inst, 0.5, 512, &mut out);
    (out, sung)
}

#[test]
fn sings_the_lyrics_syllable_by_syllable() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (out, sung) = sing_phrase(0.0, 0.0);
    assert_eq!(sung[..4], ["Hel-", "lo", "there", "I"], "{sung:?}");
    let level = rms(&out);
    assert!(level > 0.01 && level < 0.9, "phrase RMS {level}");
    // it ends: the last half second after the coda is quiet
    let tail = rms(&out[out.len() - (0.2 * FS) as usize..]);
    assert!(tail < 0.05 * level, "tail {tail} vs {level}");
}

#[test]
fn every_body_holds_at_every_quality() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    for voice in [0.0, 1.0, 2.0] {
        let hi = rms(&sing_phrase(2.0, voice).0);
        for q in [0.0, 1.0, Q_LITE as f64] {
            let lo = rms(&sing_phrase(q, voice).0);
            eprintln!("voice {voice}: High RMS {hi:.4}, quality {q} RMS {lo:.4}");
            assert!(lo > 0.5 * hi && lo < 2.0 * hi, "voice {voice}: quality {q} {lo} vs High {hi}");
        }
    }
}

#[test]
fn fast_math_sings_as_the_exact_path_does() {
    use chimera::phonetics::sing::{NoteOnOptions, plan_note_on, plan_note_off};
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    chimera::init();
    let syls: Vec<chimera::phonetics::Syllable> = serde_json::from_str(r#"[{"text":"sass","onset":[{"ipa":"s"}],"nucleus":{"ipa":"æ","stress":1},"coda":[{"ipa":"s"}],"wordEnd":true,"phraseEnd":false}]"#).unwrap();
    let render = |fast: bool| {
        let mut v = build_voice(FS, 1, 1);
        v.airway.net.fast_math = fast;
        v.airway.still_tolerance = if fast { 1e-4 } else { 0.0 };
        let on = plan_note_on(&syls[0], 64.0, &NoteOnOptions { body: BODIES[1], vibrato: 35.0, tag: 2.0, legato_from: None });
        let mut out = vec![0.0f32; (1.2 * FS) as usize];
        let shift = |gs: &[chimera::control::score::Gesture], at: f64| gs.iter().map(|g| { let mut g = *g; g.t0 += at; g.t1 += at; g }).collect::<Vec<_>>();
        let c = v.clock;
        v.schedule(&shift(&on, c + 0.006));
        let split = (0.7 * FS) as usize;
        v.render(&mut out[..split]);
        v.release(2.0);
        v.release(3.0);
        let c = v.clock;
        v.schedule(&shift(&plan_note_off(&syls[0], 64.0, BODIES[1]), c + 0.006));
        v.render(&mut out[split..]);
        out
    };
    let (exact, fast) = (render(false), render(true));
    let w = (0.01 * FS) as usize;
    let peak = rms(&exact).max(1e-9);
    let mut worst: f64 = 0.0;
    for k in 0..exact.len() / w {
        let (a, b) = (rms(&exact[k * w..(k + 1) * w]), rms(&fast[k * w..(k + 1) * w]));
        if a > 0.05 * peak {
            worst = worst.max((20.0 * (b / a).log10()).abs());
        }
    }
    let diff = exact.iter().zip(&fast).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    eprintln!("fast vs exact: worst 10 ms window {worst:.4} dB, largest sample difference {diff:e}");
    assert!(diff > 0.0, "the fast path never ran");
    assert!(worst < 0.25, "fast arithmetic moved a 10 ms window by {worst} dB");
}

/// Lite (the airway at 3/4 of the rate, resampled up) sings the same however the host splits
/// its blocks, and fills every sample.
#[test]
fn lite_quality_is_block_size_independent() {
    use chimera::phonetics::sing::{NoteOnOptions, plan_note_on};
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    chimera::init();
    let syls: Vec<chimera::phonetics::Syllable> = serde_json::from_str(r#"[{"text":"sass","onset":[{"ipa":"s"}],"nucleus":{"ipa":"æ","stress":1},"coda":[{"ipa":"s"}],"wordEnd":true,"phraseEnd":false}]"#).unwrap();
    let render = |blocks: &[usize]| {
        let mut v = build_voice(FS, 0, Q_LITE);
        let on = plan_note_on(&syls[0], 52.0, &NoteOnOptions { body: BODIES[0], vibrato: 35.0, tag: 2.0, legato_from: None });
        let c = v.clock;
        let gs: Vec<_> = on.iter().map(|g| { let mut g = *g; g.t0 += c + 0.006; g.t1 += c + 0.006; g }).collect();
        v.schedule(&gs);
        let mut out = vec![0.0f32; (0.6 * FS) as usize];
        let mut at = 0;
        let mut i = 0;
        while at < out.len() {
            let n = blocks[i % blocks.len()].min(out.len() - at);
            v.render(&mut out[at..at + n]);
            at += n;
            i += 1;
        }
        out
    };
    let a = render(&[128]);
    let b = render(&[1, 37, 512, 4096, 64, 333]);
    assert_eq!(a, b, "Lite's output depends on the block size");
    let level = rms(&a[(0.2 * FS) as usize..]);
    assert!(level > 0.01, "Lite sang nothing: RMS {level}");
    assert!(a.iter().all(|x| x.is_finite()));
}

#[test]
fn render_allocates_nothing_even_across_a_body_switch() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("alloc", "la la la la");
    let inst = Instrument::new(FS, dir, &params(2.0, 0.0), false);
    inst.settle();
    // (1.3.3) with the moving head on: the audio thread also copies the articulators each block
    inst.set_animation(true);
    let mut buf = vec![0.0f32; 256];
    let mut sink = Vec::with_capacity(1 << 20);
    run(&inst, 0.05, 256, &mut sink);
    let mut render_counted = |inst: &Instrument| {
        COUNTING.store(true, Ordering::Relaxed);
        inst.render(&mut buf);
        COUNTING.store(false, Ordering::Relaxed);
    };
    for k in 0..60 {
        if k == 5 {
            inst.midi(0x90, 57, 90);
        }
        if k == 20 {
            inst.shared.set_param(P_VOICE, 1.0); // planner rebuilds; the audio thread swaps
            inst.shared.set_param(P_BRIGHT, 0.6);
            inst.midi(0xE0, 0, 96);
        }
        if k == 30 {
            inst.shared.set_param(P_QUALITY, Q_LITE as f64); // a resampled voice swapped in
        }
        if k == 40 {
            inst.midi(0x80, 57, 0);
        }
        inst.pump(); // (not counted: the planner's thread allocates freely)
        let before = ALLOCS.load(Ordering::Relaxed);
        render_counted(&inst);
        let d = ALLOCS.load(Ordering::Relaxed) - before;
        if d > 0 {
            eprintln!("block {k}: {d} allocations");
        }
    }
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0, "the audio thread allocated or freed");
}

#[test]
fn lyric_sets_restart_and_start_position() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("sets", "# One\nla di da\n# Two\nsing to me\n");
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), false);
    inst.settle();
    let next = || inst.shared.display.lock().unwrap().next.clone();
    assert_eq!(inst.shared.display.lock().unwrap().titles, ["One", "Two"]);
    assert_eq!(next(), "la");
    inst.shared.set_param(P_LYRIC_SET, 2.0);
    inst.pump();
    assert_eq!(next(), "sing");
    let mut out = Vec::new();
    inst.midi(0x90, 60, 100);
    inst.pump();
    run(&inst, 0.1, 128, &mut out);
    inst.midi(0x80, 60, 0);
    inst.pump();
    assert_eq!(next(), "to");
    inst.shared.set_param(P_START, 3.0);
    inst.pump();
    assert_eq!(next(), "me");
    // a restart goes back to the start position; so does the transport starting
    inst.midi(0x90, 60, 100);
    inst.pump();
    // (1.3.2: while the key is down the screen shows the syllable it sings)
    assert_eq!(next(), "me");
    assert!(inst.shared.display.lock().unwrap().singing);
    inst.transport(true);
    inst.pump();
    assert_eq!(next(), "me");
    assert!(!inst.shared.display.lock().unwrap().singing, "after a jump the screen shows the next syllable");
}

#[test]
fn the_screen_follows_the_sounding_syllable() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("follow", "la di da - so
");
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), false);
    inst.settle();
    let d = || inst.shared.display.lock().unwrap().clone();
    let shown = || { let x = d(); (x.next.clone(), x.singing, x.position) };
    let mut out = Vec::new();
    let mut play = |on: bool, n: u8| { inst.midi(if on { 0x90 } else { 0x80 }, n, if on { 100 } else { 0 }); inst.pump(); run(&inst, 0.05, 128, &mut out); };
    // silence: the next syllable, as a preview
    assert_eq!(shown(), ("la".into(), false, 1));
    let jumps = inst.shared.jump_version.load(Ordering::Relaxed);
    let v0 = inst.shared.display_version.load(Ordering::Relaxed);
    // a note: the syllable it sings, then (released) the next one
    play(true, 60);
    assert_eq!(shown(), ("la".into(), true, 1));
    assert_eq!(d().after, " di da so");
    assert_ne!(inst.shared.display_version.load(Ordering::Relaxed), v0);
    play(false, 60);
    assert_eq!(shown(), ("di".into(), false, 2));
    assert_eq!(d().before, "la ");
    // legato: the second key takes over at once; releasing it glides back to the first key on
    // the same vowel, which is still the second syllable sounding
    play(true, 60);
    play(true, 62);
    assert_eq!(shown(), ("da".into(), true, 3));
    play(false, 62);
    assert_eq!(shown(), ("da".into(), true, 3), "the glide back keeps the syllable");
    // a melisma mark holds "da" on a new note: still "da" sounding
    play(true, 64);
    assert_eq!(shown(), ("da".into(), true, 3));
    play(false, 64);
    play(false, 60);
    assert_eq!(shown(), ("so".into(), false, 5));
    // notes never count as deliberate jumps (no immediate host refresh per note)
    assert_eq!(inst.shared.jump_version.load(Ordering::Relaxed), jumps);
    // the wrap: after the last syllable the next is the first again
    play(true, 60);
    assert_eq!(shown(), ("so".into(), true, 5));
    play(false, 60);
    assert_eq!(shown(), ("la".into(), false, 1));
}

#[test]
fn no_lyrics_file_sings_ah() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("none", "x");
    std::fs::remove_file(dir.join("Content").join("lyrics.txt")).unwrap();
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), false);
    inst.settle();
    assert_eq!(inst.shared.display.lock().unwrap().status, "no lyrics.txt");
    let mut out = Vec::new();
    inst.midi(0x90, 52, 100);
    inst.pump();
    let r = run(&inst, 0.5, 128, &mut out);
    assert!(r > 0.01, "ah RMS {r}");
}

#[test]
fn the_planner_thread_keeps_up_in_real_time() {
    // as on the MPC: the planner on its own thread, blocks rendered at the audio rate
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("threaded", "Hello there, I am a voice made of air.");
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), true);
    let block = 128usize;
    let dt = std::time::Duration::from_secs_f64(block as f64 / FS);
    let mut buf = vec![0.0f32; block];
    let mut out = Vec::new();
    let start = std::time::Instant::now();
    let notes = [50u8, 52, 54, 55, 57];
    for k in 0..(2.4 * FS) as usize / block {
        let t = k * block;
        let i = t / (0.4 * FS) as usize;
        let phase = t % (0.4 * FS) as usize;
        if i >= 1 && i <= notes.len() && phase < block {
            inst.midi(0x90, notes[i - 1], 100);
        }
        if i >= 1 && i <= notes.len() && (phase as f64) >= 0.3 * FS && (phase as f64) < 0.3 * FS + block as f64 {
            inst.midi(0x80, notes[i - 1], 0);
        }
        inst.render(&mut buf);
        out.extend_from_slice(&buf);
        let due = start + dt * (k as u32 + 1);
        if let Some(w) = due.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(w);
        }
    }
    let sung = rms(&out[(0.4 * FS) as usize..]);
    assert!(out.iter().all(|x| x.is_finite()));
    assert!(sung > 0.03, "RMS while singing {sung}");
    assert_eq!(inst.shared.dropped.load(Ordering::Relaxed), 0);
    let d = inst.shared.display.lock().unwrap().clone();
    assert_eq!(d.position, 6, "five notes sung: {d:?}");
}

fn type_keys(inst: &Instrument, s: &str) {
    for b in s.bytes() {
        assert!(inst.key(b));
    }
}

#[test]
fn touchscreen_lyrics_type_apply_restore_and_fall_back() {
    use chimera_mpc::editor::{K_DONE, K_LOAD, K_REVERT, K_SHIFT};
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("typed", "# File\nla di da\n");
    let inst = Instrument::new(FS, dir.clone(), &params(0.0, 0.0), false);
    inst.settle();
    let d = || inst.shared.display.lock().unwrap().clone();
    assert_eq!(d().next, "la");
    let v0 = inst.shared.text_version.load(Ordering::Relaxed);
    // typing edits only the draft; the singer keeps the file's words until Done
    inst.key(K_SHIFT);
    type_keys(&inst, "moon river, wider");
    inst.pump();
    assert_eq!(d().edit_status, "(draft) 17 chars");
    assert_eq!(d().edit_lines[0], "Moon river, wider|");
    assert_ne!(inst.shared.text_version.load(Ordering::Relaxed), v0);
    assert_eq!(d().next, "la");
    let v1 = inst.shared.text_version.load(Ordering::Relaxed);
    let j1 = inst.shared.jump_version.load(Ordering::Relaxed);
    inst.key(K_DONE);
    inst.pump();
    // Done is confirmed on the line, and the version moves so the host redraws it
    assert_eq!(d().edit_status, "(sung) 17 chars");
    assert_ne!(inst.shared.text_version.load(Ordering::Relaxed), v1);
    // and the PLAY page's lyric line is told to refresh (a deliberate act, not a note)
    assert_ne!(inst.shared.jump_version.load(Ordering::Relaxed), j1);
    assert!(d().typed_active);
    assert_eq!(d().typed, "Moon river, wider");
    assert_eq!(d().next, "Moon");
    // sings them
    let mut out = Vec::new();
    inst.midi(0x90, 55, 100);
    inst.pump();
    let j2 = inst.shared.jump_version.load(Ordering::Relaxed);
    assert!(run(&inst, 0.4, 128, &mut out) > 0.01);
    inst.midi(0x80, 55, 0);
    inst.pump();
    assert_eq!(d().next, "ri-");
    // notes move the singer but never the refresh counter (no host calls per note)
    assert_eq!(inst.shared.jump_version.load(Ordering::Relaxed), j2);
    // revert throws a draft away; load copies the file set into the draft
    type_keys(&inst, " xyz");
    inst.key(K_REVERT);
    inst.pump();
    assert_eq!(d().edit_status, "(sung) 17 chars");
    inst.key(K_LOAD);
    inst.pump();
    assert_eq!(d().edit_lines[0], "la di da|");
    // Source = File sings lyrics.txt again; back to Typed sings the typed words
    inst.shared.set_param(P_SOURCE, 1.0);
    inst.pump();
    assert!(!d().typed_active);
    assert_eq!(d().next, "la");
    inst.shared.set_param(P_SOURCE, 0.0);
    inst.pump();
    assert_eq!(d().next, "Moon");
    drop(inst);
    // a reload restores the project's typed lyrics (what the plugin saves in its state chunk)
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), false);
    inst.settle();
    inst.set_typed("Moon river, wider");
    inst.pump();
    let d = inst.shared.display.lock().unwrap().clone();
    assert!(d.typed_active);
    assert_eq!(d.next, "Moon");
    assert_eq!(d.edit_status, "(sung) 17 chars");
    assert_eq!(d.edit_lines, ["Moon river, wider|".to_string(), String::new(), String::new()]);
}

#[test]
fn keys_from_two_threads_all_arrive_in_order_per_thread() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("keythreads", "la");
    let inst = std::sync::Arc::new(Instrument::new(FS, dir, &params(0.0, 0.0), false));
    inst.settle();
    let other = inst.clone();
    let th = std::thread::spawn(move || {
        for _ in 0..50 {
            while !other.key(b'b') {}
        }
    });
    for _ in 0..50 {
        while !inst.key(b'a') {}
        inst.pump();
    }
    th.join().unwrap();
    inst.pump();
    let w = inst.manual.as_ref().unwrap().lock().unwrap().editor.draft();
    assert_eq!(w.matches('a').count(), 50);
    assert_eq!(w.matches('b').count(), 50);
}

/// The PLAY page's lyric line: the words before the next syllable and after it follow the singer,
/// and the display version moves with them (so the plugin redraws those labels).
#[test]
fn lyric_line_context_follows_the_singer() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("context", "# Test\nHello there, I am a voice made of air.\n");
    let inst = Instrument::new(FS, dir, &params(0.0, 0.0), false);
    inst.settle();
    let mut out = Vec::new();
    run(&inst, 0.05, 128, &mut out);
    let d = || inst.shared.display.lock().unwrap().clone();
    let v0 = inst.shared.display_version.load(std::sync::atomic::Ordering::Acquire);
    assert_eq!((d().before.as_str(), d().next.as_str()), ("", "Hel-"));
    assert!(d().after.starts_with("lo there"), "{:?}", d().after);
    let pose = || inst.shared.pose.load(std::sync::atomic::Ordering::Acquire);
    assert_eq!(pose(), 0, "the head rests before any note");
    for note in [60u8, 62] {
        inst.midi(0x90, note, 100);
        inst.pump();
        run(&inst, 0.2, 128, &mut out);
        // "Hel-" sings /ɛ/ (pose e), "lo" /oʊ/ (pose o)
        assert_eq!(pose(), if note == 60 { 2 } else { 4 }, "head pose while note {note} sounds");
        inst.midi(0x80, note, 0);
        inst.pump();
        run(&inst, 0.05, 128, &mut out);
    }
    let now = d();
    assert_eq!((now.before.as_str(), now.next.as_str()), ("Hello ", "there"), "{now:?}");
    assert!(now.after.starts_with(" I am"), "{:?}", now.after);
    assert!(now.after.chars().count() <= 14 + 6, "{:?}", now.after);
    assert_ne!(inst.shared.display_version.load(std::sync::atomic::Ordering::Acquire), v0);
    assert_eq!(pose(), 0, "the head rests again after the notes");
}

#[test]
fn the_head_follows_the_articulators_and_rests_in_silence() {
    use chimera_mpc::anim::{N_VIS, VIS};
    use chimera_mpc::anim_poses::{K, POSES};
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = plugin_dir("anim", "x");
    let inst = Instrument::new(FS, dir, &params(3.0, 0.0), false);
    inst.set_typed("saw see sue");
    inst.settle();
    let frame = || inst.shared.anim_frame.load(Ordering::Relaxed) as usize;
    let mut out = Vec::new();
    // render, then let the planner's ~12 Hz pose choice run once on what this ended on
    let settle = |sec: f64, out: &mut Vec<f32>| {
        run(&inst, sec, 128, out);
        std::thread::sleep(std::time::Duration::from_millis(90));
        inst.pump();
    };
    // off (the default): no snapshot, rest
    settle(0.2, &mut out);
    assert!(inst.artic_now().is_none(), "the audio thread copies nothing while the animation is off");
    assert_eq!(frame(), 0);
    inst.set_animation(true);
    settle(0.3, &mut out);
    assert_eq!(frame(), 0, "silence: the head at rest");
    let k = |name: &str| VIS.iter().position(|v| format!("{v:?}") == name).unwrap();
    let (jaw, tb_y, lip_prot, lip_width) = (k("Jaw"), k("TbY"), k("LipProt"), k("LipWidth"));
    let mut sung = Vec::new();
    for note in [57u8, 59, 60] {
        inst.midi(0x90, note, 100);
        inst.pump();
        settle(0.45, &mut out);
        let live = inst.artic_now().unwrap();
        let f = frame();
        assert!(f < K);
        // the pose shown is nearer the live articulators than rest is (it is the nearest of all)
        let d = |p: &[f32; N_VIS]| -> f32 { (0..N_VIS).map(|i| ((p[i] - live[i]) / chimera_mpc::anim_poses::SCALE[0][i]).powi(2)).sum() };
        assert!(d(&POSES[0][f]) <= d(&POSES[0][0]), "pose {f} vs rest");
        assert!(POSES[0].iter().all(|p| d(&POSES[0][f]) <= d(p)));
        sung.push((f, live));
        inst.midi(0x80, note, 0);
        inst.pump();
        settle(0.25, &mut out);
    }
    let (saw, see, sue) = (sung[0].1, sung[1].1, sung[2].1);
    // articulatory facts the drawing must show: "saw" opens the jaw more than "see"; "see" raises
    // the tongue body; "sue" protrudes and rounds the lips more than "see"
    assert!(saw[jaw] > see[jaw], "jaw: saw {} see {}", saw[jaw], see[jaw]);
    assert!(see[tb_y] > saw[tb_y], "tongue height: see {} saw {}", see[tb_y], saw[tb_y]);
    assert!(sue[lip_prot] > see[lip_prot] && sue[lip_width] < see[lip_width], "lips: sue {:?} see {:?}", sue, see);
    // ...and the poses chosen for them differ the same way
    let p = |i: usize| POSES[0][sung[i].0];
    assert!(p(0)[jaw] > p(1)[jaw], "poses: saw {} see {}", sung[0].0, sung[1].0);
    assert!(p(2)[lip_prot] > p(1)[lip_prot], "poses: sue {} see {}", sung[2].0, sung[1].0);
    assert!(sung.iter().all(|(f, _)| *f != 0), "a sung vowel is not the rest pose: {:?}", sung.iter().map(|s| s.0).collect::<Vec<_>>());
    // after the note, back to rest
    settle(0.8, &mut out);
    assert_eq!(frame(), 0, "back at rest after the coda");
    // off again: the pose goes back to rest and the snapshot stops moving while notes play
    inst.set_animation(false);
    settle(0.1, &mut out);
    let frozen = inst.artic_now();
    inst.midi(0x90, 57, 100);
    inst.pump();
    settle(0.4, &mut out);
    assert_eq!(frame(), 0);
    assert_eq!(inst.artic_now(), frozen, "nothing copied while off");
    inst.midi(0x80, 57, 0);
    inst.pump();
}
