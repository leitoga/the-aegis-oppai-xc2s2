// SPDX-License-Identifier: GPL-3.0-or-later
//! Network: the port's releases on GitHub, downloads (with the launcher's own HTTPS client), and the vendor upscaler
//! files. NVIDIA's and AMD's files are never part of the launcher: they are fetched from the vendors' own repositories, only
//! after the player accepted the license shown first, into the instance that uses them.

use crate::util::{self, host_cmd};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The recompilation's own repository (its releases are the package zips).
pub const GAME_REPO: &str = match option_env!("XC2L_GAME_REPO") {
    Some(r) => r,
    None => "weile10000/xc2-ns2-recomp",
};
/// This launcher's repository, for its own updates. Empty: no launcher update check.
pub const LAUNCHER_REPO: &str = match option_env!("XC2L_REPO") {
    Some(r) => r,
    None => "",
};

pub const STREAMLINE_TAG: &str = "v2.14.16";
pub const FFX_TAG: &str = "v2.4.0";
pub const ZIG_VERSION: &str = "0.17.0";
const ZIG_SHA256: &str = "1cbe9df9f27e6b78d14ccbca43b6703a404ef79ef1c463de901d7f088d4e2026";

/// HTTPS with the launcher's own client (rustls): nothing on the PC is needed (Kubuntu does not always have curl).
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent("aegis-oppai-launcher")
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
}

pub fn get_text(url: &str) -> Result<String, String> {
    let r = agent().get(url).set("Accept", "application/vnd.github+json").call().map_err(|e| format!("{url}: {e}"))?;
    r.into_string().map_err(|e| format!("{url}: {e}"))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub name: String,
    pub prerelease: bool,
    pub date: String,
    pub notes: String,
    pub url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub sha256: String,
}

fn parse_releases(text: &str, asset: &dyn Fn(&str) -> bool) -> Result<Vec<Release>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let arr = v.as_array().ok_or_else(|| v["message"].as_str().unwrap_or("GitHub").to_string())?;
    Ok(arr
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .map(|r| {
            let assets = r["assets"].as_array().cloned().unwrap_or_default();
            let a = assets.iter().find(|a| a["name"].as_str().is_some_and(asset)).cloned().unwrap_or(serde_json::Value::Null);
            let tag = r["tag_name"].as_str().unwrap_or_default().to_string();
            Release {
                version: tag.trim_start_matches('v').to_string(),
                name: r["name"].as_str().unwrap_or_default().to_string(),
                prerelease: r["prerelease"].as_bool().unwrap_or(false),
                date: r["published_at"].as_str().unwrap_or_default().to_string(),
                notes: r["body"].as_str().unwrap_or_default().to_string(),
                url: r["html_url"].as_str().unwrap_or_default().to_string(),
                asset_name: a["name"].as_str().unwrap_or_default().to_string(),
                asset_url: a["browser_download_url"].as_str().unwrap_or_default().to_string(),
                asset_size: a["size"].as_u64().unwrap_or(0),
                sha256: a["digest"].as_str().unwrap_or_default().trim_start_matches("sha256:").to_string(),
                tag,
            }
        })
        .collect())
}

/// The port's releases, newest first; each with its package zip (XC2-package-dist*.zip).
pub fn game_releases() -> Result<Vec<Release>, String> {
    let t = get_text(&format!("https://api.github.com/repos/{GAME_REPO}/releases?per_page=30"))?;
    parse_releases(&t, &|n| n.starts_with("XC2-package-dist") && n.ends_with(".zip"))
}

pub fn launcher_releases() -> Result<Vec<Release>, String> {
    if LAUNCHER_REPO.is_empty() {
        return Ok(Vec::new());
    }
    let t = get_text(&format!("https://api.github.com/repos/{LAUNCHER_REPO}/releases?per_page=10"))?;
    parse_releases(&t, &|n| n.ends_with("x86_64.AppImage"))
}

/// "0.1.10" > "0.1.9"
pub fn newer(a: &str, b: &str) -> bool {
    let p = |s: &str| -> Vec<u64> { s.trim_start_matches('v').split(['.', '-']).map(|x| x.parse().unwrap_or(0)).collect() };
    p(a) > p(b)
}

/// Downloads `url` to `dest` (through `dest.part`), reporting (done, total). Cancelled when `cancel` is set.
pub fn download(url: &str, dest: &Path, total: u64, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    use std::io::Write;
    if let Some(d) = dest.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let part = dest.with_extension("part");
    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let resp = match agent().get(url).call() {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("{url}: {e}");
                continue;
            }
        };
        let size = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(total);
        let mut reader = resp.into_reader();
        let mut out = fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut buf = vec![0u8; 1 << 16];
        let mut got = 0u64;
        let mut shown = std::time::Instant::now();
        let res: Result<(), String> = loop {
            if cancel.load(Ordering::SeqCst) {
                drop(out);
                let _ = fs::remove_file(&part);
                return Err("cancelled".into());
            }
            match reader.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    out.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", part.display()))?;
                    got += n as u64;
                    if shown.elapsed().as_millis() > 250 {
                        progress(got, size);
                        shown = std::time::Instant::now();
                    }
                }
                Err(e) => break Err(format!("{url}: {e}")),
            }
        };
        match res {
            Ok(()) if size == 0 || got == size => {
                progress(got, size.max(got));
                drop(out);
                return fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()));
            }
            Ok(()) => last_err = format!("{url}: {got} of {size} bytes"),
            Err(e) => last_err = e,
        }
    }
    let _ = fs::remove_file(&part);
    Err(last_err)
}

pub fn sha256(p: &Path) -> Result<String, String> {
    let o = host_cmd("sha256sum").arg(p).output().map_err(|e| format!("sha256sum: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or_default().to_string())
}

/// Extracts the zip's entries into `dest`. `pick` maps an entry name to where it goes (None: skipped).
pub fn unzip(zip: &Path, dest: &Path, pick: &dyn Fn(&str) -> Option<PathBuf>, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> Result<usize, String> {
    let f = fs::File::open(zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("{}: {e}", zip.display()))?;
    let n = z.len() as u64;
    let mut written = 0;
    for i in 0..z.len() {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = e.enclosed_name() else { continue };
        let Some(rel) = pick(&name.to_string_lossy()) else { continue };
        let out = dest.join(rel);
        if e.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            continue;
        }
        if let Some(d) = out.parent() {
            fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let mut w = fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::io::copy(&mut e, &mut w).map_err(|e| format!("{}: {e}", out.display()))?;
        if let Some(mode) = e.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777 | 0o600));
        }
        written += 1;
        if i % 200 == 0 {
            progress(i as u64, n);
        }
    }
    progress(n, n);
    Ok(written)
}

// ---- vendor upscalers ----

pub struct Vendor {
    pub id: &'static str,
    /// the license(s) shown before anything is fetched: (title, url)
    pub licenses: &'static [(&'static str, &'static str)],
}

pub const DLSS: Vendor = Vendor {
    id: "dlss",
    licenses: &[
        ("NVIDIA DLSS (NGX SDK) — nvngx_dlss.dll", "https://raw.githubusercontent.com/NVIDIA-RTX/Streamline/v2.14.16/external/ngx-sdk/license.txt"),
        ("NVIDIA Streamline — sl.*.dll", "https://raw.githubusercontent.com/NVIDIA-RTX/Streamline/v2.14.16/license.txt"),
    ],
};

pub const FSR: Vendor = Vendor {
    id: "fsr",
    licenses: &[("AMD FidelityFX SDK", "https://raw.githubusercontent.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/v2.4.0/Kits/FidelityFX/docs/license.md")],
};

pub fn vendor(id: &str) -> Option<&'static Vendor> {
    match id {
        "dlss" => Some(&DLSS),
        "fsr" => Some(&FSR),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct LicenseText {
    pub title: String,
    pub url: String,
    pub text: String,
}

pub fn licenses(v: &Vendor) -> Result<Vec<LicenseText>, String> {
    v.licenses.iter().map(|(t, u)| Ok(LicenseText { title: t.to_string(), url: u.to_string(), text: get_text(u)? })).collect()
}

const STREAMLINE_FILES: [&str; 4] = ["sl.interposer.dll", "sl.common.dll", "sl.dlss.dll", "nvngx_dlss.dll"];
const FFX_FILES: [&str; 2] = ["amd_fidelityfx_loader_vk.dll", "amd_fidelityfx_upscaler_vk.dll"];

/// Fetches the vendor files into `<pkg>/upscalers/<streamline|fsr>`, with the license texts beside them.
pub fn install_vendor(v: &Vendor, pkg: &Path, cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> Result<PathBuf, String> {
    let lic = licenses(v)?;
    match v.id {
        "dlss" => {
            let dest = pkg.join("upscalers/streamline");
            let name = format!("streamline-sdk-{STREAMLINE_TAG}.zip");
            let url = format!("https://github.com/NVIDIA-RTX/Streamline/releases/download/{STREAMLINE_TAG}/{name}");
            let zip = util::cache_dir().join(&name);
            if !zip.is_file() {
                download(&url, &zip, 276_016_170, cancel, &|d, t| progress("download", d, t))?;
            }
            let picked = unzip(&zip, &dest, &|n| {
                let file = n.strip_prefix("bin/x64/")?;
                (STREAMLINE_FILES.contains(&file) || file == "nvngx_dlss.license.txt").then(|| PathBuf::from(file))
            }, cancel, &|d, t| progress("extract", d, t))?;
            let _ = fs::remove_file(&zip);
            if picked < STREAMLINE_FILES.len() {
                return Err(format!("{name}: missing files"));
            }
            for (i, l) in lic.iter().enumerate() {
                let _ = fs::write(dest.join(format!("LICENSE-{i}.txt")), &l.text);
            }
            Ok(dest)
        }
        _ => {
            let dest = pkg.join("upscalers/fsr");
            fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            for (i, f) in FFX_FILES.iter().enumerate() {
                let url = format!("https://raw.githubusercontent.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/{FFX_TAG}/Kits/FidelityFX/signedbin/{f}");
                let total = if i == 0 { 39_184 } else { 36_135_696 };
                download(&url, &dest.join(f), total, cancel, &|d, t| progress("download", d, t))?;
            }
            let _ = fs::write(dest.join("LICENSE.md"), &lic[0].text);
            Ok(dest)
        }
    }
}

// ---- zig (builds xc2_boot.exe natively; the Windows zig in the package fails under Wine) ----

/// zig for Linux: one this PC already has (the launcher's own folder, or next to an instance), else downloaded from
/// ziglang.org and checked against its published SHA-256.
pub fn zig(instances: &[PathBuf], cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> Result<PathBuf, String> {
    let dir_name = format!("zig-x86_64-linux-{ZIG_VERSION}");
    let mine = util::data_dir().join(&dir_name);
    let mut candidates = vec![mine.clone()];
    candidates.extend(instances.iter().map(|i| i.join(&dir_name)));
    if let Some(z) = candidates.iter().map(|d| d.join("zig")).find(|z| util::is_exec(z)) {
        return Ok(z);
    }
    let tarball = util::cache_dir().join(format!("{dir_name}.tar.xz"));
    let url = format!("https://ziglang.org/download/{ZIG_VERSION}/{dir_name}.tar.xz");
    if !(tarball.is_file() && sha256(&tarball)? == ZIG_SHA256) {
        let _ = fs::remove_file(&tarball);
        download(&url, &tarball, 57_332_648, cancel, &|d, t| progress("zig", d, t))?;
        if sha256(&tarball)? != ZIG_SHA256 {
            let _ = fs::remove_file(&tarball);
            return Err(format!("{url}: SHA-256 mismatch"));
        }
    }
    fs::create_dir_all(util::data_dir()).map_err(|e| e.to_string())?;
    let st = host_cmd("tar").arg("-xJf").arg(&tarball).arg("-C").arg(util::data_dir()).status().map_err(|e| format!("tar: {e}"))?;
    if !st.success() {
        return Err("tar: zig".into());
    }
    let _ = fs::remove_file(&tarball);
    let z = mine.join("zig");
    util::is_exec(&z).then_some(z).ok_or_else(|| format!("{}: not found", mine.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_numerically() {
        assert!(newer("0.1.10", "0.1.9"));
        assert!(newer("v0.2.0", "0.1.4"));
        assert!(!newer("0.1.4", "0.1.4"));
    }
    #[test]
    fn releases_pick_the_package_zip() {
        let t = r#"[{"tag_name":"v0.1.4","name":"x","prerelease":false,"draft":false,"published_at":"2026-11-20T20:29:37Z","body":"n","html_url":"u",
          "assets":[{"name":"xc2-launcher-0.1.4-src.zip","size":1,"browser_download_url":"a"},{"name":"XC2-package-dist-0.1.4.zip","size":9,"browser_download_url":"b","digest":"sha256:ff"}]}]"#;
        let r = parse_releases(t, &|n| n.starts_with("XC2-package-dist")).unwrap();
        assert_eq!(r[0].asset_url, "b");
        assert_eq!(r[0].sha256, "ff");
        assert_eq!(r[0].version, "0.1.4");
    }
}
