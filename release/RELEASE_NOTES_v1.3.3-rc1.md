# CHIMERA Voice for Akai MPC v1.3.3-rc1

The first public release. This is the exact build that was hardware-tested on an original
first-generation **Akai MPC One**. It was not rebuilt for publication.

## What it is

A standalone MPC instrument plugin that sings typed lyrics, one syllable per note. It is built on
the physical-modeling engine of [CHIMERA](https://github.com/jamesondh/chimera-vocal-synth)
(simulated lungs, vocal folds and vocal tract; no samples) and on the
[MPC Plugin Builder SDK](https://github.com/inmusic-software/mpc-plugin-builder-sdk). It is an
independent project, not an Akai or inMusic product.

## Highlights

- **Typed lyrics**: an on-screen keyboard over the PLAY page, with three-line editing and a
  protected CLEAR ALL. Lyrics are saved in the project.
- **Live lyric highlighting** from the syllable actually being sung (LIVE LYRICS, on by default).
- **Anatomical animation** from the voice's own articulators, using a 32-pose atlas per body
  (ANIMATION, off by default).
- **Four quality modes**: LITE (recommended on the MPC One), ECO, STD and HIGH.
- **Q-Link mapping** for Body, Breath, Breathy, Rough, Vibrato, Pitch and Volume, and 7 built-in
  programs.
- **ARM32 and ARM64 binaries.** Projects from earlier versions load with their settings and
  lyrics.

## Tested hardware and performance

- **Tested on:** an original first-generation MPC One.
- **CPU display readings:** Lite about 50% peak, Eco up to about 89%. Std and High exceed this
  model's budget.
- **Animation and live lyrics:** no noticeable added latency, stuttering, underruns or CPU cost
  in hardware testing. The animation is accurate but slightly jittery.
- **Not tested:** other MPC models, and the ARM64 build on hardware.

## Install

1. Use MPC OS 3.10.1 or later. Turn on **Preferences → Enhancements (Beta) → Enable
   Enhancements** and **External Plugin Support**.
2. Unzip the file and copy **`Gator Audio - VST - Chimera Voice`** into **`VST2 Plugins`** at
   the root of an MPC drive. Restart the MPC.
3. Load **Chimera Voice** on a plugin track and choose **LITE**.

## Verify the download

```
SHA-256  78e1fcc982e0525fa45019b0c783ee9afb487ecfe660986c5598e38b22de4f79  chimera-voice-1.3.3-rc1-mpc.zip
```

Windows: `certutil -hashfile chimera-voice-1.3.3-rc1-mpc.zip SHA256`. macOS / Linux:
`shasum -a 256 chimera-voice-1.3.3-rc1-mpc.zip`.

Rebuilt from this tag's sources, the zip comes out byte-identical to this asset, except for its
`README.md` (see docs/building.md).

## Known limitations

- VOWEL, SPEAK and BEATBOX are placeholders. REVERB is unlit; use an MPC effect instead.
- The animation shows the nearest of 32 poses, with no blending, at most 12.5 changes a second.
  The larynx height is not shown.
- The voice is monophonic. The keyboard types ASCII only.
- The package's plugin description still has the SDK template text ("a 4-voice synth…"). This
  is cosmetic.
