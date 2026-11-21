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
    let id = wine.display().to_string();
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
    let mut c = host_cmd(wine);
    c.args(["reg", "add", "HKCU\\Software\\Wine\\WineDbg", "/v", "ShowCrashDialog", "/t", "REG_DWORD", "/d", "0", "/f"]);
    quiet(&mut c);
    let _ = c.status();
    // let the wineserver of this headless session end before the next command starts one with a display
    let mut c = host_cmd(wine.with_file_name("wineserver"));
    c.arg("-w");
    quiet(&mut c);
    let _ = c.status();
    let _ = fs::write(&stamp, id);
    Ok(())
}

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
