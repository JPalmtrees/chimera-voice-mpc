//! CHIMERA as a standalone MPC instrument. The plugin shell (C++, on the MPC SDK's framework) calls
//! this C ABI; everything about the voice, the singing and the lyrics lives here.
//!
//! Threads: `cm_render`, `cm_midi` and `cm_transport` from the audio thread only; `cm_set_param`
//! from any thread; `cm_new`, `cm_free` and `cm_text` from a non-real-time thread.
pub mod anim;
pub mod anim_poses;
pub mod editor;
pub mod voice;
pub mod instrument;
pub mod lyrics;
pub mod ring;
pub mod singer;

use instrument::{DEFAULTS, Instrument, N_PARAMS};
use std::ffi::{CStr, c_char};
use std::path::PathBuf;

pub const ABI_VERSION: u32 = 7;

#[unsafe(no_mangle)]
pub extern "C" fn cm_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn cm_num_params() -> u32 {
    N_PARAMS as u32
}

/// A new instrument. `plugin_dir`: the plugin's folder, UTF-8, NUL-terminated (may be null).
/// `params`: `cm_num_params()` plain values (null: the defaults).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_new(sample_rate: f64, plugin_dir: *const c_char, params: *const f64) -> *mut Instrument {
    let dir = if plugin_dir.is_null() { PathBuf::from(".") } else { PathBuf::from(unsafe { CStr::from_ptr(plugin_dir) }.to_string_lossy().into_owned()) };
    let mut p = DEFAULTS;
    if !params.is_null() {
        p.copy_from_slice(unsafe { std::slice::from_raw_parts(params, N_PARAMS) });
    }
    let fs = if sample_rate.is_finite() && sample_rate >= 8000.0 { sample_rate } else { 44100.0 };
    Box::into_raw(Box::new(Instrument::new(fs, dir, &p, true)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_free(h: *mut Instrument) {
    if !h.is_null() {
        drop(unsafe { Box::from_raw(h) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_set_param(h: *const Instrument, index: u32, value: f64) {
    if let Some(i) = unsafe { h.as_ref() } {
        i.shared.set_param(index as usize, value);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_midi(h: *const Instrument, status: u8, d1: u8, d2: u8) {
    if let Some(i) = unsafe { h.as_ref() } {
        i.midi(status, d1, d2);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_transport(h: *const Instrument, playing: u32) {
    if let Some(i) = unsafe { h.as_ref() } {
        i.transport(playing != 0);
    }
}

/// Render `n` mono samples into `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_render(h: *const Instrument, out: *mut f32, n: usize) {
    if out.is_null() || n == 0 {
        return;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out, n) };
    match unsafe { h.as_ref() } {
        Some(i) => i.render(out),
        None => out.fill(0.0),
    }
}

/// Text for the screen, UTF-8 into `buf` (NUL-terminated, cut to `cap`); returns its length.
/// kind 0: the next syllable; 1: lyric set `arg`'s title (1-based); 2: status; 3: "position/count";
/// 4: the lyrics editor's status line; 5: what is sung ("Typed", or set `arg`'s title);
/// 6: the words sung before the next syllable; 7: the words after it; 8, 9, 10: the editor's draft
/// as three lines around its cursor (each at most `editor::LINE_BYTES`); 11: the lyrics' next line
/// (the words after kind 7); 12: "SINGING" while kinds 0, 3, 6, 7 and 11 are about the syllable
/// sounding now, "NEXT" while they are about the one the next key sings (1.3.2: kind 0 is the
/// sounding syllable during a note).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_text(h: *const Instrument, kind: u32, arg: i32, buf: *mut c_char, cap: usize) -> usize {
    let Some(i) = (unsafe { h.as_ref() }) else { return 0 };
    if buf.is_null() || cap == 0 {
        return 0;
    }
    let s = {
        let d = match i.shared.display.lock() {
            Ok(d) => d,
            Err(e) => e.into_inner(),
        };
        match kind {
            0 => d.next.clone(),
            1 => usize::try_from(arg - 1).ok().and_then(|k| d.titles.get(k).cloned()).unwrap_or_else(|| "-".into()),
            2 => d.status.clone(),
            3 => format!("{}/{}", d.position, d.count),
            4 => d.edit_status.clone(),
            5 => {
                if d.typed_active {
                    "Typed".into()
                } else {
                    usize::try_from(arg - 1).ok().and_then(|k| d.titles.get(k).cloned()).unwrap_or_else(|| "-".into())
                }
            }
            6 => d.before.clone(),
            7 => d.after.clone(),
            8..=10 => d.edit_lines[(kind - 8) as usize].clone(),
            11 => d.upcoming.clone(),
            12 => (if d.singing { "SINGING" } else { "NEXT" }).into(),
            _ => String::new(),
        }
    };
    let mut n = s.len().min(cap - 1);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, n);
        *buf.add(n) = 0;
    }
    n
}

/// Any thread, real-time safe: an on-screen keyboard key (printable ASCII, or a control code from
/// `editor`). Returns 0 if the press was dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_key(h: *const Instrument, code: u8) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.key(code) as u32,
        None => 0,
    }
}

/// Changes whenever the editor's text or the typed lyrics change.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_text_version(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.text_version.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// Any thread: changes whenever the next syllable or the words around it change (kinds 0, 3, 6, 7).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_display_version(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.display_version.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// Any thread, real-time safe: changes when the lyrics or the place in them change by a deliberate
/// act (Done, restart, start syllable, source, set, a restored project, the transport starting),
/// never by a note. The shell refreshes the lyric line's labels on it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_jump_version(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.jump_version.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// Any thread (1.3.3): the moving head on or off. Off: the audio thread stops copying the
/// articulators and the pose goes back to rest (0) within about 80 ms.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_set_animation(h: *const Instrument, on: u32) {
    if let Some(i) = unsafe { h.as_ref() } {
        i.set_animation(on != 0);
    }
}

/// Any thread, real-time safe (1.3.3): the head pose to show (0 = rest .. cm_anim_frames() - 1),
/// chosen about 12 times a second from the articulators sounding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_anim_frame(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.anim_frame.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// Any thread, real-time safe (1.3.3): changes whenever `cm_anim_frame` does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_anim_version(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.anim_version.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// The number of head poses per body (the skin's film strips have this many frames).
#[unsafe(no_mangle)]
pub extern "C" fn cm_anim_frames() -> u32 {
    anim_poses::K as u32
}

/// Any thread, real-time safe: the head drawing's mouth pose for what is sounding (0 rest, 1 a,
/// 2 e, 3 i, 4 o, 5 u).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_pose(h: *const Instrument) -> u32 {
    match unsafe { h.as_ref() } {
        Some(i) => i.shared.pose.load(std::sync::atomic::Ordering::Acquire),
        None => 0,
    }
}

/// Non-real-time: the lyrics editor's state (sung text, draft, cursor) for the project, into `buf`
/// if `cap` is enough; returns its full length in bytes (call with `cap` 0 to size the buffer).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_state_get(h: *const Instrument, buf: *mut u8, cap: usize) -> usize {
    let Some(i) = (unsafe { h.as_ref() }) else { return 0 };
    let t = i.editor_state();
    if !buf.is_null() && cap >= t.len() {
        unsafe { std::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len()) };
    }
    t.len()
}

/// Non-real-time: the editor state from a saved project (as `cm_state_get` gave it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cm_state_set(h: *const Instrument, data: *const u8, len: usize) {
    let Some(i) = (unsafe { h.as_ref() }) else { return };
    let bytes = if data.is_null() || len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(data, len) } };
    i.set_editor_state(bytes);
}
