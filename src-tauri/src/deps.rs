// SPDX-License-Identifier: GPL-3.0-or-later
//! What the game needs besides the launcher, downloaded into the user's own folders (no password, nothing installed in the
//! system): a portable Wine 11.22, and GE-Proton for DLSS.

use crate::util::{self, host_cmd};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

// ---- Wine: one portable build, the same on every distribution ----

/// The Wine the launcher uses: Kron4ek's portable build of Wine 11.22 (WoW64, 64-bit only, no 32-bit libraries needed), the
/// version the port was set up and played with. The distribution's own Wine differs from one release to the next (Ubuntu
/// and Kubuntu ship older ones), which made Set up behave differently on each.
pub const WINE_VERSION: &str = "11.22";
const WINE_DIR: &str = "wine-11.22-amd64-wow64";
const WINE_URL: &str = "https://github.com/Kron4ek/Wine-Builds/releases/download/11.22/wine-11.22-amd64-wow64.tar.xz";
const WINE_SIZE: u64 = 98_200_812;
const WINE_SHA256: &str = "e2967393d203121195cba94159efe9b575a6fa8fad2815c557036e69285093d8";

/// The launcher's own Wine, when it is there.
pub fn own_wine() -> Option<PathBuf> {
    let w = util::data_dir().join(WINE_DIR).join("bin/wine");
    util::is_exec(&w).then_some(w)
}

/// Downloads the portable Wine (SHA-256 checked) into the launcher's folder. Returns its `wine`.
pub fn install_wine(cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> Result<PathBuf, String> {
    if let Some(w) = own_wine() {
        return Ok(w);
    }
    let tarball = util::cache_dir().join(format!("{WINE_DIR}.tar.xz"));
    if !(tarball.is_file() && crate::net::sha256(&tarball)? == WINE_SHA256) {
        let _ = fs::remove_file(&tarball);
        crate::net::download(WINE_URL, &tarball, WINE_SIZE, cancel, &|d, t| progress("download", d, t))?;
        if crate::net::sha256(&tarball)? != WINE_SHA256 {
            let _ = fs::remove_file(&tarball);
            return Err(format!("{WINE_URL}: SHA-256 mismatch"));
        }
    }
    progress("extract", 0, 0);
    fs::create_dir_all(util::data_dir()).map_err(|e| e.to_string())?;
    let st = host_cmd("tar").arg("-xJf").arg(&tarball).arg("-C").arg(util::data_dir()).status().map_err(|e| format!("tar: {e}"))?;
    let _ = fs::remove_file(&tarball);
    if !st.success() {
        return Err("tar: wine".into());
    }
    own_wine().ok_or_else(|| format!("{}: not found", util::data_dir().join(WINE_DIR).display()))
}

/// Every Wine call of the launcher: no debug output, no menu entries, no crash window (a crash ends the program, and the
/// launcher reports it), no Gecko. Mono stays enabled: the port's .NET tools load their DLLs through mscoree.
pub const WINE_OVERRIDES: &str = "mshtml=;winemenubuilder.exe=d;winedbg.exe=d";

/// The prefix, created or brought up to date before anything else runs in it, without any window: no display is given to
/// wineboot (so neither "the configuration is being updated" nor "install Wine Mono / Gecko" can show), and the crash
/// window is turned off in it. Remembers the Wine it was prepared with, so this runs again only when that changes.
pub fn prepare_prefix(wine: &Path, prefix: &Path) -> Result<(), String> {
    let stamp = prefix.join(".aegis-wine");
    // the stamp names the settings too: a prefix prepared before the controller settings is prepared again
    let id = format!("{}|pads-1", wine.display());
    if prefix.join("system.reg").is_file() && fs::read_to_string(&stamp).map(|s| s == id).unwrap_or(false) {
        return Ok(());
    }
    fs::create_dir_all(prefix).map_err(|e| format!("{}: {e}", prefix.display()))?;
    let quiet = |c: &mut std::process::Command| {
        c.env("WINEPREFIX", prefix)
            .env("WINEDEBUG", "-all")
            .env("WINEDLLOVERRIDES", "mscoree,mshtml=;winemenubuilder.exe=d;winedbg.exe=d")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    };
    let mut c = host_cmd(wine);
    c.args(["wineboot", "-u"]);
    quiet(&mut c);
    let st = c.status().map_err(|e| format!("wineboot: {e}"))?;
    if !st.success() {
        return Err(format!("wineboot: exit {}", st.code().unwrap_or(-1)));
    }
    let reg = |args: &[&str]| {
        let mut c = host_cmd(wine);
        c.arg("reg").arg("add").args(args).arg("/f");
        quiet(&mut c);
        let _ = c.status();
    };
    reg(&["HKCU\\Software\\Wine\\WineDbg", "/v", "ShowCrashDialog", "/t", "REG_DWORD", "/d", "0"]);
    // Controllers: Wine reads them through the host's SDL2 library, which Ubuntu/Kubuntu do not always have; then the game
    // saw no pad at all although the launcher listed it. Wine's own Linux paths (evdev and hidraw) are turned on as well.
    let bus = "HKLM\\System\\CurrentControlSet\\Services\\winebus";
    reg(&[bus, "/v", "DisableInput", "/t", "REG_DWORD", "/d", "0"]);
    reg(&[bus, "/v", "DisableHidraw", "/t", "REG_DWORD", "/d", "0"]);
    // let the wineserver of this headless session end before the next command starts one with a display
    let mut c = host_cmd(wine.with_file_name("wineserver"));
    c.arg("-w");
    quiet(&mut c);
    let _ = c.status();
    let _ = fs::write(&stamp, id);
    Ok(())
}

// ---- MangoHud: the frame rate counter ----
// The port's own counter is drawn by its overlay, which xc2_boot turns off for XC2 (po.overlay = false in xc2_boot.cpp: the
// game's keys never reach the overlay), so the launcher shows the frame rate with MangoHud instead: just the number, in a
// small dark rounded box. A system MangoHud is used as it is; otherwise the official release is downloaded once
// (SHA-256 pinned) and its 64-bit layer is loaded from the launcher's folder.

const MANGO_URL: &str = "https://github.com/flightlessmango/MangoHud/releases/download/v0.8.4/MangoHud-0.8.4.r0.g992103e.tar.gz";
const MANGO_SIZE: u64 = 10_282_346;
const MANGO_SHA256: &str = "aa0b2a009552f5e7c50baabf361267a79ea5e5c0fea440786f3b202d51f57753";
pub const MANGO_LAYER: &str = "VK_LAYER_MANGOHUD_overlay_x86_64";

pub fn system_mangohud() -> bool {
    ["/usr/share/vulkan/implicit_layer.d/MangoHud.x86_64.json", "/usr/share/vulkan/implicit_layer.d/MangoHud.json", "/etc/vulkan/implicit_layer.d/MangoHud.x86_64.json"]
        .iter()
        .any(|p| Path::new(p).is_file())
        || util::home().join(".local/share/vulkan/implicit_layer.d/MangoHud.x86_64.json").is_file()
}

/// The launcher's own MangoHud layer folder (with its manifest), downloading it the first time.
pub fn own_mangohud() -> Result<PathBuf, String> {
    let dir = util::data_dir().join("mangohud-0.8.4");
    let manifest = dir.join("layer/MangoHud.x86_64.json");
    let lib = dir.join("lib64/libMangoHud.so");
    if manifest.is_file() && lib.is_file() {
        return Ok(dir.join("layer"));
    }
    let tarball = util::cache_dir().join("MangoHud-0.8.4.tar.gz");
    if !(tarball.is_file() && crate::net::sha256(&tarball)? == MANGO_SHA256) {
        let _ = fs::remove_file(&tarball);
        crate::net::download(MANGO_URL, &tarball, MANGO_SIZE, &AtomicBool::new(false), &|_, _| {})?;
        if crate::net::sha256(&tarball)? != MANGO_SHA256 {
            let _ = fs::remove_file(&tarball);
            return Err(format!("{MANGO_URL}: SHA-256 mismatch"));
        }
    }
    let work = util::cache_dir().join("mangohud-unpack");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let ok = host_cmd("tar").arg("-xzf").arg(&tarball).arg("-C").arg(&work).status().map(|s| s.success()).unwrap_or(false)
        && host_cmd("tar")
            .arg("-xf")
            .arg(work.join("MangoHud/MangoHud-package.tar"))
            .arg("-C")
            .arg(&work)
            .arg("./usr/lib/mangohud/lib64/libMangoHud.so")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    if !ok {
        return Err("tar: MangoHud".into());
    }
    fs::create_dir_all(dir.join("lib64")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("layer")).map_err(|e| e.to_string())?;
    fs::copy(work.join("usr/lib/mangohud/lib64/libMangoHud.so"), &lib).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&work);
    let _ = fs::remove_file(&tarball);
    // an explicit layer (enabled by VK_INSTANCE_LAYERS), pointing at the library in the launcher's folder
    let json = serde_json::json!({
        "file_format_version": "1.0.0",
        "layer": {
            "name": MANGO_LAYER, "type": "GLOBAL", "api_version": "1.3.0",
            "library_path": lib.display().to_string(), "implementation_version": "1",
            "description": "Vulkan Hud Overlay",
            "functions": { "vkNegotiateLoaderLayerInterfaceVersion": "vkNegotiateLoaderLayerInterfaceVersion" }
        }
    });
    util::write_atomic(&manifest, &serde_json::to_string_pretty(&json).unwrap_or_default())?;
    Ok(dir.join("layer"))
}

/// MangoHud's settings for the counter: only the number, white, in a small dark rounded box, in the chosen corner.
pub fn mango_counter(corner: &str) -> String {
    let pos = match corner {
        "top_right" => "top-right",
        "bottom_left" => "bottom-left",
        "bottom_right" => "bottom-right",
        _ => "top-left",
    };
    format!("fps_only\nfont_size=30\ntext_color=FFFFFF\nbackground_color=1F1719\nbackground_alpha=0.75\nround_corners=12\nposition={pos}\noffset_x=20\noffset_y=20\n")
}

/// The loading card MangoHud draws inside the game while its window is still black: the stage, the percentage and a text
/// bar, centred on a dark rounded box. MangoHud re-reads its file while the game runs, so the bar moves. (MangoHud 0.8 has no
/// image option: the logo is not possible here.) `#` starts a comment in MangoHud's file, so the bar is drawn with `|`.
pub fn mango_loading(title: &str, percent: u32, screen: (u32, u32)) -> String {
    let filled = (percent.min(100) as usize * 30) / 100;
    let bar = format!("[{}{}]", "|".repeat(filled), ".".repeat(30 - filled));
    let (w, h) = screen;
    let font = (h / 40).clamp(24, 64);
    let box_w = font * 20;
    let x = w.saturating_sub(box_w) / 2;
    let y = (h / 2).saturating_sub(font * 2);
    format!(
        "legacy_layout=0\ngpu_stats=0\ncpu_stats=0\nfps=0\nframe_timing=0\nframetime=0\nfont_size={font}\ntext_color=FFFFFF\nbackground_color=1F1719\nbackground_alpha=0.92\nround_corners=16\nposition=top-left\noffset_x={x}\noffset_y={y}\nwidth={box_w}\ncustom_text={title}   {percent} %\ncustom_text={bar}\n"
    )
}

/// Nothing on screen (the counter is off): MangoHud stays loaded but hides itself.
pub const MANGO_HIDDEN: &str = "no_display\n";

// ---- GE-Proton ----

/// Where a downloaded Proton goes: Steam's own compatibility tools folder when Steam is installed (Steam then offers it too),
/// else the launcher's.
pub fn proton_home() -> PathBuf {
    match crate::launch::steam_roots().into_iter().next() {
        Some(s) => s.join("compatibilitytools.d"),
        None => util::data_dir().join("compatibilitytools.d"),
    }
}

/// The newest GE-Proton release: (tag, tarball url, size, sha512 url).
fn latest_ge() -> Result<(String, String, u64, String), String> {
    let t = crate::net::get_text("https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest")?;
    let v: serde_json::Value = serde_json::from_str(&t).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().unwrap_or_default().to_string();
    let assets = v["assets"].as_array().cloned().unwrap_or_default();
    let find = |suffix: &str| assets.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(suffix) && !n.contains("aarch64"))).cloned();
    let tar = find(".tar.gz").ok_or("GE-Proton: no tarball")?;
    let sum = find(".sha512sum").ok_or("GE-Proton: no sha512sum")?;
    Ok((
        tag,
        tar["browser_download_url"].as_str().unwrap_or_default().to_string(),
        tar["size"].as_u64().unwrap_or(0),
        sum["browser_download_url"].as_str().unwrap_or_default().to_string(),
    ))
}

/// Downloads the newest GE-Proton, checks its SHA-512 and unpacks it. Returns the `proton` script.
pub fn install_proton(cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> Result<PathBuf, String> {
    let (tag, url, size, sum_url) = latest_ge()?;
    let dest = proton_home();
    if dest.join(&tag).join("proton").is_file() {
        return Ok(dest.join(&tag).join("proton"));
    }
    let tarball = util::cache_dir().join(format!("{tag}.tar.gz"));
    crate::net::download(&url, &tarball, size, cancel, &|d, t| progress("download", d, t))?;
    progress("verify", 0, 0);
    let want = crate::net::get_text(&sum_url)?.split_whitespace().next().unwrap_or_default().to_lowercase();
    let got = host_cmd("sha512sum").arg(&tarball).output().map_err(|e| e.to_string())?;
    let got = String::from_utf8_lossy(&got.stdout).split_whitespace().next().unwrap_or_default().to_lowercase();
    if want.is_empty() || want != got {
        let _ = fs::remove_file(&tarball);
        return Err(format!("{tag}: SHA-512 mismatch"));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    progress("extract", 0, 0);
    fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let st = host_cmd("tar").arg("-xzf").arg(&tarball).arg("-C").arg(&dest).status().map_err(|e| format!("tar: {e}"))?;
    let _ = fs::remove_file(&tarball);
    if !st.success() {
        return Err(format!("tar: {tag}"));
    }
    let p = dest.join(&tag).join("proton");
    p.is_file().then_some(p.clone()).ok_or_else(|| format!("{}: not found", p.display()))
}

/// The launcher's own folder for Proton builds (searched with Steam's).
pub fn own_proton_dir() -> PathBuf {
    util::data_dir().join("compatibilitytools.d")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wine_pins() {
        assert!(WINE_URL.ends_with(&format!("{WINE_DIR}.tar.xz")));
        assert_eq!(WINE_SHA256.len(), 64);
        assert!(!WINE_OVERRIDES.contains("mscoree"), "the .NET tools need mscoree when they run");
    }
}

// ---- SDL3 for Proton ----

#[cfg(bundled_sdl3)]
static SDL3: &[u8] = include_bytes!(env!("XC2L_SDL3"));
#[cfg(not(bundled_sdl3))]
static SDL3: &[u8] = &[];

/// The folder holding the launcher's own libSDL3.so.0 (written out the first time), when this build carries one.
pub fn own_sdl3() -> Option<PathBuf> {
    if SDL3.is_empty() {
        return None;
    }
    let dir = util::data_dir().join(format!("sdl3-{}", SDL3.len()));
    let lib = dir.join("libSDL3.so.0");
    if lib.metadata().map(|m| m.len() as usize) .ok() != Some(SDL3.len()) {
        fs::create_dir_all(&dir).ok()?;
        let tmp = dir.join("libSDL3.so.0.nuevo");
        fs::write(&tmp, SDL3).ok()?;
        fs::rename(&tmp, &lib).ok()?;
    }
    Some(dir)
}
