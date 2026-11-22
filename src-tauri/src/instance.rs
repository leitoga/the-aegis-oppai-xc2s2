// SPDX-License-Identifier: GPL-3.0-or-later
//! An instance is one install folder, laid out like the hand-made one this launcher grew from:
//!
//! ```text
//! <instance>/pkg/            the release package (XC2-package-dist-<version>.zip), unzipped; Set up builds xc2_boot.exe here
//! <instance>/prefix/         the Wine prefix (TAAU, FSR 3.1)
//! <instance>/proton-pfx/     Proton's prefix (NVIDIA DLSS)
//! <instance>/opciones.conf   the player's options (VAR=value lines, read by bash too)
//! <instance>/cover.jpg       the game's icon, taken from the player's own dump (optional; xc2-icono.jpg is read as well)
//! <instance>/.aegis-oppai.json   what the launcher knows about how it was built
//! ```

use crate::util::{self, mtime, tilde};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// The save account folder the runtime serves `save:` from (pkg/save/account/<uid>/).
pub const SAVE_UID: &str = "0100000000000000635232534e324358";
/// XC2's title id: the save folder emulators use.
pub const TITLE_ID: &str = "0100E95004038000";

pub fn pkg(inst: &Path) -> PathBuf {
    inst.join("pkg")
}

/// What `.aegis-oppai.json` holds.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Marker {
    /// release tag the package came from ("v0.1.4")
    pub tag: String,
    /// "estable" or "preliminar"
    pub channel: String,
    /// the dump it was built from (a .nup, or a folder)
    pub source: String,
    /// seconds since the epoch of the last finished Set up
    pub built: u64,
}

pub fn read_marker(inst: &Path) -> Marker {
    fs::read_to_string(inst.join(".aegis-oppai.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn write_marker(inst: &Path, m: &Marker) -> Result<(), String> {
    util::write_atomic(&inst.join(".aegis-oppai.json"), &serde_json::to_string_pretty(m).unwrap_or_default())
}

/// The package's version, from the first line of its RELEASE-NOTES.md ("# XC2 NS2 Recomp v0.1.4 — ..."), else README.txt.
pub fn package_version(inst: &Path) -> Option<String> {
    let p = pkg(inst);
    let from = |text: String| -> Option<String> {
        text.lines().take(6).find_map(|l| {
            l.split_whitespace().find_map(|w| {
                let w = w.trim_start_matches('v').trim_end_matches(|c: char| !c.is_ascii_digit());
                let parts: Vec<&str> = w.split('.').collect();
                (parts.len() >= 2 && parts.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))).then(|| w.to_string())
            })
        })
    };
    fs::read_to_string(p.join("RELEASE-NOTES.md")).ok().and_then(from).or_else(|| fs::read_to_string(p.join("README.txt")).ok().and_then(from))
}

/// The player's options. Field names are the keys of opciones.conf.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[allow(non_snake_case)]
pub struct Options {
    /// window size: "auto" or WxH
    pub VENTANA: String,
    /// "windowed" or "borderless"
    pub MODO: String,
    /// lock the internal render size at 720p: "0" / "1"
    pub BLOQUEAR: String,
    /// MangoHud FPS counter: "0" / "1"
    pub FPS: String,
    /// the game's output resolution: 1920x1080, 2560x1440, 3840x2160
    pub SALIDA: String,
    /// 3D render scale: 1, 1.5, 2
    pub ESCALA: String,
    /// taau, fsr3, dlss
    pub REESCALADO: String,
    /// FSR 3.1 RCAS sharpness 0..1
    pub NITIDEZ: String,
    /// the game's language (NXRT_SYSTEM_LANGUAGE)
    pub IDIOMA: String,
    /// the only controller the game sees: "auto" (all) or "vvvv:pppp"
    pub MANDO: String,
    /// where the port's own frame rate counter goes: top_left, top_right, bottom_left, bottom_right
    pub ESQUINA: String,
    /// the port's in-game menu (F1, or minus+plus on the pad): "0" / "1"
    pub MENU: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            VENTANA: "auto".into(),
            MODO: "windowed".into(),
            BLOQUEAR: "0".into(),
            FPS: "0".into(),
            SALIDA: "2560x1440".into(),
            ESCALA: "1".into(),
            REESCALADO: "taau".into(),
            NITIDEZ: "0.88".into(),
            IDIOMA: "LatinAmericanSpanish".into(),
            MANDO: "auto".into(),
            ESQUINA: "top_left".into(),
            MENU: "0".into(),
        }
    }
}

pub fn read_options(inst: &Path) -> Options {
    let mut o = Options::default();
    let Ok(text) = fs::read_to_string(inst.join("opciones.conf")) else { return o };
    for line in text.lines() {
        let Some((k, v)) = line.trim().split_once('=') else { continue };
        let v = v.trim().trim_matches('"').to_string();
        match k.trim() {
            "VENTANA" => o.VENTANA = v,
            "MODO" => o.MODO = v,
            "BLOQUEAR" => o.BLOQUEAR = v,
            "FPS" => o.FPS = v,
            "SALIDA" => o.SALIDA = v,
            "ESCALA" => o.ESCALA = v,
            "REESCALADO" => o.REESCALADO = v,
            "NITIDEZ" => o.NITIDEZ = v,
            "IDIOMA" => o.IDIOMA = v,
            "MANDO" => o.MANDO = v,
            "ESQUINA" => o.ESQUINA = v,
            "MENU" => o.MENU = v,
            _ => {}
        }
    }
    o
}

fn safe(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ':')
}

pub fn write_options(inst: &Path, o: &Options) -> Result<(), String> {
    // the file is sourced by bash (jugar-xc2.sh): only plain words go in
    let fields = [
        ("VENTANA", &o.VENTANA), ("MODO", &o.MODO), ("BLOQUEAR", &o.BLOQUEAR), ("FPS", &o.FPS), ("SALIDA", &o.SALIDA),
        ("ESCALA", &o.ESCALA), ("REESCALADO", &o.REESCALADO), ("NITIDEZ", &o.NITIDEZ), ("IDIOMA", &o.IDIOMA),
        ("MANDO", &o.MANDO),
        ("ESQUINA", &o.ESQUINA),
        ("MENU", &o.MENU),
    ];
    let mut text = String::new();
    for (k, v) in fields {
        if !safe(v) {
            return Err(format!("{k}: invalid value"));
        }
        text.push_str(&format!("{k}={v}\n"));
    }
    util::write_atomic(&inst.join("opciones.conf"), &text)
}

/// Which upscaler files an instance has (never shipped by the launcher: downloaded after the vendor's license).
pub fn has_dlss(inst: &Path) -> bool {
    let d = pkg(inst).join("upscalers/streamline");
    ["sl.interposer.dll", "sl.common.dll", "sl.dlss.dll", "nvngx_dlss.dll"].iter().all(|f| d.join(f).is_file())
}

pub fn has_fsr(inst: &Path) -> bool {
    let d = pkg(inst).join("upscalers/fsr");
    ["amd_fidelityfx_loader_vk.dll", "amd_fidelityfx_upscaler_vk.dll"].iter().all(|f| d.join(f).is_file())
}

/// The cover image in the instance (taken from the player's dump), if any.
pub fn cover_path(inst: &Path) -> Option<PathBuf> {
    ["cover.jpg", "cover.png", "xc2-icono.jpg"].iter().map(|n| inst.join(n)).find(|p| p.is_file())
}

/// Set up is finished: the exe is built and the setup engine wrote its "finished" mark.
pub fn ready(inst: &Path) -> bool {
    let p = pkg(inst);
    p.join("xc2_boot.exe").is_file()
        && p.join("launch.env").is_file()
        && fs::read_to_string(p.join("setup_state.tsv")).map(|t| t.lines().any(|l| l.starts_with("meta\tfinished"))).unwrap_or(false)
}

/// What the UI shows for one instance.
#[derive(Serialize, Clone)]
pub struct Info {
    pub path: String,
    pub display: String,
    pub exists: bool,
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub built: Option<u64>,
    pub ready: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub language: String,
    pub has_cover: bool,
    pub upscaler: String,
}

pub fn info(inst: &Path) -> Info {
    let m = read_marker(inst);
    let o = read_options(inst);
    let version = package_version(inst).unwrap_or_default();
    let built = if m.built > 0 { Some(m.built) } else { mtime(&pkg(inst).join("xc2_boot.exe")) };
    Info {
        path: inst.display().to_string(),
        display: tilde(inst),
        exists: pkg(inst).is_dir(),
        tag: if m.tag.is_empty() && !version.is_empty() { format!("v{version}") } else { m.tag.clone() },
        channel: if m.channel.is_empty() { "estable".into() } else { m.channel.clone() },
        version,
        built,
        ready: ready(inst),
        dlss: has_dlss(inst),
        fsr: has_fsr(inst),
        language: o.IDIOMA.clone(),
        has_cover: cover_path(inst).is_some(),
        upscaler: o.REESCALADO,
    }
}

/// Whether a folder can be added as an instance: it has pkg/ with the setup engine (built or not).
pub fn looks_like_instance(p: &Path) -> bool {
    pkg(p).join("engine/xc2_setup.py").is_file() || pkg(p).join("xc2_boot.exe").is_file()
}

// ---- saves ----

pub fn account_dir(inst: &Path) -> PathBuf {
    pkg(inst).join("save/account").join(SAVE_UID)
}

#[derive(Serialize)]
pub struct SaveFile {
    pub name: String,
    pub size: u64,
    pub modified: Option<u64>,
}

#[derive(Serialize)]
pub struct Saves {
    pub folder: String,
    pub files: Vec<SaveFile>,
    pub latest: Option<u64>,
    pub backups: usize,
}

pub fn list_saves(inst: &Path) -> Saves {
    let dir = account_dir(inst);
    let mut files: Vec<SaveFile> = fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("sav")))
                .map(|e| SaveFile {
                    name: e.file_name().to_string_lossy().into_owned(),
                    size: e.metadata().map(|m| m.len()).unwrap_or(0),
                    modified: mtime(&e.path()),
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    let backups = fs::read_dir(dir.parent().unwrap_or(&dir))
        .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{SAVE_UID}.respaldo-"))).count())
        .unwrap_or(0);
    Saves { folder: dir.display().to_string(), latest: files.iter().filter_map(|f| f.modified).max(), files, backups }
}

/// A copy of the account folder next to it, `<uid>.respaldo-<seconds>`, made before anything is written into it.
pub fn backup_saves(inst: &Path) -> Result<Option<PathBuf>, String> {
    let dir = account_dir(inst);
    if !dir.is_dir() {
        return Ok(None);
    }
    let dest = dir.with_file_name(format!("{SAVE_UID}.respaldo-{}", util::now()));
    fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    for e in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), dest.join(e.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(Some(dest))
}

/// Every XC2 save under `src`, at any depth (symlinks not followed). When the same file name shows up more than once (copies,
/// several emulator profiles), the newest one wins.
pub fn find_saves(src: &Path) -> Vec<PathBuf> {
    let mut best: std::collections::BTreeMap<String, (u64, PathBuf)> = std::collections::BTreeMap::new();
    let mut stack = vec![(src.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if depth < 24 {
                    stack.push((p, depth + 1));
                }
            } else if ft.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sav")) {
                let name = e.file_name().to_string_lossy().to_lowercase();
                let t = mtime(&p).unwrap_or(0);
                if best.get(&name).is_none_or(|(bt, _)| t > *bt) {
                    best.insert(name, (t, p));
                }
            }
        }
    }
    best.into_values().map(|(_, p)| p).collect()
}

/// Copies the saves found under `src` (any depth) into the account folder, after a backup. Returns (copied, backup).
pub fn import_saves(inst: &Path, src: &Path) -> Result<(usize, Option<PathBuf>), String> {
    let found = find_saves(src);
    if found.is_empty() {
        return Err(format!("no .sav: {}", src.display()));
    }
    let backup = backup_saves(inst)?;
    let dest = account_dir(inst);
    fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut copied = 0;
    for f in &found {
        if let Some(name) = f.file_name() {
            if fs::copy(f, dest.join(name)).is_ok() {
                copied += 1;
            }
        }
    }
    Ok((copied, backup))
}

/// Emulator save folders for XC2 on this PC: `<emu>/nand/user/save/0000000000000000/<profile>/<title id>`.
#[derive(Serialize)]
pub struct EmuSave {
    pub emulator: String,
    pub path: String,
    pub files: usize,
    pub modified: Option<u64>,
}

pub fn emulator_saves() -> Vec<EmuSave> {
    let share = util::home().join(".local/share");
    let mut out = Vec::new();
    for (name, dir) in [("Lichii", "lichii"), ("Eden", "eden"), ("Citron", "citron"), ("Sudachi", "sudachi"), ("Suyu", "suyu"), ("yuzu", "yuzu")] {
        let root = share.join(dir).join("nand/user/save/0000000000000000");
        let Ok(rd) = fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let p = e.path().join(TITLE_ID);
            if !p.is_dir() {
                continue;
            }
            let saves: Vec<PathBuf> = fs::read_dir(&p)
                .map(|r| r.flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x == "sav")).collect())
                .unwrap_or_default();
            if saves.is_empty() {
                continue;
            }
            out.push(EmuSave {
                emulator: name.into(),
                path: p.display().to_string(),
                files: saves.len(),
                modified: saves.iter().filter_map(|s| mtime(s)).max(),
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_round_trip_and_bash_format() {
        let d = std::env::temp_dir().join(format!("xc2l-opt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("opciones.conf"), "VENTANA=auto\nMODO=borderless\nBLOQUEAR=0\nFPS=1\nSALIDA=3840x2160\nESCALA=1\nREESCALADO=dlss\nNITIDEZ=1\n").unwrap();
        let o = read_options(&d);
        assert_eq!(o.MODO, "borderless");
        assert_eq!(o.REESCALADO, "dlss");
        assert_eq!(o.IDIOMA, "LatinAmericanSpanish", "absent key keeps the default");
        write_options(&d, &o).unwrap();
        assert_eq!(read_options(&d), o);
        let mut bad = o.clone();
        bad.VENTANA = "$(rm -rf ~)".into();
        assert!(write_options(&d, &bad).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn saves_found_at_any_depth_newest_wins() {
        let d = std::env::temp_dir().join(format!("xc2l-sav-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let deep = d.join("a/b/c/d/e/f/g");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(d.join("old")).unwrap();
        fs::write(d.join("old/bf2savefile.sav"), b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(deep.join("bf2savefile.sav"), b"new").unwrap();
        fs::write(deep.join("notes.txt"), b"x").unwrap();
        let f = find_saves(&d);
        assert_eq!(f.len(), 1);
        assert_eq!(fs::read(&f[0]).unwrap(), b"new");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn version_from_release_notes() {
        let d = std::env::temp_dir().join(format!("xc2l-ver-{}", std::process::id()));
        fs::create_dir_all(d.join("pkg")).unwrap();
        fs::write(d.join("pkg/RELEASE-NOTES.md"), "# XC2 NS2 Recomp v0.1.4 — the frame on screen\n").unwrap();
        assert_eq!(package_version(&d).as_deref(), Some("0.1.4"));
        let _ = fs::remove_dir_all(&d);
    }
}
