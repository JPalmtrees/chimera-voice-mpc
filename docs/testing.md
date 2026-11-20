# Testing

`python scripts/test.py` runs every automated test against a build from `scripts/build.py`. None
of them replace playing the plugin on an MPC. A candidate is called hardware-tested only after
that.

## 1. Rust (engine bridge)

`cargo test --release -p chimera-mpc`, run in `external/chimera-vocal-synth` after bootstrap.

- **Unit tests:**
  - the lyrics editor: typing, shift, cursor, Unicode backspace, state round trip, the protected
    Clear All, and three-line wrapping. A long UTF-8 text is walked character by character, and
    every line must fit in 30 bytes;
  - lyrics files;
  - the wait-free ring buffer;
  - the Lite resampler;
  - the articulator snapshot (round trip, and never torn across threads, over 200,000 writes);
  - nearest-pose matching.
- **Integration tests** (`tests/instrument.rs`, the real instrument offline):
  - **Singing:** syllables in order; every body holds at every quality; Lite independent of
    block size.
  - **Arithmetic:** fast arithmetic within tolerance of the exact path.
  - **Real time:** **zero heap allocations while rendering**, across body and quality switches
    and with the animation on; the planner thread keeps up in real time.
  - **Lyrics:** lyric sets and restart; typed lyrics (type, apply, restore, fall back).
  - **Display:** the screen follows the sounding syllable (notes, legato, melisma, silence, wrap).
  - **Articulation:** the head poses follow it. In the live articulators and in the poses chosen,
    "saw" opens the jaw more than "see", "see" raises the tongue, and "sue" rounds and protrudes
    the lips. The head rests in silence, and turning it off stops the snapshot.

## 2. Generated skin

`plugin/tests/skin_test.py <Plugin Skins folder | release zip> [params.json]` checks the
**packaged** `TUI.json`, not the generator's source. Visibility is evaluated the way the MPC
behaves: a "Show" entry is always drawn.

It checks:

- **Pages:** the tabs (PLAY, VOICE); the keyboard is hidden unless open, and the panel hides the
  Q-Link highlight.
- **Captions:** no text label is bound to "Invalid".
- **Touch order:** nothing is drawn over a button.
- **Quality:** LITE, ECO, STD and HIGH set values 3, 0, 1 and 2 on both pages.
- **Keyboard layout:** DONE and CANCEL captioned and closing the panel, no Load File key,
  Backspace far from Clear All, Space at least 3.5 keys wide.
- **PLAY page content:** the draft lines are bound; no Start Syllable readout or help text; the
  Live Lyrics and Animation switches are present.
- **Head:** the moving-head strips (32 frames, inside the head panel, shown by Voice).
- **Q-Links:** 13-16 are Body, Breath, Breathy and Rough; 9, 10 and 12 are Vibrato, Pitch and
  Volume.
- **Images:** every image the skin names is present.

The test fails on the 1.3.0-rc2, 1.3.1-rc1 and 1.3.2-rc1 packages for the problems that were
fixed after them.

## 3. Compiled plugin (VST2 harnesses)

They are built with the SDK's zig and run against the native build (Windows).

- **`keyboard_test.cpp`:**
  - **Parameters:** every parameter index is the same as in 1.2-1.3.2 (88 parameters).
  - **Typing:** a new instance sings typed words; Clear All asks before erasing; typing,
    duplicate-tap suppression and backspace work.
  - **Done and Cancel:** Done applies, selects typed words and refreshes the lyric line; five
    open, type and CANCEL cycles apply nothing.
  - **Saved state:** saving and reloading restores the lyrics; programs keep the lyrics and
    Quality; a 1.1 project's High loads as High; a legacy "File" project keeps its file.
  - **Quality:** all four Quality values work.
  - **Long lyrics:** a 193-character, three-sentence lyric with "café" is held, saved, reloaded,
    stays on screen while the cursor moves, and is sung to its last syllable (49 syllables).
- **`realtime_test.cpp`:** a host that answers every notification by re-reading the display
  text, synchronously.
  - It checks that Done refreshes the line once (a few calls).
  - **Live Lyrics on:** it records what the host is shown at each refresh, which must be the
    syllables really sung, marked SINGING or NEXT. It checks the spacing (at least 0.2 s), the
    rate (5 a second or less) and 5 calls per refresh.
  - **Animation on:** at least 4 distinct poses, at least 0.08 s apart, at most 12.5 a second,
    back at rest at the end.
  - **Both off:** zero host calls from the audio thread while singing.
  - **Timing:** the typed syllables are sung in order, and 99% of 128-frame blocks render in
    under half the 2.9 ms deadline, in each phase.

## 4. SDK host simulator

`external/mpc-plugin-builder-sdk/.sdk-tools/host_sim(.exe) <native plugin> test --seed N` checks
VST2 conformance, parameter text, state round trip with randomized values, invalid chunks, MIDI
and silence. One check sometimes warns that a re-saved chunk differs. This is expected: some
display-only parameters (refresh counters) move after a load.

## 5. On the MPC One

Before a candidate is tagged `-hw-tested`, it is installed from its zip and played:

1. It loads.
2. Each page looks right.
3. Typing, DONE and CANCEL work.
4. Live lyrics follow the notes; the animation follows them, and rests in silence.
5. All four Quality buttons work.
6. The Q-Link knobs work.
7. CPU readout per quality, with no crackles at tempo in Lite.
8. Saving and reloading keeps the lyrics.

## Results for this release

Run on 2026-11-20 from a clean clone of this repository (Windows 11, rustc 1.100.0), against the
build made from it:

| Suite | Result |
|---|---|
| Rust: 16 unit + 13 integration tests | all passed |
| Generated skin (`skin_test.py`) | OK, 0 failures (it also passes on the exact hardware-tested zip, and fails on the 1.3.0-rc2, 1.3.1-rc1 and 1.3.2-rc1 zips) |
| `keyboard_test` | OK, 0 failures |
| `realtime_test` | OK, 0 failures. Live lyrics: 12 refreshes in 2.6 s (4.6/s), closest 200 ms, 60 calls. Animation: 19 head calls in 3.2 s (5.9/s), closest 81 ms, 7 distinct poses. Both off: 0 host calls from the audio thread. p99 block time 0.40 / 0.40 / 0.41 ms (live / animation / both off), deadline 2.9 ms |
| Host simulator, seeds 1-3 | PASS, 19 checks each, 0 warnings |

The whole run took 3 min 7 s. The PC timings are for orientation; they say nothing about the
MPC's CPU.
