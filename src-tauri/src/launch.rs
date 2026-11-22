// SPDX-License-Identifier: GPL-3.0-or-later
//! Starts xc2_boot.exe the way jugar-xc2.sh does: Wine for TAAU and FSR 3.1, Proton for NVIDIA DLSS (Proton puts the
//! driver's NGX and dxvk-nvapi in its prefix), the runtime's NXRT_* switches from launch.env plus the player's options.

use crate::instance::{self, Options};
use crate::util::{self, host_cmd, win};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const EXE: &str = "xc2_boot.exe";

/// Steam's folders on this PC: Arch/CachyOS (~/.local/share/Steam), Debian/Ubuntu (~/.steam/steam, ~/.steam/debian-installation),
/// and the Flatpak.
pub fn steam_roots() -> Vec<PathBuf> {
    let h = util::home();
    let mut v: Vec<PathBuf> = Vec::new();
    for p in [
        h.join(".local/share/Steam"),
        h.join(".steam/steam"),
        h.join(".steam/root"),
        h.join(".steam/debian-installation"),
        h.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ] {
        let real = fs::canonicalize(&p).unwrap_or(p.clone());
        if real.join("steamapps").is_dir() || real.join("compatibilitytools.d").is_dir() {
            if !v.contains(&real) {
                v.push(real);
            }
        }
    }
    v
}

/// Proton builds this PC has, best first: CachyOS's, then GE, then Valve's.
pub fn proton_candidates() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/steam/compatibilitytools.d"),
        PathBuf::from("/usr/local/share/steam/compatibilitytools.d"),
        // GE-Proton the launcher downloaded when there is no Steam
        crate::deps::own_proton_dir(),
    ];
    for s in steam_roots() {
        roots.push(s.join("compatibilitytools.d"));
        roots.push(s.join("steamapps/common"));
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for r in roots {
        let Ok(rd) = fs::read_dir(&r) else { continue };
        for e in rd.flatten() {
            let p = e.path().join("proton");
            let name = e.file_name().to_string_lossy().to_lowercase();
            let real = fs::canonicalize(&p).unwrap_or(p.clone());
            if p.is_file() && name.contains("proton") && !found.iter().any(|f| fs::canonicalize(f).unwrap_or(f.clone()) == real) {
                found.push(p);
            }
        }
    }
    let rank = |p: &PathBuf| {
        let n = p.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        if n.contains("cachyos") && n.contains("slr") { 0 } else if n.contains("cachyos") { 1 } else if n.contains("ge-proton") || n.contains("proton-ge") { 2 } else if n.contains("experimental") { 3 } else { 4 }
    };
    found.sort_by_key(rank);
    found
}

pub fn proton(setting: &str) -> Option<PathBuf> {
    let s = setting.trim();
    if !s.is_empty() {
        let p = PathBuf::from(s);
        let p = if p.is_dir() { p.join("proton") } else { p };
        return p.is_file().then_some(p);
    }
    proton_candidates().into_iter().next()
}

pub fn wine(setting: &str) -> Option<PathBuf> {
    let s = setting.trim();
    if !s.is_empty() {
        return util::is_exec(Path::new(s)).then(|| PathBuf::from(s));
    }
    // the launcher's own portable Wine 11.22 first (the same everywhere), then the distribution's, then WineHQ's (/opt/wine-*)
    crate::deps::own_wine().or_else(|| util::which("wine")).or_else(|| {
        ["/opt/wine-stable/bin/wine", "/opt/wine-staging/bin/wine", "/opt/wine-devel/bin/wine"].iter().map(PathBuf::from).find(|p| util::is_exec(p))
    })
}

#[derive(Serialize)]
pub struct Runners {
    pub wine: Option<String>,
    pub proton: Option<String>,
    pub protons: Vec<String>,
    pub gamemode: bool,
    pub mangohud: bool,
}

pub fn runners(wine_setting: &str, proton_setting: &str) -> Runners {
    Runners {
        wine: wine(wine_setting).map(|p| p.display().to_string()),
        proton: proton(proton_setting).map(|p| p.display().to_string()),
        protons: proton_candidates().iter().map(|p| p.display().to_string()).collect(),
        gamemode: util::which("gamemoderun").is_some(),
        mangohud: util::which("mangohud").is_some() || Path::new("/usr/share/vulkan/implicit_layer.d/MangoHud.x86_64.json").is_file(),
    }
}

/// launch.env: the switches the setup engine wrote for this build.
fn launch_env(pkg: &Path) -> Vec<(String, String)> {
    fs::read_to_string(pkg.join("launch.env"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// Sets `key = value` in `[section]` of an ini file, adding the key (or the section) when it is not there. Every other line is
/// kept as it is.
pub fn set_ini(text: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let mut out: Vec<String> = Vec::new();
    let mut in_sec = false;
    let mut seen_sec = false;
    let mut done = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            if in_sec && !done {
                // the section ended without the key
                while out.last().is_some_and(|x| x.trim().is_empty()) {
                    out.pop();
                }
                out.push(format!("{key} = {value}"));
                out.push(String::new());
                done = true;
            }
            in_sec = t == header;
            seen_sec |= in_sec;
        } else if in_sec && !done && t.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            out.push(format!("{key} = {value}"));
            done = true;
            continue;
        }
        out.push(l.to_string());
    }
    if !done {
        if !seen_sec {
            if out.last().is_some_and(|x| !x.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(header);
        }
        out.push(format!("{key} = {value}"));
    }
    out.join("\n") + "\n"
}

/// controls.ini: the window mode and the port's own frame rate counter (just "FPS nn", in the corner the player chose).
fn write_controls(pkg: &Path, o: &Options) {
    let path = pkg.join("controls.ini");
    let Ok(mut text) = fs::read_to_string(&path) else { return };
    let before = text.clone();
    if text.lines().any(|l| l.trim_start().starts_with("window_mode")) {
        text = text.lines().map(|l| if l.trim_start().starts_with("window_mode") && l.contains('=') { format!("window_mode = {}", o.MODO) } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    }
    let corner = match o.ESQUINA.as_str() {
        "top_right" | "bottom_left" | "bottom_right" => o.ESQUINA.as_str(),
        _ => "top_left",
    };
    text = set_ini(&text, "fps", "show", if o.FPS == "1" { "1" } else { "0" });
    text = set_ini(&text, "fps", "corner", corner);
    // 0: the frame rate only (1 adds the frame time, 2 the 1% low and the worst frame)
    text = set_ini(&text, "fps", "detail", "0");
    if text != before {
        let _ = fs::write(&path, text);
    }
}

/// What a launch would run: the program, its arguments and the variables, for the log and the tests.
pub struct Plan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub proton: bool,
    pub log: PathBuf,
}

pub fn plan(inst: &Path, o: &Options, wine_setting: &str, proton_setting: &str, stamp: u64) -> Result<Plan, String> {
    let pkg = instance::pkg(inst);
    if !pkg.join(EXE).is_file() {
        return Err("no-exe".into());
    }
    let w = win(&pkg);
    let mut env: Vec<(String, String)> = vec![
        ("WINEPREFIX".into(), inst.join("prefix").display().to_string()),
        ("WINEDEBUG".into(), "-all".into()),
    ];
    env.extend(launch_env(&pkg));
    let push = |env: &mut Vec<(String, String)>, k: &str, v: String| env.push((k.to_string(), v));
    push(&mut env, "NXRT_INI", format!("{w}\\nxrt.ini"));
    push(&mut env, "NXRT_FS_ROMFS", format!("{w}\\game\\romfs-301"));
    push(&mut env, "NXRT_FS_SAVE_DIR", format!("{w}\\save"));
    push(&mut env, "NXRT_NVN_SHADER_CACHE", format!("{w}\\shader_cache\\spirv"));
    push(&mut env, "NXRT_NVN_DUMP_MISSING", format!("{w}\\work\\missing"));
    push(&mut env, "NXRT_AOC_DIR", format!("{w}\\dlc"));
    push(&mut env, "WINEDLLOVERRIDES", format!("xc2_rt=n;{}", crate::deps::WINE_OVERRIDES));
    push(&mut env, "NXRT_SYSTEM_LANGUAGE", if o.IDIOMA.is_empty() { "LatinAmericanSpanish".into() } else { o.IDIOMA.clone() });
    if o.VENTANA != "auto" {
        if let Some((wd, ht)) = o.VENTANA.split_once('x') {
            push(&mut env, "NXRT_GFX_WINDOW_WIDTH", wd.into());
            push(&mut env, "NXRT_GFX_WINDOW_HEIGHT", ht.into());
        }
    }
    push(&mut env, "NXRT_UPSCALE_LOCK_RENDER_SIZE", o.BLOQUEAR.clone());
    push(&mut env, "NXRT_AM_RESOLUTION", o.SALIDA.clone());
    if o.ESCALA != "1" {
        push(&mut env, "NXRT_GFX_RENDER_SCALE", o.ESCALA.clone());
    }
    let mut use_proton = false;
    if o.REESCALADO == "dlss" && instance::has_dlss(inst) {
        push(&mut env, "NXRT_DLSS_UPSCALER", "dlss".into());
        push(&mut env, "NXRT_UPSCALE_DLSS_DLL", format!("{w}\\upscalers\\streamline"));
        push(&mut env, "NXRT_DIAG_WATCHDOG_SECONDS", "600".into());
        use_proton = true;
    } else if o.REESCALADO == "fsr3" && instance::has_fsr(inst) {
        push(&mut env, "NXRT_DLSS_UPSCALER", "fsr3".into());
        push(&mut env, "NXRT_DLSS_SHARPNESS", o.NITIDEZ.clone());
        push(&mut env, "NXRT_UPSCALE_FSR_DLL", format!("{w}\\upscalers\\fsr\\amd_fidelityfx_loader_vk.dll"));
    }
    if let Some(only) = crate::hw::sdl_only(&o.MANDO) {
        // SDL inside the game (under Wine/Proton) then opens only that controller
        push(&mut env, "SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT", only);
    }
    let log_rel = format!("work\\logs\\play-{stamp}.log");
    let log = pkg.join("work/logs").join(format!("play-{stamp}.log"));
    let game_args = vec!["--exefs".to_string(), format!("{w}\\exefs"), "--link".into(), "host".into(), "--gfx".into(), "window".into(), "--log".into(), format!("{w}\\{log_rel}")];

    let gamemode = util::which("gamemoderun");
    let (runner, mut args): (PathBuf, Vec<String>) = if use_proton {
        let proton = proton(proton_setting).ok_or("no-proton")?;
        // Proton runs without Steam too: then the launcher's own folder stands in for Steam's
        let steam = steam_roots().into_iter().next().unwrap_or_else(|| {
            let d = util::data_dir().join("steam");
            let _ = fs::create_dir_all(&d);
            d
        });
        push(&mut env, "STEAM_COMPAT_CLIENT_INSTALL_PATH", steam.display().to_string());
        push(&mut env, "STEAM_COMPAT_DATA_PATH", inst.join("proton-pfx").display().to_string());
        push(&mut env, "SteamAppId", "0".into());
        push(&mut env, "SteamGameId", "0".into());
        push(&mut env, "UMU_ID", "0".into());
        // Proton wants the exe's absolute path
        (proton, vec!["run".into(), pkg.join(EXE).display().to_string()])
    } else {
        (wine(wine_setting).ok_or("no-wine")?, vec![EXE.to_string()])
    };
    args.extend(game_args);
    let (program, args) = match gamemode {
        Some(g) => {
            let mut a = vec![runner.display().to_string()];
            a.extend(args);
            (g, a)
        }
        None => (runner, args),
    };
    Ok(Plan { program, args, env, proton: use_proton, log })
}

/// Starts the game, detached from the launcher (it keeps running when the launcher closes).
/// Starts the game. `on_exit(code, last lines of the output)` is called when it ends with an error within its first minute
/// (it did not really start): the launcher shows that instead of waiting forever.
pub fn start(inst: &Path, o: &Options, wine_setting: &str, proton_setting: &str, on_exit: Box<dyn FnOnce(i32, String) + Send>) -> Result<Plan, String> {
    if util::process_running(EXE) {
        return Err("running".into());
    }
    let pkg = instance::pkg(inst);
    let p = plan(inst, o, wine_setting, proton_setting, util::now())?;
    // the game's files as plain Linux links (an instance built before v0.1.6 still has Wine's junctions)
    crate::compile::repair_links(inst);
    if p.proton {
        // Proton creates its prefix inside this folder, but not the folder itself (pfx.lock: FileNotFoundError)
        fs::create_dir_all(inst.join("proton-pfx")).map_err(|e| format!("proton-pfx: {e}"))?;
    } else {
        // no Wine window ever: the prefix is brought up to date headless before the game starts in it
        let w = wine(wine_setting).ok_or("no-wine")?;
        crate::deps::prepare_prefix(&w, &inst.join("prefix"))?;
    }
    write_controls(&pkg, o);
    let _ = fs::create_dir_all(pkg.join("work/logs"));
    let _ = fs::remove_file(pkg.join("shader_cache/spirv.preload.pack"));
    let out = fs::File::create(pkg.join("work/logs/launcher-stdout.log")).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c: Command = host_cmd(&p.program);
    c.args(&p.args).current_dir(&pkg).stdin(Stdio::null()).stdout(out).stderr(err);
    // the counter is the port's own now (controls.ini [fps]); MangoHud stays off unless the player forces it
    if std::env::var_os("MANGOHUD").is_none() {
        c.env("DISABLE_MANGOHUD", "1");
    }
    for (k, v) in &p.env {
        c.env(k, v);
    }
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let mut child = c.spawn().map_err(|e| format!("{}: {e}", p.program.display()))?;
    let started = std::time::Instant::now();
    let out_log = pkg.join("work/logs/launcher-stdout.log");
    // reap it in the background (no zombie while the launcher is open), and report an early failure
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        if code != 0 && started.elapsed().as_secs() < 60 {
            let text = fs::read_to_string(&out_log).unwrap_or_default();
            let lines: Vec<&str> = text.lines().filter(|l| !l.contains("MESA-EGL") && !l.starts_with("pci id for fd")).collect();
            let tail = lines[lines.len().saturating_sub(25)..].join("\n");
            on_exit(code, tail);
        }
    });
    Ok(p)
}

/// Ends the game (the player asked: the options changed and need a restart).
pub fn stop() {
    if let Ok(o) = host_cmd("pgrep").arg("-x").arg(EXE).output() {
        for pid in String::from_utf8_lossy(&o.stdout).split_whitespace() {
            let _ = host_cmd("kill").arg(pid).status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_keys_set_or_added() {
        let t = "[display]\nwindow_mode = windowed\n\n[pad]\ngyro = 1\n";
        let a = set_ini(t, "fps", "show", "1");
        assert!(a.ends_with("[fps]\nshow = 1\n"), "{a}");
        let b = set_ini(&a, "fps", "show", "0");
        assert!(b.contains("show = 0") && !b.contains("show = 1"));
        let c = set_ini(&b, "fps", "corner", "top_right");
        assert!(c.ends_with("show = 0\ncorner = top_right\n"), "{c}");
        let d = set_ini(t, "display", "vsync", "1");
        assert!(d.starts_with("[display]\nwindow_mode = windowed\nvsync = 1\n\n[pad]"), "{d}");
    }

    #[test]
    fn wine_plan_matches_the_script() {
        let d = std::env::temp_dir().join(format!("xc2l-plan-{}", std::process::id()));
        let pkg = d.join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join(EXE), b"").unwrap();
        fs::write(pkg.join("launch.env"), "# c\nNXRT_XC2_FPS=60\n").unwrap();
        let o = Options { REESCALADO: "taau".into(), ESCALA: "1.5".into(), ..Default::default() };
        let p = plan(&d, &o, "/bin/sh", "", 7).unwrap();
        let get = |k: &str| p.env.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        assert_eq!(get("NXRT_XC2_FPS").as_deref(), Some("60"));
        assert_eq!(get("NXRT_GFX_RENDER_SCALE").as_deref(), Some("1.5"));
        assert!(get("WINEDLLOVERRIDES").unwrap().starts_with("xc2_rt=n;"));
        assert!(!get("WINEDLLOVERRIDES").unwrap().contains("mscoree"));
        assert!(get("NXRT_DLSS_UPSCALER").is_none());
        assert!(!p.proton);
        assert!(p.args.iter().any(|a| a.ends_with("\\work\\logs\\play-7.log")));
        let _ = fs::remove_dir_all(&d);
    }
}
