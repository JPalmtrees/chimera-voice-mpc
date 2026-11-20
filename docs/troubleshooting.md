# Troubleshooting

| Problem | Check |
|---|---|
| The plugin doesn't appear | The MPC runs MPC OS 3.10.1 or later. **Preferences → Enhancements (Beta)** has both **Enable Enhancements** and **External Plugin Support** on. The folder `Gator Audio - VST - Chimera Voice` (not the zip, and not a folder containing it) is inside `VST2 Plugins` at the root of the drive. Restart the MPC after copying. |
| No sound | Play notes on the plugin's track. The first second after loading uses letter-to-sound rules while the pronouncing dictionary loads; it still sings. Check the VOLUME knob and the track level. |
| Crackles or tempo skips | Choose **LITE** on PLAY (top right). On a first-generation MPC One, Std and High exceed the CPU budget, and Eco has little headroom. Close other heavy plugins, or raise the audio buffer (Preferences → Audio). The CPU readout on PLAY shows average and peak. |
| CPU readout doesn't change | It is not pushed to the screen; it updates when the MPC redraws the page (switch pages and back). |
| Words sung out of order or doubled | Turn off any arpeggiator or note repeat on the controller. One was the cause of an early "bug" report. |
| Lyric box doesn't follow the notes | **LIVE LYRICS: ON** (PLAY). It refreshes at most 5 times a second, so very short notes can be skipped. |
| Head doesn't move | **ANIMATION: ON** (head panel). It is off by default. |
| A project opens with the keyboard showing | Tap **CANCEL** or **DONE**. The panel's open state is saved with the project. |
| An old project sings `lyrics.txt` | Projects saved on "File" keep it. The position readout says `FILE ...`. Type words and tap **DONE** to switch to typed words. |
| Building: `CHIMERA fork not found` | Run `python scripts/bootstrap.py` (or `scripts/build.py`); check `git submodule update --init` ran. |
| Building: a patch "neither applies nor is applied" | The submodule has local changes. Inspect with `git -C external/<name> status`. `git -C external/<name> checkout -- .` discards them (it does not touch `plugin/` or `engine/`). |
| Building: `cargo` or a Rust target missing | Install Rust from https://rustup.rs and add the targets listed in [building.md](building.md). |
| Building on Linux/macOS: native link fails | `plugin/link.json` lists Windows system libraries for the native test build (see [building.md](building.md)). |
