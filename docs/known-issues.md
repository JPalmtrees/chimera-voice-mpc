# Known issues in the released 1.3.3-rc1 package

These are in the hardware-tested zip (SHA-256
`78e1fcc982e0525fa45019b0c783ee9afb487ecfe660986c5598e38b22de4f79`). They are documented, not
fixed: fixing them would change the package, and the published zip must stay byte-for-byte what
was tested on the MPC.

## Leftover SDK template description (cosmetic)

The plugin's description still carries the text of the SDK's project template:

> Chimera Voice: a 4-voice synth for the MPC. Describe your plugin here.

| Where | File in the zip |
|---|---|
| plugin metadata | `Gator Audio - VST - Chimera Voice/plugin.json` (`"description"`) |
| install notes | `INSTALL.txt` (generated from it by the SDK's packager) |
| source | `plugin/plugin.json` |

- **Impact:** none on how the plugin loads or sounds. The text is also wrong: the voice is
  monophonic, not 4-voice. It was not seen on the MPC's screens during testing, which used the
  plugin's name and vendor.
- **Fix in the next version:** change `description` in `plugin/plugin.json` (for example
  "Physical-modeling singing voice: typed lyrics, one syllable per note"), rebuild, and test the
  new package on hardware.

## Build-machine path inside the binaries (harmless)

Panic-location strings in `chimera-voice-arm32.so` and `chimera-voice-arm64.so` include the Rust
package cache path of the build machine (`C:\Users\Administrator\.cargo\registry\...`). They hold
no personal information and have no effect at run time. A future build can strip them with
`--remap-path-prefix`.
