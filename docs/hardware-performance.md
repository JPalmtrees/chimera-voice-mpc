# Hardware performance

Two kinds of numbers appear here, and they are kept apart:

- **MPC readings**: the developer's observations of the MPC One's own CPU display while playing.
  They are approximate, and were not recorded with a measurement tool.
- **PC measurements**: from the automated tests on the development PC (AMD Ryzen 9 9950X3D,
  Windows 11). They show relative costs and real-time headroom on a desktop, not MPC
  performance.

## Target hardware

Original first-generation Akai MPC One. Its 32-bit ARM build targets the Cortex-A17
(`target-cpu=cortex-a17`). The ARM64 build (for newer MPCs) has not been tested on hardware.

## MPC One readings

| Version | Observation |
|---|---|
| 1.1.0 | Eco: about 52% (average and peak); occasional crackles |
| 1.2.0-rc1 | "much better": no tempo skips or glitches; peaks of 95-98% with the BASSLINE plugin also loaded |
| 1.3.0-rc2 / 1.3.1 era | Lite about 50% peak; Eco about 89% peak (usable); Std about 118-125%; High about 167-200% |
| 1.3.2-rc1 | live lyrics on: no audible glitches |
| 1.3.3-rc1 | animation on: no noticeable added latency, stuttering, underruns or CPU cost; motion follows the articulation, slightly jittery |

Lite and Eco were judged to sound essentially the same on the MPC.

## PC measurements

From `plugin/tests/realtime_test.cpp` (clean-clone run of 2026-11-20). The test plays 128-frame blocks at real-time pace (2.9 ms
apart) through the compiled VST, with a host that re-reads display text at every notification.

| Measure (1.3.3-rc1 build) | Result |
|---|---|
| render time per block, median / p99 / worst | about 0.19 / 0.40 / 0.5-0.6 ms, against a 2.9 ms deadline |
| p99 with Live Lyrics on / Animation on / both off | 0.40 / 0.40 / 0.41 ms (the differences are run-to-run noise) |
| live lyric refreshes while singing | 4.6 a second, never closer than 200 ms, 5 host calls each |
| head-pose refreshes while singing "la di da" | 5.9 a second, never closer than 81 ms, 1 host call each; 6-7 distinct poses (varies by run); back at rest after |
| host calls from the audio thread while singing, Live Lyrics and Animation off | 0 |

Desktop cost of each quality mode, from `engine/chimera-mpc/examples/bench_mpc.rs` (best of three
runs, busy singing, share of one core):

| Quality | Airway rate | Desktop cost |
|---|---|---|
| Lite | 33.1 kHz, resampled to 44.1 kHz | 2.4-2.5% |
| Eco | 44.1 kHz | 3.7-3.8% |
| Std | 44.1 kHz, articulation every 1.45 ms | 4.4-4.5% |
| High | 88.2 kHz, exact arithmetic | 10.3-11.3% |

The desktop and MPC ratios differ. The desktop CPU handles divides, square roots and `hypot`
cheaply; the Cortex-A17 does not. That is why the 1.2 optimizations target those operations,
even though they change the desktop cost very little. The optimizations:

- **Newton steps:** these replace most per-sample divides and square roots (about 124 down to
  about 5 per sample on a male Eco phrase, counted). The output changes by less than 1e-5, and a
  test bounds it at 0.25 dB per 10 ms window.
- **Bit-exact pruning:** nearest-point searches skip segments that cannot be nearest.

## What was not measured

- MPC CPU with a measurement tool (only the on-screen readout).
- The cost of host screen redraws on the MPC itself, beyond "no noticeable change".
- Any MPC other than the first-generation MPC One.
