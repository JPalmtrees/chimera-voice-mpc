# Development history

The port was developed over a few days in November 2026, in short cycles: build a candidate, test
it on the MPC One, find the cause of what went wrong, fix it, and test again. Every candidate the
developer confirmed on hardware is tagged in the two development repositories and kept as a zip
with its SHA-256.

## How the work was done

- **Upstream work.** The CHIMERA engine (airway physics, vocal folds, articulators, anatomy,
  phonetic planners) is Jameson Hodge's. The VST2 framework, build tools, skin generator and host
  simulator are inMusic's MPC Plugin Builder SDK.
- **AI-assisted implementation.** The MPC-specific code was written with an AI coding assistant
  (Claude, in Claude Code). That covers the Rust bridge crate, the engine performance changes, the
  C++ shell, the skin generator, the pose-atlas tooling and the tests.
- **The project owner's role:**
  - setting each release's requirements and constraints (never renumber parameters, never change
    the DSP during UI work, stability first, preserve every rollback point);
  - choosing between the proposed approaches and reviewing results;
  - all testing on the physical MPC One, and the diagnoses that only hardware could give. For
    example, the 1.3.0-rc1 "stutter" came from an external controller's arpeggiator.

## Releases

| Version | Highlights | Hardware result |
|---|---|---|
| 1.1.0 | first standalone build: CHIMERA's live Sing mode, lyrics from `lyrics.txt`, an on-screen keyboard tab | works; Eco about 52% CPU with crackles; Done didn't return to the page |
| 1.2.0-rc1 | Lite quality (3/4-rate airway, resampled); per-sample divides and square roots replaced by Newton steps; bit-exact geometry pruning; a TYPE panel that Done closes; programs keep the Quality setting | much better: no tempo skips or glitches |
| 1.3.0-rc1 | PLAY page (anatomical head, lyric line, knob row); restyled keyboard | broken: keyboard drawn over PLAY; "stutter" (later traced to an arpeggiator) |
| 1.3.0-rc2 | keyboard panel hidden by its own handle; per-note host notifications removed; skin and real-time regression tests | works; CPU per quality recorded |
| 1.3.1-rc1 | LYRICS tab removed; typed words by default; three-line editor (the 31-byte display limit); Quality buttons that take touches; keyboard redesign (DONE/CANCEL captions, protected CLEAR ALL, wide SPACE) | works very well; a Q-Link highlight showed over the keyboard |
| 1.3.2-rc1 | two tabs (PLAY, VOICE); Q-Link highlight hidden over the keyboard; live lyrics from the sounding syllable, rate-limited; Live Lyrics switch | works |
| 1.3.3-rc1 | anatomical animation from the voice's articulators (lock-free snapshot, 32-pose atlas per body); Animation switch | works; accurate but slightly jittery; no performance cost noticed |

## Root causes found along the way

| Symptom on the MPC | Cause | Fix and regression test |
|---|---|---|
| Keyboard drawn over PLAY and LYRICS (1.3.0-rc1) | skin entries marked "Show" are always drawn; the conditions added to hide them were ignored | panel hidden by its own `IndexedEnabling` handle with "Hide"; `skin_test.py` evaluates visibility the hardware's way |
| Stutter, wrong words (1.3.0-rc1) | an external MIDI controller's arpeggiator | none needed. The build also made about 36 host calls from the audio thread per 6 notes; these were removed, then reintroduced in 1.3.2 in a bounded form; `realtime_test.cpp` records them |
| Long lyrics seemed to hit a limit (1.3.0) | the SDK passes at most 31 bytes of a parameter's display text; the cursor sat past that | three lines of at most 30 bytes; editor unit test walks a long UTF-8 text |
| Lyric line showed the old phrase after Done | no refresh after a deliberate change | `cm_jump_version` refresh on Done, Restart, project load, transport start |
| Quality buttons did nothing (1.3.0-rc2) | caption labels drawn over the buttons took the touches | buttons drawn last, captions under them; checked by `skin_test.py` |
| DONE and BACK keys blank (1.3.0-rc2) | text labels bound to "Invalid" are not drawn | every caption bound to a real parameter; checked |
| Q-Link highlight over SHIFT/SPACE (1.3.1-rc1) | the knob row's highlight rectangles lie under the keyboard | `hideQLinkBounds` on the keyboard panel; checked |
| Crash on a fast load/unload (host simulator) | the dictionary thread outlived the plugin | thread joined on close |

## Tags and checksums (development repositories)

| Version | Engine tag (fork of CHIMERA) | SDK tag | Release zip SHA-256 |
|---|---|---|---|
| 1.1.0 | `v1.1.0-hw-tested` | `chimera-voice-v1.1.0-hw-tested` | `036c9c02c5132612830fe38b028f2863e967f3cbf369835ecbcb0985f8ca3792` |
| 1.2.0-rc1 | `v1.2.0-rc1-hw-tested` | `chimera-voice-v1.2.0-rc1-hw-tested` | `1d80c128693d710348886d87b0f739b621ee8e816b3791e723e1088066294a87` |
| 1.3.0-rc2 | `v1.3.0-rc2-hw-tested` | `chimera-voice-v1.3.0-rc2-hw-tested` | `e54fc2986e45963b86629b1dc6943f116841229e79fb9b000786693cb6c567f9` |
| 1.3.1-rc1 | `v1.3.1-rc1-hw-tested` | `chimera-voice-v1.3.1-rc1-hw-tested` | `81c79696a83fe96c50e582f23a11ac9a490e2f265394b1a82a8543413e54f9d1` |
| 1.3.2-rc1 | `v1.3.2-rc1-hw-tested` | `chimera-voice-v1.3.2-rc1-hw-tested` | `f5512d53bbd10d5080c8a5e430484ffe9e15b1b7854e3ae164e643318c0fb141` |
| 1.3.3-rc1 | `v1.3.3-rc1-hw-tested` (`0ba7bb1`) | `chimera-voice-v1.3.3-rc1-hw-tested` (`0e419e3`) | `78e1fcc982e0525fa45019b0c783ee9afb487ecfe660986c5598e38b22de4f79` |

This public repository starts a new history. Its first release, `v1.3.3-rc1`, contains the 1.3.3-rc1
sources exactly. Only the 1.3.3-rc1 zip is published, as a release asset.
