# Contributing

Thank you for helping. This plugin runs on real-time hardware with little headroom, so the most
useful contributions are careful ones.

## Reporting a bug

Open an issue with the "Bug report" template and include:

- the MPC model and MPC OS version;
- the plugin version (from the zip name or `plugin.json`) and the Quality mode;
- the CPU readout on PLAY (average and peak) and the other plugins loaded;
- whether Live Lyrics and Animation were on;
- what you did, what you expected and what happened (a photo or video of the screen helps);
- for timing problems: your controller, and whether any arpeggiator or note repeat was on.

## Changing the code

1. Build and test first: `python scripts/build.py && python scripts/test.py` (see [docs/building.md](docs/building.md)).
2. Edit `engine/chimera-mpc` (Rust) or `plugin/` (C++, skin, tests), never the copies inside
   `external/`. Changes to the upstream engine or SDK go into `patches/` as a new or updated diff.
3. Keep the rules the plugin depends on:
   - **Parameters are permanent.** Never reorder, remove or renumber a parameter; append new ones
     at the end. Saved projects refer to them by position and id.
   - **The audio thread** (`cm_render`, `process()`) must not allocate, lock, do I/O or format
     text. The Rust tests count allocations during rendering.
   - **Host notifications** from `process()` must be bounded and switchable (see
     [docs/architecture.md](docs/architecture.md)). `realtime_test.cpp` records them.
   - **Skin rules** learned on the MPC are enforced by `plugin/tests/skin_test.py`. Run it on
     the built skin.
4. Add a test for every bug you fix.
5. Say what you tested and on what. A change is only "hardware-tested" once it has been played
   on an MPC; please state the model.

## Scope

Planned but not started: the VOWEL, SPEAK and BEATBOX modes, and smoother animation (more poses
or hysteresis). Changes to CHIMERA's synthesis belong upstream:
https://github.com/jamesondh/chimera-vocal-synth.
