// SPDX-License-Identifier: GPL-3.0-or-later
//! XC2 Launcher for Linux: sets up the Xenoblade Chronicles 2 NS2 static recompilation (xc2-ns2-recomp) from the player's own
//! dump, keeps its instances, and starts the game through Wine or Proton. It ships no game files, no game art and no NVIDIA or
//! AMD files.

mod compile;
mod deps;
mod hw;
mod instance;
mod kwin;
mod launch;
mod net;
mod nup;
mod mods;
mod savedit;
mod util;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct Config {
    /// "es" or "en"; empty: from the system's language
    lang: String,
    /// instance folders, in the order they were added
    instances: Vec<String>,
    /// the instance Play starts
    default: String,
    /// "estable" or "preliminar"
    channel: String,
    /// the last dump the player chose (.nup or folder)
    last_source: String,
    /// custom wine / proton (empty: found)
    wine: String,
    proton: String,
    /// build jobs (0: threads - 1)
    jobs: u32,
    /// minimize the launcher when the game starts
    minimize_on_play: bool,
    /// the release the player said "later" to (banner hidden until a newer one)
    dismissed: String,
    /// WebKit's slower, safer drawing (DMA-BUF off): for drivers where the window stays blank
    safe_render: bool,
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| util::home().join(".config"));
    base.join("aegis-oppai/config.json")
}

fn load() -> Config {
    let mut c: Config = fs::read_to_string(config_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if c.channel.is_empty() {
        c.channel = "estable".into();
    }
    c
}

fn save(c: &Config) -> Result<(), String> {
    util::write_atomic(&config_path(), &serde_json::to_string_pretty(c).map_err(|e| e.to_string())?)
}

fn jobs(c: &Config) -> usize {
    if c.jobs > 0 {
        return c.jobs as usize;
    }
    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(2).max(1)
}

fn known(c: &Config, path: &str) -> Result<PathBuf, String> {
    c.instances.iter().find(|i| i.as_str() == path).map(PathBuf::from).ok_or_else(|| "unknown-instance".to_string())
}

#[derive(Default)]
struct Releases(Mutex<Option<Vec<net::Release>>>);

#[derive(Default)]
struct VendorTask {
    running: AtomicBool,
    cancel: AtomicBool,
}

#[derive(Serialize)]
struct AppState {
    config: Config,
    version: String,
    instances: Vec<instance::Info>,
    running: bool,
    building: compile::Progress,
    runners: launch::Runners,
    system_lang: String,
    lichii: Vec<instance::EmuSave>,
    jobs: usize,
}

#[tauri::command]
async fn get_state(app: AppHandle, build: State<'_, Arc<compile::Shared>>) -> Result<AppState, String> {
    let c = load();
    let mut p = build.progress.lock().unwrap().clone();
    if p.state.is_empty() {
        p.state = "idle".into();
    }
    Ok(AppState {
        instances: c.instances.iter().map(|i| instance::info(Path::new(i))).collect(),
        running: util::process_running(launch::EXE),
        building: p,
        runners: launch::runners(&c.wine, &c.proton),
        system_lang: std::env::var("LANG").unwrap_or_default(),
        lichii: instance::emulator_saves(),
        jobs: jobs(&c),
        version: app.package_info().version.to_string(),
        config: c,
    })
}

#[tauri::command]
async fn set_config(patch: serde_json::Value) -> Result<(), String> {
    let mut v = serde_json::to_value(load()).map_err(|e| e.to_string())?;
    if let (Some(obj), Some(p)) = (v.as_object_mut(), patch.as_object()) {
        for (k, val) in p {
            if k != "instances" {
                obj.insert(k.clone(), val.clone());
            }
        }
    }
    let c: Config = serde_json::from_value(v).map_err(|e| e.to_string())?;
    save(&c)
}

/// Adds an existing install folder (with pkg/). Returns its info.
#[tauri::command]
async fn add_instance(path: String) -> Result<instance::Info, String> {
    let p = PathBuf::from(path.trim());
    // a pkg/ folder chosen directly: its parent is the instance
    let p = if p.file_name().is_some_and(|n| n == "pkg") && p.join("engine").is_dir() { p.parent().unwrap().to_path_buf() } else { p };
    if !instance::looks_like_instance(&p) {
        return Err("not-instance".into());
    }
    let mut c = load();
    let s = p.display().to_string();
    if !c.instances.contains(&s) {
        c.instances.push(s.clone());
    }
    if c.default.is_empty() || !c.instances.contains(&c.default) {
        c.default = s;
    }
    save(&c)?;
    Ok(instance::info(&p))
}

/// Forgets an instance (its folder stays on disk: the launcher deletes nothing).
#[tauri::command]
async fn forget_instance(path: String) -> Result<(), String> {
    let mut c = load();
    c.instances.retain(|i| i != &path);
    if c.default == path {
        c.default = c.instances.first().cloned().unwrap_or_default();
    }
    save(&c)
}

#[tauri::command]
async fn set_default(path: String) -> Result<(), String> {
    let mut c = load();
    known(&c, &path)?;
    c.default = path;
    save(&c)
}

#[tauri::command]
async fn instance_size(path: String) -> u64 {
    tauri::async_runtime::spawn_blocking(move || util::dir_size(Path::new(&path))).await.unwrap_or(0)
}

/// The instance's cover (from the player's own dump) as a data: URL.
#[tauri::command]
async fn cover(path: String) -> Option<String> {
    use base64::Engine;
    let p = instance::cover_path(Path::new(&path))?;
    let bytes = fs::read(&p).ok()?;
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else { "image/jpeg" };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Uses an image the player picks (their own dump's icon) as the instance's cover.
#[tauri::command]
async fn set_cover(path: String, image: String) -> Result<(), String> {
    let c = load();
    let inst = known(&c, &path)?;
    let ext = if image.to_lowercase().ends_with(".png") { "png" } else { "jpg" };
    for n in ["cover.jpg", "cover.png"] {
        let _ = fs::remove_file(inst.join(n));
    }
    fs::copy(&image, inst.join(format!("cover.{ext}"))).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn read_options(path: String) -> instance::Options {
    instance::read_options(Path::new(&path))
}

#[tauri::command]
async fn write_options(path: String, options: instance::Options) -> Result<(), String> {
    let c = load();
    instance::write_options(&known(&c, &path)?, &options)
}

#[derive(Serialize)]
struct Played {
    proton: bool,
}

/// The screen size the game draws at (MangoHud's offsets are in its pixels): the window size option, else the display's.
fn game_screen(o: &instance::Options) -> (u32, u32) {
    let parse = |m: &str| m.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
    if o.VENTANA != "auto" {
        if let Some(s) = parse(&o.VENTANA) {
            return s;
        }
    }
    hw::displays().iter().find_map(|d| parse(&d.current)).unwrap_or((1920, 1080))
}

/// Follows the runtime's log (pkg/nxrt.log, written anew at each start): while the game's window is black MangoHud shows
/// the loading card (rewritten here as it advances); when frames go out, the counter (or nothing), and on KDE the Proton
/// console is minimized and the game focused.
fn watch_start(pkg: PathBuf, since: std::time::SystemTime, o: instance::Options, lang: String, first: bool, failed: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let log = pkg.join("nxrt.log");
        let file = pkg.join("work/aegis-mangohud.conf");
        let screen = game_screen(&o);
        let title = if lang == "es" { "Preparando los gráficos" } else { "Preparing the graphics" };
        let t0 = std::time::Instant::now();
        let mut window_at: Option<std::time::Instant> = None;
        let mut seen_running = false;
        let mut last = String::new();
        // seconds the black window usually lasts: longer the first time (the driver has no pipeline cache yet)
        let expect = if first { 60.0 } else { 20.0 };
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if failed.load(Ordering::SeqCst) || t0.elapsed().as_secs() > 600 {
                break;
            }
            let running = util::process_running(launch::EXE);
            seen_running |= running;
            if seen_running && !running && t0.elapsed().as_secs() > 5 {
                break;
            }
            let fresh = log.metadata().and_then(|m| m.modified()).map(|m| m >= since).unwrap_or(false);
            let text = if fresh { fs::read_to_string(&log).unwrap_or_default() } else { String::new() };
            if text.contains("nvn: present interval") || text.contains("hitch: ") {
                let _ = fs::write(&file, deps::mango_loading(title, 100, screen));
                std::thread::sleep(std::time::Duration::from_millis(900));
                break;
            }
            if window_at.is_none() && text.contains("gfx: swapchain") {
                window_at = Some(std::time::Instant::now());
            }
            if let Some(at) = window_at {
                let p = at.elapsed().as_secs_f64() / expect;
                // eases out and never reaches the end before the game draws
                let pct = (3.0 + 93.0 * (1.0 - (1.0 - p.min(1.0)).powi(2))) as u32;
                let conf = deps::mango_loading(title, pct, screen);
                if conf != last {
                    let _ = fs::write(&file, &conf);
                    last = conf;
                }
            }
        }
        let _ = fs::write(&file, if o.FPS == "1" { deps::mango_counter(&o.ESQUINA) } else { deps::MANGO_HIDDEN.to_string() });
        kwin::release_game();
    });
}

#[tauri::command]
async fn play(app: AppHandle, path: String, build: State<'_, Arc<compile::Shared>>) -> Result<Played, String> {
    if build.progress.lock().unwrap().state == "running" {
        return Err("building".into());
    }
    let c = load();
    let inst = known(&c, &path)?;
    if !instance::ready(&inst) {
        return Err("not-ready".into());
    }
    let o = instance::read_options(&inst);
    if o.REESCALADO == "dlss" && !instance::has_dlss(&inst) {
        return Err("no-dlss".into());
    }
    if util::process_running(launch::EXE) {
        return Err("running".into());
    }
    // mods added or changed by hand, or a game update that moved the archive index: their records are redone first
    mods::prepare_stale(&instance::pkg(&inst));
    let proton = o.REESCALADO == "dlss";
    // the first start of a prefix is the long one
    let first = if proton { !inst.join("proton-pfx/pfx").is_dir() } else { !inst.join("prefix/.aegis-wine").is_file() };
    let lang = if c.lang.is_empty() { "en".to_string() } else { c.lang.clone() };
    let since = std::time::SystemTime::now();
    let failed = Arc::new(AtomicBool::new(false));
    let (a, f) = (app.clone(), failed.clone());
    let (inst2, o2, wine, proton_s) = (inst.clone(), o.clone(), c.wine.clone(), c.proton.clone());
    tauri::async_runtime::spawn_blocking(move || {
        launch::start(&inst2, &o2, &wine, &proton_s, Box::new(move |code, tail| {
            f.store(true, Ordering::SeqCst);
            let _ = a.emit("game-exit", serde_json::json!({ "code": code, "log": tail }));
        }))
    })
    .await
    .map_err(|e| e.to_string())??;
    watch_start(instance::pkg(&inst), since, o, lang, first, failed);
    if c.minimize_on_play {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.minimize();
        }
    }
    Ok(Played { proton })
}

#[tauri::command]
async fn game_running() -> bool {
    util::process_running(launch::EXE)
}

#[tauri::command]
async fn stop_game() {
    launch::stop();
}

#[derive(Serialize)]
struct Hardware {
    displays: Vec<hw::Display>,
    pads: Vec<hw::Pad>,
}

/// The monitors' resolutions and the controllers, for Settings.
#[tauri::command]
async fn hardware() -> Hardware {
    tauri::async_runtime::spawn_blocking(|| Hardware { displays: hw::displays(), pads: hw::pads() })
        .await
        .unwrap_or(Hardware { displays: Vec::new(), pads: Vec::new() })
}

// ---- what the game needs from the system ----

/// Downloads the launcher's portable Wine 11.22 (no password, nothing installed in the system).
#[tauri::command]
async fn install_wine(app: AppHandle, task: State<'_, Arc<VendorTask>>) -> Result<String, String> {
    if task.running.swap(true, Ordering::SeqCst) {
        return Err("busy".into());
    }
    task.cancel.store(false, Ordering::SeqCst);
    let t = task.inner().clone();
    let a = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        deps::install_wine(&t.cancel, &|phase, d, total| {
            let _ = a.emit("vendor-progress", serde_json::json!({"phase": phase, "done": d, "total": total, "id": "wine"}));
        })
    })
    .await
    .map_err(|e| e.to_string());
    task.running.store(false, Ordering::SeqCst);
    Ok(res??.display().to_string())
}

/// Downloads GE-Proton (for DLSS) into Steam's compatibility tools folder, or the launcher's when there is no Steam.
#[tauri::command]
async fn install_proton(app: AppHandle, task: State<'_, Arc<VendorTask>>) -> Result<String, String> {
    if task.running.swap(true, Ordering::SeqCst) {
        return Err("busy".into());
    }
    task.cancel.store(false, Ordering::SeqCst);
    let t = task.inner().clone();
    let a = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        deps::install_proton(&t.cancel, &|phase, d, total| {
            let _ = a.emit("vendor-progress", serde_json::json!({"phase": phase, "done": d, "total": total, "id": "proton"}));
        })
    })
    .await
    .map_err(|e| e.to_string());
    task.running.store(false, Ordering::SeqCst);
    Ok(res??.display().to_string())
}

// ---- releases and updates ----

#[derive(Serialize)]
struct ReleaseInfo {
    game: Vec<net::Release>,
    launcher: Option<net::Release>,
    error: Option<String>,
}

#[tauri::command]
async fn releases(app: AppHandle, cache: State<'_, Releases>, refresh: bool) -> Result<ReleaseInfo, String> {
    if !refresh {
        if let Some(r) = cache.0.lock().unwrap().clone() {
            return Ok(ReleaseInfo { game: r, launcher: None, error: None });
        }
    }
    let current = app.package_info().version.to_string();
    let (game, launcher) = tauri::async_runtime::spawn_blocking(move || {
        let g = net::game_releases();
        let l = net::launcher_releases().ok().and_then(|v| v.into_iter().find(|r| !r.prerelease && net::newer(&r.version, &current)));
        (g, l)
    })
    .await
    .map_err(|e| e.to_string())?;
    // XC2L_DEMO_UPDATE: shows the launcher's update banner without a published release (screenshots, testing)
    let launcher = launcher.or_else(|| {
        std::env::var("XC2L_DEMO_UPDATE").ok().map(|v| net::Release {
            tag: format!("v{v}"),
            version: v.clone(),
            name: format!("XC2 Launcher v{v}"),
            prerelease: false,
            date: String::new(),
            notes: "- Demo".into(),
            url: String::new(),
            asset_name: String::new(),
            asset_url: String::new(),
            asset_size: 0,
            sha256: String::new(),
        })
    });
    match game {
        Ok(g) => {
            *cache.0.lock().unwrap() = Some(g.clone());
            Ok(ReleaseInfo { game: g, launcher, error: None })
        }
        Err(e) => Ok(ReleaseInfo { game: cache.0.lock().unwrap().clone().unwrap_or_default(), launcher, error: Some(e) }),
    }
}

/// Replaces this AppImage with the newer one and starts it.
#[tauri::command]
async fn update_launcher(app: AppHandle, release: net::Release) -> Result<(), String> {
    let me = std::env::var_os("APPIMAGE").map(PathBuf::from).ok_or("not-appimage")?;
    if release.asset_url.is_empty() {
        return Err("no-asset".into());
    }
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let tmp = me.with_extension("new");
        let cancel = AtomicBool::new(false);
        net::download(&release.asset_url, &tmp, release.asset_size, &cancel, &|d, t| {
            let _ = a.emit("vendor-progress", serde_json::json!({"phase": "launcher", "done": d, "total": t}));
        })?;
        if !release.sha256.is_empty() && net::sha256(&tmp)? != release.sha256 {
            let _ = fs::remove_file(&tmp);
            return Err("sha256".into());
        }
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &me).map_err(|e| e.to_string())?;
        util::host_cmd(&me).spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    app.exit(0);
    Ok(())
}

// ---- set up ----

#[tauri::command]
fn start_build(app: AppHandle, build: State<Arc<compile::Shared>>, mut request: compile::Request) -> Result<(), String> {
    if util::process_running(launch::EXE) {
        return Err("running".into());
    }
    {
        let mut p = build.progress.lock().unwrap();
        if p.state == "running" {
            return Err("building".into());
        }
        *p = compile::Progress { state: "running".into(), instance: request.instance.clone(), step: "package".into(), ..Default::default() };
    }
    let mut c = load();
    let inst = PathBuf::from(request.instance.trim());
    if inst.as_os_str().is_empty() || !inst.is_absolute() {
        build.progress.lock().unwrap().state = "error".into();
        return Err("folder".into());
    }
    request.wine = c.wine.clone();
    request.jobs = jobs(&c);
    let s = inst.display().to_string();
    if !c.instances.contains(&s) {
        c.instances.push(s.clone());
    }
    if c.default.is_empty() {
        c.default = s;
    }
    if !request.source.trim().is_empty() {
        c.last_source = request.source.trim().to_string();
    }
    save(&c)?;
    let shared = build.inner().clone();
    shared.cancel.store(false, Ordering::SeqCst);
    shared.log.lock().unwrap().clear();
    let instances: Vec<PathBuf> = c.instances.iter().map(PathBuf::from).collect();
    std::thread::spawn(move || {
        let a1 = app.clone();
        let a2 = app.clone();
        let emit = move |p: compile::Progress| {
            let _ = a1.emit("build-progress", p);
        };
        let emit_log = move |l: Vec<String>| {
            let _ = a2.emit("build-log", l);
        };
        let ctx = compile::Ctx::new(&shared, &emit, &emit_log, instances);
        let end = compile::run(&ctx, &request);
        let _ = app.emit("build-progress", end);
    });
    Ok(())
}

#[tauri::command]
fn cancel_build(build: State<Arc<compile::Shared>>) {
    build.cancel.store(true, Ordering::SeqCst);
    let inst = build.progress.lock().unwrap().instance.clone();
    let prefix = PathBuf::from(inst).join("prefix");
    compile::kill(&build, Some(&prefix));
}

#[tauri::command]
fn build_log(build: State<Arc<compile::Shared>>) -> Vec<String> {
    build.log.lock().unwrap().clone()
}

/// Where a new instance goes by default: ~/Games/xc2-recomp-<version> (a free name).
#[tauri::command]
async fn suggest_folder(version: String) -> String {
    let base = util::home().join("Games");
    let v = if version.is_empty() { "nueva".to_string() } else { version };
    let mut p = base.join(format!("xc2-recomp-{v}"));
    let mut n = 2;
    while p.exists() {
        p = base.join(format!("xc2-recomp-{v}-{n}"));
        n += 1;
    }
    p.display().to_string()
}

/// The dump inside the folder the player chose, at any depth: a .nup, a folder with exefs/main and romfs/, or a dump root
/// with base/ and updates/. The shallowest match wins; a .nup counts only when no folder layout is found.
fn find_source(root: &Path) -> Option<(PathBuf, &'static str)> {
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), 0usize)]);
    let mut nup: Option<PathBuf> = None;
    while let Some((d, depth)) = queue.pop_front() {
        if d.join("exefs/main").is_file() && d.join("romfs").is_dir() {
            return Some((d, "folder"));
        }
        if d.join("base").is_dir() && d.join("updates").is_dir() {
            return Some((d, "dump"));
        }
        let Ok(rd) = fs::read_dir(&d) else { continue };
        let mut subs: Vec<PathBuf> = Vec::new();
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                // inside a game tree there is nothing more to find
                if depth < 12 && n != "romfs" && n != "exefs" && !n.starts_with('.') {
                    subs.push(p);
                }
            } else if ft.is_file() && nup.is_none() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("nup")) && is_nup(&p) {
                nup = Some(p);
            }
        }
        subs.sort();
        queue.extend(subs.into_iter().map(|p| (p, depth + 1)));
    }
    nup.map(|p| (p, "nup"))
}

fn is_nup(p: &Path) -> bool {
    use std::io::Read;
    let mut m = [0u8; 4];
    fs::File::open(p).and_then(|mut f| f.read_exact(&mut m)).is_ok() && &m == b"NUP\x01"
}

#[derive(Serialize)]
struct SourceCheck {
    ok: bool,
    kind: String,
    detail: String,
    /// what was found (the folder itself, or something inside it): what Set up uses
    path: String,
}

/// Looks for the dump in what the player chose (a .nup file, or any folder that holds one somewhere inside).
#[tauri::command]
async fn check_source(path: String) -> SourceCheck {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(path.trim());
        let found = if p.is_file() { is_nup(&p).then(|| (p.clone(), "nup")) } else { find_source(&p) };
        match found {
            Some((f, kind)) => {
                let detail = match kind {
                    "nup" => format!("{:.1} GB · {}", f.metadata().map(|m| m.len()).unwrap_or(0) as f64 / 1e9, util::tilde(&f)),
                    "folder" => format!("exefs + romfs · {}", util::tilde(&f)),
                    _ => format!("base + updates · {}", util::tilde(&f)),
                };
                SourceCheck { ok: true, kind: kind.into(), detail, path: f.display().to_string() }
            }
            None => SourceCheck { ok: false, kind: "none".into(), detail: String::new(), path: String::new() },
        }
    })
    .await
    .unwrap_or(SourceCheck { ok: false, kind: "none".into(), detail: String::new(), path: String::new() })
}

// ---- vendor upscalers ----

#[tauri::command]
async fn vendor_license(id: String) -> Result<Vec<net::LicenseText>, String> {
    let v = net::vendor(&id).ok_or("vendor")?;
    tauri::async_runtime::spawn_blocking(move || net::licenses(v)).await.map_err(|e| e.to_string())?
}

/// Downloads NVIDIA's or AMD's files into the instance. `accepted` is the player's tick under the license text.
#[tauri::command]
async fn install_vendor(app: AppHandle, task: State<'_, Arc<VendorTask>>, path: String, id: String, accepted: bool) -> Result<String, String> {
    if !accepted {
        return Err("license".into());
    }
    let v = net::vendor(&id).ok_or("vendor")?;
    let c = load();
    let inst = known(&c, &path)?;
    if task.running.swap(true, Ordering::SeqCst) {
        return Err("busy".into());
    }
    task.cancel.store(false, Ordering::SeqCst);
    let t = task.inner().clone();
    let a = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        net::install_vendor(v, &instance::pkg(&inst), &t.cancel, &|phase, d, total| {
            let _ = a.emit("vendor-progress", serde_json::json!({"phase": phase, "done": d, "total": total, "id": v.id}));
        })
    })
    .await
    .map_err(|e| e.to_string());
    task.running.store(false, Ordering::SeqCst);
    Ok(res??.display().to_string())
}

#[tauri::command]
fn cancel_vendor(task: State<Arc<VendorTask>>) {
    task.cancel.store(true, Ordering::SeqCst);
}

// ---- mods ----

#[tauri::command]
async fn mods_list(path: String) -> Result<Vec<mods::Mod>, String> {
    let c = load();
    Ok(mods::list(&instance::pkg(&known(&c, &path)?)))
}

#[tauri::command]
async fn mod_enable(path: String, name: String, on: bool) -> Result<(), String> {
    let c = load();
    mods::set_enabled(&instance::pkg(&known(&c, &path)?), &name, on)
}

#[derive(Serialize)]
struct ModAdded {
    name: String,
    prepared: mods::Prepared,
}

#[tauri::command]
async fn mod_add(path: String, from: String) -> Result<ModAdded, String> {
    let c = load();
    let pkg = instance::pkg(&known(&c, &path)?);
    let (name, prepared) = mods::add(&pkg, Path::new(&from))?;
    Ok(ModAdded { name, prepared })
}

#[tauri::command]
async fn mod_remove(path: String, name: String) -> Result<(), String> {
    let c = load();
    mods::remove(&instance::pkg(&known(&c, &path)?), &name)
}

#[tauri::command]
async fn mod_prepare(path: String, name: String) -> Result<mods::Prepared, String> {
    let c = load();
    mods::prepare(&instance::pkg(&known(&c, &path)?), &name)
}

// ---- saves ----

#[tauri::command]
async fn saves(path: String) -> instance::Saves {
    instance::list_saves(Path::new(&path))
}

#[derive(Serialize)]
struct Imported {
    copied: usize,
    backup: Option<String>,
}

#[tauri::command]
async fn import_saves(path: String, from: String) -> Result<Imported, String> {
    let c = load();
    let inst = known(&c, &path)?;
    if util::process_running(launch::EXE) {
        return Err("running".into());
    }
    let (copied, backup) = instance::import_saves(&inst, Path::new(&from))?;
    Ok(Imported { copied, backup: backup.map(|b| b.display().to_string()) })
}

#[tauri::command]
async fn backup_saves(path: String) -> Result<Option<String>, String> {
    let c = load();
    Ok(instance::backup_saves(&known(&c, &path)?)?.map(|b| b.display().to_string()))
}

#[tauri::command]
async fn save_values(path: String) -> Result<savedit::Values, String> {
    let c = load();
    savedit::read(&instance::account_dir(&known(&c, &path)?))
}

#[derive(Serialize)]
struct Edited {
    values: savedit::Values,
    backup: Option<String>,
}

/// Money and core crystals into the save, after a backup of the whole account folder; never while the game runs (it keeps
/// the save in memory and would write its own over it).
#[tauri::command]
async fn set_save_values(path: String, values: savedit::Values) -> Result<Edited, String> {
    let c = load();
    let inst = known(&c, &path)?;
    if util::process_running(launch::EXE) {
        return Err("running".into());
    }
    let dir = instance::account_dir(&inst);
    savedit::read(&dir)?;
    let backup = instance::backup_saves(&inst)?.map(|b| b.display().to_string());
    Ok(Edited { values: savedit::write(&dir, &values)?, backup })
}

/// Puts the newest backup back in place of the saves.
#[tauri::command]
async fn restore_backup(path: String) -> Result<String, String> {
    let c = load();
    let inst = known(&c, &path)?;
    if util::process_running(launch::EXE) {
        return Err("running".into());
    }
    instance::restore_latest_backup(&inst).map(|b| b.display().to_string())
}

// ---- files and dialogs ----

#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("missing".into());
    }
    util::open_path(&p)
}

/// Opens a release page in the browser (GitHub links only).
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/") || url.contains(char::is_whitespace) {
        return Err("url".into());
    }
    util::host_cmd("xdg-open").arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick(app: AppHandle, kind: String, title: String, start: Option<String>) -> Option<String> {
    let mut d = app.dialog().file().set_title(title);
    if let Some(s) = start.filter(|s| Path::new(s).is_dir()) {
        d = d.set_directory(s);
    } else {
        d = d.set_directory(util::home());
    }
    let r = match kind.as_str() {
        "nup" => d.add_filter("NUP", &["nup"]).blocking_pick_file(),
        "zip" => d.add_filter("ZIP", &["zip"]).blocking_pick_file(),
        "image" => d.add_filter("Image", &["jpg", "jpeg", "png"]).blocking_pick_file(),
        _ => d.blocking_pick_folder(),
    };
    r.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

/// Running from the AppImage: a .desktop entry (and its icon) in the user's applications, so the desktop's task bar and
/// Alt+Tab know the window (its class is "aegis-oppai") and the launcher is in the applications menu. Kept pointing at the
/// AppImage it was last started from.
fn register_desktop_entry() {
    let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) else { return };
    let share = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| util::home().join(".local/share"));
    let icon_dir = share.join("icons/hicolor/256x256/apps");
    let icon = icon_dir.join("aegis-oppai.png");
    if let Some(appdir) = std::env::var_os("APPDIR").map(PathBuf::from) {
        let src = ["usr/share/icons/hicolor/512x512/apps", "usr/share/icons/hicolor/256x256@2/apps", "usr/share/icons/hicolor/128x128/apps"]
            .iter()
            .flat_map(|d| fs::read_dir(appdir.join(d)).into_iter().flatten().flatten().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|x| x == "png"))
            .or_else(|| fs::read_dir(&appdir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "png")));
        if let Some(src) = src {
            let _ = fs::create_dir_all(&icon_dir);
            let _ = fs::copy(src, &icon);
        }
    }
    let exec = appimage.display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=The Aegis’s Oppai\nGenericName=XC2S2 Edition Launcher\nComment=Xenoblade Chronicles 2 NS2 recompilation launcher\nExec=\"{exec}\"\nIcon=aegis-oppai\nTerminal=false\nCategories=Game;\nStartupWMClass=aegis-oppai\nStartupNotify=true\n"
    );
    let path = share.join("applications/aegis-oppai.desktop");
    if fs::read_to_string(&path).ok().as_deref() != Some(entry.as_str()) {
        let _ = util::write_atomic(&path, &entry);
        let _ = util::host_cmd("update-desktop-database").arg(share.join("applications")).status();
    }
}

fn main() {
    register_desktop_entry();
    // WebKit's fast drawing (DMA-BUF) by default; Settings > Advanced > "Safe drawing" turns it off for the drivers where the
    // window stays blank with it. A variable the player set wins.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() && load().safe_render {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // Ubuntu 24.04 and newer (AppArmor) forbid unprivileged user namespaces to programs without a profile, an AppImage
    // among them: WebKit's bubblewrap sandbox then fails and the window stays blank. The launcher shows only its own
    // local page, never a web page, so it runs without that sandbox there.
    let userns_restricted = fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns").map(|v| v.trim() == "1").unwrap_or(false);
    if userns_restricted && std::env::var_os("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS").is_none() {
        std::env::set_var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(compile::shared())
        .manage(Releases::default())
        .manage(Arc::new(VendorTask::default()))
        .on_window_event(|window, event| {
            // closing the window stops a running Set up rather than leaving Wine and the compilers behind
            if let tauri::WindowEvent::Destroyed = event {
                let st = window.state::<Arc<compile::Shared>>();
                if st.progress.lock().unwrap().state == "running" {
                    st.cancel.store(true, Ordering::SeqCst);
                    let inst = st.progress.lock().unwrap().instance.clone();
                    compile::kill(&st, Some(&PathBuf::from(inst).join("prefix")));
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state, set_config, add_instance, forget_instance, set_default, instance_size, cover, set_cover, read_options,
            write_options, play, game_running, stop_game, releases, update_launcher, start_build, cancel_build, build_log,
            suggest_folder, check_source, vendor_license, install_vendor, cancel_vendor, saves, import_saves, backup_saves,
            save_values, set_save_values, restore_backup, mods_list, mod_enable, mod_add, mod_remove, mod_prepare,
            open_path, open_url, pick, hardware, install_wine, install_proton,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
