# CHIMERA Voice for Akai MPC

Standalone physical-modeling vocal synthesizer for Akai MPC hardware, featuring live lyrics,
animated vocal-tract articulation and optimized ARM audio processing.

![The PLAY page, rendered on a PC from the plugin's skin files](media/play-page-skin-preview.png)

<sub>Above: the PLAY page drawn on a PC from the plugin's real skin images and layout, with
substitute fonts and a new instance's text (`scripts/skin_preview.py`). It is not a capture from
the MPC. **Photo or video from the MPC One: [to be added](docs/media-checklist.md).**</sub>

CHIMERA Voice is an MPC instrument plugin that sings the words you type, one syllable per note,
with no samples: every sound comes from a simulated human airway (lungs, vocal folds, pharynx,
mouth and nose) shaped by moving articulators. It brings the open-source
[CHIMERA](https://github.com/jamesondh/chimera-vocal-synth) synthesis engine, written for the
browser and desktop, to the standalone MPC, where it runs on the device's own ARM processor with
a touchscreen interface and the hardware Q-Link knobs.

It is an independent, community project built with the
[MPC Plugin Builder SDK](https://github.com/inmusic-software/mpc-plugin-builder-sdk). It is not an
official Akai Professional or inMusic product, and the CHIMERA engine is the work of its own
author (see [Attribution](#attribution)).

**Status:** version 1.3.3-rc1, hardware-tested on an original first-generation MPC One.
Download it from the [Releases](../../releases) page.

## Features

- **Physically modeled singing.** CHIMERA's airway simulation and phonetic planner, with three
  bodies (male, female, child) and controls for breath pressure, breathiness, fold tension,
  roughness, vibrato and more.
- **Typed lyrics, sung syllable by syllable.** An on-screen keyboard over the PLAY page; each
  note sings the next syllable (English through the CMU pronouncing dictionary; also pinyin,
  Korean and Russian through CHIMERA's front ends). Up to 4,000 characters, saved in the project.
- **Live lyric highlighting.** The lyric line shows the syllable actually being sung, from the
  singer's own state, refreshed at most 5 times a second.
- **Articulator-driven anatomical animation.** The head on the PLAY page shows the pose nearest
  to what the voice's tongue, jaw, lips and velum are doing, about 12 times a second.
- **Low-CPU Lite quality.** A cheaper airway model (simulated at 3/4 rate, resampled) for the
  first-generation MPC One, beside Eco, Std and High.
- **Custom touchscreen interface and Q-Links.** Two pages (PLAY and VOICE), a working quality
  selector, and the eight performance controls on the Q-Link knobs.
- **ARM32 and ARM64 builds** for the MPC range, plus a native build for desktop testing.
- **Save-state and preset compatibility.** Parameters are only ever appended; projects from
  earlier versions load with their settings and lyrics.

## Engineering highlights

These are the problems that shaped the design. [docs/development-history.md](docs/development-history.md)
has the full sequence, and [docs/architecture.md](docs/architecture.md) the details.

- **A 2017-era ARM CPU running a 2026 desktop simulation.** CHIMERA simulates the airway sample by
  sample. The first MPC build used about 52% of the MPC One's audio budget in Eco and crackled. The
  fixes kept the sound: Newton-step replacements for per-sample divides and square roots (about
  124 down to about 5 per sample on a test phrase, within 1e-5 of the exact output), bit-exact
  pruning of the geometry searches, and a separate Lite model at 3/4 rate (about 35% cheaper than
  Eco on a desktop). On the MPC One, Lite peaks around 50%.
- **Nothing slow on the audio thread.** Notes are planned on a separate thread and reach the
  audio thread through wait-free queues; voices are rebuilt off the audio thread and swapped in;
  a test counts heap allocations during rendering and requires zero.
- **The host's screen is expensive.** An early build refreshed six on-screen values at every note
  from the audio callback. It was removed after a stutter report, which later turned out to be a
  MIDI controller's arpeggiator. The design that shipped refreshes only on deliberate actions, or
  when the sung syllable changes, at most 5 times a second, with a switch to turn it off. Tests
  record every host call made from inside the audio callback.
- **Working within a skin system that can't draw.** MPC skins show images, labels and film-strip
  frames; they can't draw lines or blend. The animated head is therefore a pose atlas: the plugin
  sang a corpus offline, recorded its articulators and clustered them into 32 poses per body, each
  drawn with CHIMERA's own anatomy. Live, a lock-free snapshot of the articulators is matched to
  the nearest pose.
- **Undocumented host behaviour, found on hardware.** On the MPC, a "Show" item is always drawn,
  a text label bound to no parameter is invisible, a label over a button takes its taps, and
  parameter text is cut at 31 bytes. Each was diagnosed from hardware reports and is now checked
  by an automated test of the generated skin.
- **Reproducible, verifiable releases.** Every hardware-tested build has a tag and a SHA-256.
  This repository pins the exact upstream commits the release was built from, and its sources
  are byte-identical to the tested ones.

## Architecture

```mermaid
flowchart LR
  subgraph MPC["MPC host (MPC OS, standalone)"]
    MIDI["MIDI notes / transport"]
    SKIN["Touch skin (TUI.json)<br/>PLAY + VOICE pages"]
    QL["Q-Link knobs"]
  end
  subgraph CPP["C++ plugin shell (VST2, SDK framework)"]
    PROC["process()<br/>MIDI forward, render,<br/>rate-limited refreshes"]
    PARAMS["Parameters 0..87<br/>display text"]
  end
  subgraph RUST["Rust engine bridge (chimera-mpc, static library)"]
    AUDIO["Audio thread:<br/>CHIMERA HumanVoice<br/>(airway, folds, articulators)"]
    PLAN["Planner thread:<br/>singer, note planner,<br/>lyrics editor, pose choice"]
    SNAP["Articulator snapshot<br/>(seqlock, lock-free)"]
  end
  MIDI --> PROC
  QL --> PARAMS
  SKIN <--> PARAMS
  PROC -->|"cm_midi (wait-free queue)"| PLAN
  PLAN -->|"gestures (wait-free queue)"| AUDIO
  PROC -->|cm_render| AUDIO
  AUDIO -->|"12 values per block"| SNAP
  SNAP --> PLAN
  PLAN -->|"syllable, lyric line, pose"| PARAMS
  PROC -->|"audioMasterAutomate<br/>(only on change, capped)"| SKIN
```

- **Rust (`engine/chimera-mpc`)**: everything about the voice. It covers the audio-thread
  renderer, the planner thread (the live singing controller, CHIMERA's note planners, lyrics and
  the typed-lyrics editor), the animation's articulator snapshot and pose matching, and a C ABI
  (`cm_*`).
- **C++ (`plugin/src/ChimeraVoice.cpp`)**: a thin VST2 shell on the MPC SDK framework. It handles
  parameters and their display text, saving the state, and the rate-limited host refreshes.
- **Skin (`plugin/skins_post.py`, `plugin/play_page.py`, `plugin/skin/`)**: generated at build
  time. It holds the PLAY page, the keyboard panel, the pose-atlas images and the Q-Link map.

## Hardware performance

Readings below come from the CPU display on an original first-generation MPC One, observed by the
developer. They are not lab benchmarks.

| Quality | Peak CPU on the MPC One | Practical? |
|---|---|---|
| Lite | about 50% | yes (recommended) |
| Eco | about 89% at times | usable, little headroom |
| Std | about 118-125% | no |
| High | about 167-200% | no |

The animation (1.3.3-rc1) and live lyrics (1.3.2-rc1) caused no noticeable added latency,
stuttering, underruns or CPU cost in hardware testing. PC measurements (render time per audio
block, host calls per second) are in [docs/hardware-performance.md](docs/hardware-performance.md),
kept apart from the MPC readings.

## Installation

1. Use an Akai MPC running **MPC OS 3.10.1 or later**. Only the first-generation MPC One has been
   tested.
2. On the MPC, enable plugins: **Preferences → Enhancements (Beta) → Enable Enhancements** and
   **External Plugin Support**.
3. Download `chimera-voice-1.3.3-rc1-mpc.zip` from [Releases](../../releases). Check its SHA-256:
   `78e1fcc982e0525fa45019b0c783ee9afb487ecfe660986c5598e38b22de4f79`.
4. Unzip it. Copy the folder **`Gator Audio - VST - Chimera Voice`** into a folder named
   **`VST2 Plugins`** at the root of an MPC drive (SD card or USB), then restart the MPC.
5. Load **Chimera Voice** (vendor "Gator Audio", the plugin's maker name) on a plugin track and
   play notes. Choose **LITE** on an MPC One.

The zip also contains `INSTALL.txt`. [docs/troubleshooting.md](docs/troubleshooting.md) covers a
plugin that doesn't appear, crackles or high CPU.

## Building from source

```sh
git clone --recurse-submodules https://github.com/JPalmtrees/chimera-voice-mpc.git
cd chimera-voice-mpc
rustup target add armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu
python scripts/build.py --package   # engine for ARM32/ARM64/native, then the plugin and zip
python scripts/test.py              # Rust, skin, compiled-plugin and host-simulator tests
```

You need Python 3.10+, Git and Rust. On Windows you also need the `x86_64-pc-windows-gnu` target
for the native test build. The SDK installs its C++ toolchain (zig) through pip on first use.
The build was verified from a clean clone on Windows. [docs/building.md](docs/building.md) has
the details, the output paths and the platform limits.

## Testing

- **Rust:** unit and integration tests of the engine bridge. They cover syllable timing, lyrics
  and the editor, quality modes, zero allocation while rendering, display tracking, the
  articulator snapshot and articulation facts ("saw" opens the jaw more than "see").
- **Generated skin:** a test of the packaged `TUI.json` against the rules learned on the MPC.
- **Compiled plugin:** two VST2 harnesses.
  - `keyboard_test`: parameters, typing, Done/Cancel, saved state from older versions, long
    UTF-8 lyrics.
  - `realtime_test`: the syllables sung, the host calls made from the audio callback and their
    rate, render time per block.
- **SDK host simulator:** VST2 conformance, state round trip, MIDI and silence.
- **On hardware:** each release candidate is played on the MPC One before it is called
  hardware-tested.

See [docs/testing.md](docs/testing.md).

## Limitations

- Hardware-validated on the **first-generation MPC One** only. Other MPC models and firmware
  versions are untested (the ARM64 build is untested on hardware).
- The animation shows the nearest of **32 pre-drawn poses** per body, at most 12.5 changes a
  second, with no blending. It is accurate but slightly jittery, and the larynx height is not
  shown.
- **VOWEL, SPEAK and BEATBOX** are shown on the PLAY page but are not implemented yet.
- There is one voice at a time (monophonic), and only ASCII can be typed on the keyboard.
- The released package's description still has the SDK template's text ("a 4-voice synth…
  Describe your plugin here."). This is cosmetic and left as tested; see
  [docs/known-issues.md](docs/known-issues.md).
- This is an independent project, not endorsed by Akai Professional or inMusic.

## Attribution

- **CHIMERA**, the physical-modeling vocal synthesizer, by Jameson Hodge (MIT):
  https://github.com/jamesondh/chimera-vocal-synth. The engine, airway, anatomy and phonetics
  are upstream work. This project adds the `chimera-mpc` crate and changes five engine files for
  performance ([patches/chimera-vocal-synth](patches/chimera-vocal-synth)).
- **MPC Plugin Builder SDK**, by inMusic Brands, Inc. (MIT):
  https://github.com/inmusic-software/mpc-plugin-builder-sdk. This covers the VST2 framework, build
  and packaging tools, skin generator and host simulator. This project adds two build hooks
  ([patches/mpc-plugin-builder-sdk](patches/mpc-plugin-builder-sdk)).
- The CMU Pronouncing Dictionary, the pinyin data and the Rust crates linked into the plugin are
  listed with their licenses in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

**How this was built.** This is an AI-assisted engineering project. The code was implemented
largely with an AI coding assistant (Claude, in Claude Code). It worked to the project owner's
requirements, constraints, priorities and decisions.

The owner did the following:

- defined each release's goals;
- chose the trade-offs, such as the stability-first defaults and never changing parameter
  numbers;
- reviewed the results;
- did all physical testing on the MPC One, including tracing one "stutter" to an external
  controller's arpeggiator.

The upstream engine and SDK are their authors' work. See
[docs/development-history.md](docs/development-history.md).

## Contributing

Bug reports from real hardware are the most valuable contribution. Include the MPC model,
firmware, quality mode and CPU readout (see [CONTRIBUTING.md](CONTRIBUTING.md)).

## License

MIT for this project's own files ([LICENSE](LICENSE)). The upstream projects and bundled data
keep their own licenses ([licenses/](licenses), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
"MPC" and "Akai Professional" are trademarks of inMusic Brands, Inc.; "VST" is a trademark of
Steinberg Media Technologies GmbH.
