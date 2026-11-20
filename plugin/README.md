# Chimera Voice (Gator Audio)

CHIMERA's physical-modelling singing voice as a standalone MPC instrument. There are no samples:
every sound comes from a simulated airway. That means lungs, vocal folds, pharynx, mouth and nose,
all shaped by articulators and driven by CHIMERA's own phonetic planner. Each MIDI note sings the
next syllable of your lyrics.

## Install

1. You need MPC OS **3.10.1 or later**. On the MPC, turn on **Preferences → Enhancements (Beta) →
   Enable Enhancements and External Plugin Support**.
2. Copy the folder `build/Gator Audio - VST - Chimera Voice/` into a folder named `VST2 Plugins`
   at the top of an MPC drive (SD card, USB stick or internal drive). You can also copy the
   unzipped package from `python tools/package.py "plugins/Chimera Voice"`.
3. Load **Chimera Voice** on a plugin program and play pads or keys.

## The PLAY page

Two pages: **PLAY** (performance, lyrics and the keyboard) and **VOICE** (every synthesis control).
1.3.1 removed the LYRICS page and 1.3.2 the KEYS page (the keyboard opens over PLAY); their
parameters are all still there, so older projects load as saved. Typed words are the normal way to
give the voice its lyrics. A new instance starts singing "Hello there, I am a voice made of
air." as typed words you can edit.

- **Top row:** the mode buttons and **QUALITY** (LITE / ECO / STD / HIGH, the same order as on
  VOICE). Touch one to select it; the selected one is yellow. **LITE** is the low-CPU mode for an
  original MPC One. **SING** is the only mode; VOWEL, SPEAK and BEATBOX are greyed out and marked
  "not in this version".
- **The head (left):** CHIMERA's own anatomy of the chosen body (male, female or child, drawn from
  the engine's `HumanVocalTract`), with its tract length. **ANIMATION: ON / OFF** (off by default)
  makes it move with the voice: the tongue, jaw, lips, teeth and soft palate take the pose
  nearest to what the voice's articulators are doing, about 12 times a second (see "The moving
  head" below). Off, it stays at rest; nothing about the sound changes either way.
- **The lyrics panel (right):**
  - **CURRENT PHRASE**, with what the yellow box shows: "SINGING 3/24" while a note sings
    syllable 3, "NEXT 4/24" in silence (the syllable the next key will sing).
  - **RESTART** goes back to the start syllable. **TYPE LYRICS** opens the keyboard.
  - The big line: the words before, the **syllable** in the yellow box, the words after, and
    under it the lyrics' next line. It comes from the singer's own state (the syllable the
    sounding note sang; a held melisma keeps its syllable; a legato glide back to a held key
    keeps the syllable that key sang).
  - **LIVE LYRICS: ON / OFF** (tap it). On: the line follows the singing, refreshed at most 5
    times a second (a change in between shows at the next refresh). Off: as in 1.3.1, it is
    refreshed only by DONE, RESTART, a loaded project or the MPC's PLAY. Either way those
    refresh it at once.
  - **CPU**: this instrument's share of the audio deadline, average and peak.
- **The knob row:** BODY, BREATH, BREATHY, ROUGH, VIBRATO, PITCH, VOLUME. They are Q-Links 13–16
  and 9–12. Q-Links 5–8 are Pressed, Bright, Ring and Vib Rate, and 1–4 are Play Sync, Start Syl,
  Vel Sens and Fine. **REVERB** is unlit: the plugin has no reverb, so use an MPC insert effect
  (its Q-Link does nothing).

**Projects from before 1.3.1** load as they were saved, with their lyrics. A project that sings a
lyrics.txt set (Source = File) keeps singing it; the position readout then says "FILE 1 <title>".
Type words and press DONE to switch it to typed words.

## Typing lyrics on the touchscreen

Press **TYPE LYRICS** on PLAY. The keyboard opens over the page:

- The top line says `(draft)` while your text differs from what is being sung, `(sung)` once DONE
  has handed it to the singer, and how many characters it has (`SHIFT` while Shift is armed).
- Below it, **three lines of your text** around the cursor (`|`). They scroll with the cursor, so
  long, multi-sentence lyrics can be read and edited anywhere with **<** and **>**. Enter starts
  a new line (a breath).
- **DONE** (yellow) sings the text from the start syllable, selects typed words if a file was
  being sung, closes the keyboard and shows PLAY with the new words.
- **CANCEL** closes the keyboard and throws away changes since the last DONE.
- **BACKSPACE** (top right) deletes before the cursor. **CLEAR ALL** (bottom left) asks first: the
  top line says "TAP CLEAR ALL AGAIN TO ERASE", and only a second tap erases; any other key
  cancels the question.
- Letters, digits, `' ? - , . ! ~` and the wide **SPACE** type. **SHIFT** capitalises the next
  letter.

- The text can be up to 4,000 characters (several minutes of singing). It is saved **in the MPC
  project** (and in plugin presets), with any unfinished draft and its cursor.
- Nothing is written to your drives. The plugin never writes files.
- Typing sends no notes. Programs (presets) change the voice, not the words, and not Quality.
- Each key is one touch. Two touches of the same key within 0.16 s count as one, because some
  MPCs report one tap twice.

## Lyrics from a file (optional; no computer needed once the file is on the drive)

Typed words are the normal workflow since 1.3.1; this file path is kept for projects that use it
(Source = File, no longer shown on a page). The plugin reads a plain-text `lyrics.txt`. It uses the first one it finds, in this order:

1. the **root of the drive** that holds `VST2 Plugins` (for example `SD/lyrics.txt`)
2. `<drive>/Chimera Voice/lyrics.txt`
3. the plugin folder itself
4. the plugin's `Content/lyrics.txt`, which ships with examples

The file format:

- `# Title` starts a lyric set (1.3.1 no longer shows a set selector; older projects keep theirs). Text before the first
  `#`, or a whole file with no `#`, is one set.
- Line breaks and punctuation become breaths. A lone `-` or `~` holds the vowel onto the next
  note (a melisma). Lines starting with `//` are comments.
- English uses the CMU pronouncing dictionary. When the dictionary is still loading (the
  first second or two after the plugin loads) or doesn't have a word, English falls back to
  letter-to-sound rules. Pinyin with tone numbers, Chinese characters, Korean Hangul and
  Russian Cyrillic also work, through CHIMERA's front ends.
- The file is checked every 2 seconds. Swap the card or edit the file and the new words are
  picked up without reloading the plugin.

How playing works (CHIMERA's live Sing mode, ported as-is):

- **Note on** sings the next syllable at the note's pitch: onset consonants, then the held vowel
  with vibrato.
- **Note off** sings the coda.
- **Overlapping notes** sing legato: the previous coda runs into the next onset.
- **Releasing the top note while another is held** glides back to the held note on the same
  vowel.
- **Velocity** sets how hard the singer breathes.

**Start Syl** sets where the lyrics begin. Turning it moves there at once. The **Restart**
toggle jumps back to that syllable each time you flip it. With **Play Sync** on, pressing PLAY
on the MPC also jumps back there, so a sequence always sings the right words.

## Controls

| Page | Control | What it does |
|---|---|---|
| VOICE | Voice | Male / Female / Child: a different vocal tract, folds and default pitch |
| | Breath | lung pressure (cmH2O): softer to louder and more pressed |
| | Breathy | gap between the folds: clear to airy |
| | Volume | output level |
| | Pressed | fold tension |
| | Bright | how sharply the folds close (brightness) |
| | Rough | jitter and shimmer |
| | Ring | narrows the epilarynx: the "singer's formant" near 3 kHz |
| | Vibrato, Vib Rate | depth on held vowels (cents) and rate (Hz) |
| | Vel Sens | how much velocity changes the breath |
| | Quality | LITE / ECO / STD / HIGH buttons (see CPU below) |
| | Transpose, Fine, Bend Rng | pitch offset, and the pitch-bend range |
| | CPU | this instrument's share of the audio deadline: average, then peak |
| PLAY | Quality, RESTART, TYPE LYRICS, Start Syl, the knob row | described above |
| | Play Sync | Q-Link 1: jump back to Start Syl when the MPC starts playing |
| | Live Lyrics | the lyric line follows the singing (on by default) |
| (no page) | Lyrics, Source | a project's lyrics.txt set and Typed / File: kept for older projects |

There are 7 built-in programs: Tenor, Soprano, Child, Airy Whisper, Opera Bass, Belt and Rough
Rocker.

The voice is **monophonic**. CHIMERA models one throat, and each extra voice would cost a full
simulation.

## CPU and the Quality switch

| Quality | Airway simulated at | Articulation update | Arithmetic | Desktop cost (Ryzen 9 9950X3D), busy singing |
|---|---|---|---|---|
| **Eco** (default) | 44.1 kHz | every 2.9 ms | fast | 3.7–3.8% of a core |
| **Std** | 44.1 kHz | every 1.45 ms | fast | 4.4–4.5% |
| **High** | 88.2 kHz (as the web app) | every 1.45 ms | exact | 10.3–11.3% |
| **Lite** (1.2) | 33.1 kHz, resampled to 44.1 kHz | every 5.8 ms | fast | 2.4–2.5% |

On an original MPC One, 1.1's Eco measured about 52% (average and peak on the CPU readout), with
occasional crackles. On the desktop, 1.2's Eco costs the same as 1.1's (3.8% both, best of three
runs); its changes target what is expensive on the MPC's Cortex-A17 and cheap on a desktop CPU:
most per-sample divides and square roots, and most `hypot` calls in the geometry (see "Fast
arithmetic" below). Whether that makes Eco cheaper on the MPC has not been measured yet. Lite is
about 35% cheaper than Eco on the desktop.

1. Start with **Eco** and watch the CPU readout (re-open the page to refresh it).
2. If it crackles or the peak stays above about 50%, choose **Lite**.
3. **Std** costs a little more than Eco; **High** is unlikely to fit on a 32-bit MPC.

**Lite** is a separate, cheaper model, not a degraded Eco. The airway's sections must be at least
one sound-travel step long, so at 3/4 of the rate the tract has fewer, longer sections and fewer
time steps: the simulation costs about (3/4)² of Eco's. Its bandwidth ends near 15 kHz, which
covers the voice and its fricatives, but the coarser tract moves the upper formants (F3/F4,
around 2.5–3.5 kHz) by a few dB, so the timbre is slightly different from Eco; listen and choose.

A larger audio buffer (Preferences → Audio) gives more headroom.

"Fast" arithmetic is defined in the engine (`AirwayNetwork::fast_math` and `still_tolerance`):

- **Moving sections:** a one-step Newton update of 1/A, 1/A_min and √A replaces the divides and
  square roots. It is exact again at every control step.
- **Walls:** they use reciprocal multiplies.
- **Held notes:** while no articulator moves, the tract geometry isn't recomputed.
- **Since 1.2:** each node's compliance multiplies by 1/ρc², its T/C and each branch's
  1/(L/T + R) take one Newton step from the previous sample's, and the kinetic-loss quadratic
  uses its series when its argument is small (as it is in the open tract). On a male Eco
  phrase this takes the per-sample divides and square roots from about 124 to about 5 (counted).

It changes the output by less than 1e-5 (about −110 dB against 1.1's Eco). A test checks that it
stays under 0.25 dB in every 10 ms window. In practice it measures 0.0000 dB.

The tract geometry (computed at each articulation update) also got cheaper in 1.2, bit-exactly
in every mode: nearest-point searches skip the `hypot` and divide of segments that cannot be the
nearest, and ray tests work out each vertex's side of the ray once.

## Building

You need Rust (rustup, with targets `armv7-unknown-linux-gnueabihf` and
`aarch64-unknown-linux-gnu`) and the CHIMERA fork at `../chimera-vocal-synth` beside the SDK, or
set `CHIMERA_DIR` to point at it.

```
python "plugins/Chimera Voice/build_engine.py"
```

The PLAY page's artwork is committed in `skin/`. To redraw it (this needs Pillow:
`pip install pillow`), run `python skin/make_assets.py`. The head comes from
`skin/head_outlines.json`, which is written by the engine fork's
`cargo run --release -p chimera-mpc --example head_outlines`. `play_page.py` lays out the page,
and `skins_post.py` adds it to the generated skin as the first tab.

This runs four steps:

1. `cargo` builds `crates/chimera-mpc` as a static library for arm32, arm64 and this computer.
2. The libraries are copied to `lib/<target>/`, where `link.json` names them for
   `tools/build.py`.
3. The dictionary, the Mandarin lexicon and the licence files are copied to `Content/`.
4. The normal SDK build and validation run.

Test it with:

```
set MPC_PLUGIN_FOLDER=plugins\Chimera Voice\build\Gator Audio - VST - Chimera Voice
.sdk-tools\host_sim "plugins/Chimera Voice/build/native/chimera-voice.dll" test --notes 50 --seconds 3 --wav test.wav
```

Engine tests live in the fork:

- `cargo test -p chimera-mpc --release` covers singing, lyrics, quality modes, fast-maths
  equivalence, allocation-free rendering and the real-time planner thread.
- `cargo test -p chimera --release` covers the original engine suite, including the golden
  fixtures. The engine changes are bit-exact by default.
- `cargo run --release -p chimera-mpc --example bench_mpc [held]` is the benchmark (best of
  three runs per mode).
- `cargo run --release -p chimera-mpc --example render_mpc -- <dir> [baseline dir]` renders a
  fixed phrase per mode and voice to WAV, and compares against an earlier render: sample
  difference and 1/3-octave band differences (`BANDS=1` prints each band).
- `tests/keyboard_test.cpp` drives the built plugin through its VST2 interface. It types with
  duplicated key reports, presses Done, then checks the state-chunk round trip into a new
  instance and that a program change keeps the words. To build and run it:
  `zig c++ -std=c++17 -I framework "plugins/Chimera Voice/tests/keyboard_test.cpp" -o keyboard_test`,
  then `keyboard_test <native plugin>` with `MPC_PLUGIN_FOLDER` set as above.

## How it is built (real-time safety)

- **Audio thread** (Rust, called from `process()`): it renders the voice, applies gestures and
  knob values, and forwards MIDI. It doesn't allocate, lock or do I/O, and a test counts
  allocations during render to prove it.
- **Planner thread:** it runs the live singing controller (a port of the web app's
  `singing.ts`), CHIMERA's note planners, lyrics loading and voice rebuilds (Voice and Quality
  changes). The rebuilt voice is swapped in on the audio thread, and the old one is freed back on
  the planner thread.
- **Dictionary thread:** it loads the CMU dictionary once at start-up. Since 1.3 it is joined
  when the plugin closes. Before that, a host that unloaded the library right after closing it
  (a quick load-and-unload, as in a plugin scan) could crash while the thread was still
  loading.
- **Communication:** the threads talk through two wait-free single-producer/single-consumer
  queues.

## Known limits

- **Note timing:** notes are planned on the planner thread, so a note sounds about 1–3 ms plus up
  to one audio block after its MIDI event. Planning is not sample-accurate.
- **Live readouts:** the MPC re-reads a value's text when the plugin tells it the value moved, or
  when it redraws the page. The plugin tells it on a key press (the keyboard's four lines), on
  DONE, RESTART, the start syllable, a loaded project or the transport starting, and, with Live
  Lyrics on, when the sung syllable changes: 5 calls (Next, Before, After, position, next line)
  per refresh, at most 5 refreshes a second (at most 25 calls a second; 1.3.0-rc1 made about six
  per note, unlimited). The calls are made from the audio callback, after the block is rendered:
  the MPC gives a plugin no other thread to notify it from (no editor idle; its own screen and
  knob traffic arrives on the audio thread), and that path is the one already proven on an
  original MPC One (typing, DONE). The text itself is formatted on the MPC's UI thread when it
  re-reads the labels. What the screen redraw costs the MPC can only be seen on the hardware: if
  it ever disturbs the audio, turn Live Lyrics off. `tests/realtime_test.cpp` records what the
  host is shown at each refresh, checks the rate, and checks that with Live Lyrics off no host
  call is made while notes play.
- **The moving head (1.3.3):** the original CHIMERA app redraws its anatomy from the voice's 17
  articulator values at 60 frames a second, as vector lines. The MPC's skin can only show frames
  of pre-drawn images, so:
  - offline, the engine fork's `--example anim_poses` sings a corpus through this instrument
    (Lite), records the 12 visible articulators (jaw; tongue body, tip, curl and root; lip
    aperture, protrusion, width and labiodental contact; velum) after every 128-sample block,
    and clusters them into 32 poses per body (pose 0: at rest), transitions included. Each pose
    is drawn by CHIMERA's anatomy; `skin/make_assets.py --anim` splits the drawings into a still
    base and a film strip of only the region that moves (about 190 x 186 px), exactly (outside
    it the poses differ by at most 2/255);
  - live, with Animation on, the audio thread copies those 12 values after each block (lock-free,
    no allocation); the planner thread picks the nearest pose about every 80 ms; the plugin moves
    "Tract" to it, one host call per change, at most 12.5 a second (5.9 a second measured while
    singing "la di da"). Off: no copying, no calls, the rest pose.
  - Limits: the nearest pose is shown, not a blend (the skin can't blend two images); the larynx
    height and the other articulators (epilarynx, ventricular folds, lateral channel, clicks) are
    drawn at rest; a pose change is at most 12.5 a second, so fast consonants can be skipped.
    The images of the three heads decode to about 14 MB (1.3.2's six-frame heads: 11.5 MB).
- **Screen text length:** the SDK hands the MPC at most 31 bytes of a parameter's value text.
  Up to 1.3.0 the keyboard's one draft line was longer, so after about 23 characters the cursor
  and the newest letters were cut off the screen (the text itself was kept). 1.3.1 shows the draft
  as three lines of at most 30 bytes, cut on character boundaries.
- **Voice changes:** changing Voice or Quality rebuilds the voice, so a note sounding at that
  moment stops.
- **Keyboard:** the panel over PLAY is hidden by its own handle (valid only while Keyboard is on),
  which `tests/skin_test.py` checks. If a project saved with the panel open reopens with it open,
  press CANCEL or DONE. While it is open it hides the MPC's Q-Link highlight (hideQLinkBounds):
  the highlight of the knob row used to show through over SHIFT and SPACE (1.3.1-rc1).
- **Skin rules learnt on an original MPC One** (all checked by `tests/skin_test.py` on the
  packaged skin): an entry marked "Show" is always drawn; a Text label bound to no parameter is
  not drawn (1.3.0-rc2's blank DONE and BACK keys); a Label drawn over a Button takes its touches
  (1.3.0-rc2's Quality buttons did nothing), so Buttons are drawn last.
- **Keyboard text:** only ASCII can be typed. Unicode lyrics (from an older project or lyrics.txt)
  are kept, shown and edited.
- **No song mode:** CHIMERA's whole-song tunes and Sing-a-tune mode, the creatures and the
  beatbox are not exposed. This plugin is the live Sing path.

## Hardware test history (original first-generation MPC One)

Each release below was installed and played on an original MPC One by the developer. Its zip is
kept by the developer as `chimera-voice-<version>-mpc-HW-TESTED.zip`, and its commits are
tagged `v<version>-hw-tested` (engine fork) and `chimera-voice-v<version>-hw-tested` (this SDK).

| Version | zip SHA-256 | Findings |
|---|---|---|
| 1.1.0 | `036c9c02...92` | works; Eco about 52% CPU, some crackles; Done didn't return to the page |
| 1.2.0-rc1 | `1d80c128...94` | much better: no tempo skips or glitches; peaks 95-98% with BASSLINE also loaded |
| 1.3.0-rc2 | `e54fc298...f9` | PLAY renders, typed lyrics sung. CPU peaks: Lite ~50%, Eco ~89%, Std 118-125%, High 167-200%. (1.3.0-rc1's stutter was the controller's arpeggiator) |
| 1.3.1-rc1 | `81c79696...d1` | works very well: keyboard, DONE back to PLAY, CURRENT PHRASE; the Q-Link highlight showed over SHIFT/SPACE |
| 1.3.2-rc1 | `f5512d53...41` | live lyrics, typing, navigation, audio and Lite all work |
| 1.3.3-rc1 | `78e1fcc9...79` | the head animation works and follows the synthesized articulation, accurately synchronised but slightly jittery; no new latency, underruns, stuttering or noticeable performance cost; live lyrics and the other controls still work |

Full hashes: 1.3.3-rc1 `78e1fcc982e0525fa45019b0c783ee9afb487ecfe660986c5598e38b22de4f79`
(engine `0ba7bb1`, SDK `0e419e3`); the others are in the tags' messages and by `sha256sum` of
the backups.

The animation's jitter is expected from its design: it shows the nearest of 32 poses, without
blending, at most 12.5 changes a second, so it steps between poses. Possible later remedies,
each to be hardware-tested on its own: more poses (48 measured about 15% closer, ~6 MB more image
memory), or a little hysteresis in the pose choice (keep the current pose unless another is
clearly nearer).

## Licences

| Part | Licence |
|---|---|
| CHIMERA engine | MIT, © 2026 Jameson Hodge (`Content/CHIMERA-LICENSE.txt`) |
| MPC Plugin SDK framework | MIT, © 2026 inMusic Brands (`Content/MPC-SDK-LICENSE.txt`) |
| CMU Pronouncing Dictionary | BSD-style, © Carnegie Mellon University (`Content/CMUDICT-LICENSE.txt`) |
| Mandarin pinyin data | MIT, © mozillazg (`Content/MANDARIN-PINYIN-LICENSE.txt`) |
| Rust crates (`libm`, `serde`, `serde_json`, `flate2`/`miniz_oxide`) | MIT/Apache-2.0 |

The example lyrics are public-domain texts. This plugin is MIT licensed.
