<p align="center">
  <img src="ui/logo.png" alt="The Aegis’s Oppai · XC2S2 Edition" width="320">
</p>

<h1 align="center">The Aegis’s Oppai · XC2S2 Edition</h1>

<p align="center">A Linux launcher for the Xenoblade Chronicles 2 – Nintendo Switch 2 Edition static recompilation</p>

A Linux launcher (one AppImage for every distribution) for the **Xenoblade Chronicles 2 – Nintendo Switch 2 Edition** static
recompilation, [xc2-ns2-recomp](https://github.com/weile10000/xc2-ns2-recomp). It builds the game on your PC from **your own
dump**, keeps several installs ("instances") side by side, and starts the game through **Wine** (TAAU, AMD FSR 3.1) or **Proton**
(real NVIDIA DLSS). Interface in English and Latin American Spanish.

> The recompiled game is a Windows program (`xc2_boot.exe`): this launcher runs it through Wine/Proton. A native Linux runtime may
> come later; it is not part of this release.

## What it has

- **One AppImage** built on Ubuntu 22.04: runs on Ubuntu/Kubuntu 22.04 and newer, Mint, Pop!_OS, Arch, CachyOS, Manjaro,
  Fedora… No `libfuse2` needed (static runtime), no `curl` needed (its own HTTPS client).
- **First run / "Build another version"**: pick your `.nup`, or a folder with `exefs/` + `romfs/` (it searches every subfolder),
  pick the port's release (downloaded from GitHub, or a `.zip` you already have) and an install folder. *Set up* then runs the
  port's own setup engine (`engine/xc2_setup.py`, with the package's Python under Wine) to translate the game code, builds
  `xc2_boot.exe` with **native Linux zig 0.17.0** (downloaded from ziglang.org, SHA-256 checked) and translates the shaders.
- **Play**: exactly what the hand-made `jugar-xc2.sh` did — the runtime's `NXRT_*` switches from `launch.env`, Wine for
  TAAU/FSR, Proton (proton-cachyos, GE-Proton or Valve's; Arch, Ubuntu and Flatpak Steam folders) for DLSS with `UMU_ID=0`,
  the exe's absolute path and `NXRT_DIAG_WATCHDOG_SECONDS=600`; `gamemoderun` when installed.
- **Game settings on the main page**: upscaler (TAAU / FSR 3.1 / DLSS) and FSR sharpness, output resolution (every mode your
  monitor offers, via `kscreen-doctor` or `xrandr`), 3D render scale (×1 / ×1.5 / ×2), windowed / borderless, window size,
  internal resolution locked at 720p, the port's own **FPS counter** ("FPS nn" in the corner you choose), **controller**
  (only the one you pick is seen by the game), game language. Stored in `<instance>/opciones.conf`.
- **NVIDIA DLSS**: the launcher shows NVIDIA's licenses (NGX SDK and Streamline) and only after you tick "I accept" downloads
  Streamline **v2.14.16** from NVIDIA's own GitHub release, extracts the four DLLs into the instance and deletes the zip.
- **AMD FSR 3.1**: same flow with AMD's license, the two signed DLLs from the FidelityFX SDK **v2.4.0** repository.
- **Saves**: import from Lichii (or Eden, Citron, Sudachi, Suyu, yuzu) or from any folder (any depth; the newest copy wins),
  always after a backup of the current saves; make a backup; open the folder.
- **Instances**: add an existing install folder, build another version, set the default, check for updates of the port and of
  the launcher (channel stable / preview); "Update" downloads the newer package over the instance and rebuilds it.
- Fully responsive window (720×520 to 4K) with its own title bar, maximize/restore, and a "safe drawing" switch for drivers
  where WebKit's window stays blank. Registers itself in the applications menu so the task bar knows its window.

## What it does **not** have (on purpose)

- **No game files**, nothing made from them, and **no game art**: everything is built on your PC from your own dump.
- **No NVIDIA or AMD files**: they are fetched from the vendors' repositories only after you accept their license, and stay in
  that instance. Do not share an instance folder that contains them.
- No keys, firmware, or anything to obtain the game.

## Status

Tested on CachyOS (KDE Plasma, Wayland), RTX 3070, 4K TV, with an install that had been set up by hand before:

| | Status |
|---|---|
| AppImage builds (Ubuntu 22.04 container) and opens on CachyOS | ✅ verified |
| Adding an existing install: version, saves, DLSS/FSR files are read | ✅ verified |
| **Play** with Proton + DLSS: same variables as `jugar-xc2.sh`, Streamline 2.14.16 starts, DLSS runs | ✅ verified |
| Monitor resolutions and controller list | ✅ detected on the test PC |
| Unit tests (`cargo test`, 11 tests: launch plan, options file, saves search, ini edits, parsers) | ✅ pass |
| **Set up from a `.nup` inside the launcher** (package download → lift → native zig link → shaders) | ⚠️ not run end to end yet; each step is the one used for the hand-made install |
| Downloading DLSS / FSR from the launcher | ⚠️ URLs and zip contents checked, full flow not run yet |
| Opening on Ubuntu / Kubuntu | ⚠️ built for it, not yet run on a real Ubuntu PC |
| FPS counter showing only "FPS nn" (`controls.ini` `[fps] detail = 0`) | ⚠️ value read from the runtime, not yet seen in game |
| Picking one controller (`SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT`) under Wine/Proton | ⚠️ not tested in game |
| Launcher self-update | ⚠️ not tested (needs a second release) |
| Native Linux runtime (no Wine/Proton) | ❌ not started |

Reports are welcome: open an issue with your distribution, GPU and driver, and `launcher-setup.log` /
`pkg/work/logs/launcher-stdout.log` from the instance folder. **Never attach game files.**

## What you need to play

| | Arch / CachyOS | Ubuntu / Kubuntu |
|---|---|---|
| Wine (TAAU, FSR 3.1) | `sudo pacman -S wine` | `sudo apt install wine` (or WineHQ in `/opt/wine-stable`) |
| Proton (DLSS) | `proton-cachyos` or GE-Proton | Steam + GE-Proton in `~/.steam/steam/compatibilitytools.d` |
| Optional | `gamemode` | `gamemode` |

Plus a Vulkan 1.3 GPU driver and your own dump of the game (version 3.0.1).

## Using it

```bash
chmod +x The-Aegis-Oppai-XC2S2-*-x86_64.AppImage
./The-Aegis-Oppai-XC2S2-*-x86_64.AppImage
```

An instance folder has the same layout as a hand-made install, so an existing one can be added as it is:

```
<instance>/pkg/             the port's release package (XC2-package-dist-<version>.zip) and what Set up builds in it
<instance>/prefix/          Wine prefix                 <instance>/proton-pfx/   Proton prefix (DLSS)
<instance>/opciones.conf    the game settings           <instance>/.aegis-oppai.json   version, channel, dump it came from
```

## Building

**The release AppImage** (one file for every distribution) is built in an Ubuntu 22.04 container:

```bash
sudo pacman -S --needed podman        # or docker
scripts/appimage-ubuntu.sh            # -> build/The-Aegis-Oppai-XC2S2-<version>-x86_64.AppImage
```

Pushing a tag `v*` makes the GitHub workflow build it and attach it to that release. For a quick local build (it only runs on
distributions as new as yours): `webkit2gtk-4.1` + `cargo install tauri-cli --version "^2" --locked` + `scripts/appimage.sh`.
`cd src-tauri && cargo test` runs the tests; `ui/` can be opened in a browser without Tauri (`ui/mock.js`, `?demo=main|first|build`).

Code: `src-tauri/src` (Rust, Tauri 2: `launch.rs` starts the game, `compile.rs` is Set up, `net.rs` downloads and vendor files,
`instance.rs` options and saves, `hw.rs` monitors and controllers) and `ui/` (plain HTML/CSS/JS, no build step).

## License

GPL-3.0-or-later, like the port's own launcher. The *The Aegis’s Oppai* emblem is `assets/logo-full.png`; `icon/make_icon.py`
makes the icons from it. Xenoblade Chronicles is a trademark of its owners; this project is not affiliated with them or with
Nintendo.
