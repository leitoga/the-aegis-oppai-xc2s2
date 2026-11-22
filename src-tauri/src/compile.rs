// SPDX-License-Identifier: GPL-3.0-or-later
//! Set up ("Compilar"): builds an instance from the player's own dump.
//!
//! 1. package  the release zip (downloaded from the port's GitHub release, or one the player has) unzipped into <instance>/pkg
//! 2. prefix   the Wine prefix
//! 3. lift     the package's own setup engine (engine/xc2_setup.py, run by the package's Python under Wine): game files in,
//!             ARM code lifted to C++ (--skip link,shaders,done)
//! 4. zig      zig for Linux (the package's Windows zig fails under Wine)
//! 5. link     the lifted C++ compiled and linked into xc2_boot.exe with native zig, the same flags as the engine
//! 6. shaders  the engine again (--skip lift,link): the shader cache, nxrt.ini and launch.env
//! 7. done     the launcher's marker, default options and the cover from the dump
//!
//! The engine keeps its own state on disk, so a stopped Set up continues where it stopped.

use crate::instance;
use crate::net::{self, Release};
use crate::util::{self, host_cmd, win};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const STEPS: [&str; 8] = ["package", "prefix", "extract", "lift", "zig", "link", "shaders", "done"];

#[derive(Serialize, Clone, Default)]
pub struct Progress {
    /// idle, running, done, error, cancelled
    pub state: String,
    pub instance: String,
    pub step: String,
    pub step_index: usize,
    /// 0..1 within the step, negative when unknown
    pub fraction: f64,
    pub message: String,
}

#[derive(Default)]
pub struct Shared {
    pub progress: Mutex<Progress>,
    pub log: Mutex<Vec<String>>,
    pub cancel: AtomicBool,
    pub pid: AtomicU32,
}

#[derive(Deserialize, Clone)]
pub struct Request {
    pub instance: String,
    /// the dump: a .nup, or a folder (exefs/ + romfs/, or base/ + updates/)
    pub source: String,
    /// a release from the list, or None with `local_zip`
    pub release: Option<Release>,
    pub local_zip: Option<String>,
    pub channel: String,
    /// "new" or "update" (unzip over an instance that is already there)
    pub mode: String,
    pub wine: String,
    pub jobs: usize,
}

pub struct Ctx<'a> {
    pub shared: &'a Shared,
    pub emit: &'a (dyn Fn(Progress) + Sync),
    pub emit_log: &'a (dyn Fn(Vec<String>) + Sync),
    pub instances: Vec<PathBuf>,
    log_file: Mutex<Option<fs::File>>,
}

impl<'a> Ctx<'a> {
    pub fn new(shared: &'a Shared, emit: &'a (dyn Fn(Progress) + Sync), emit_log: &'a (dyn Fn(Vec<String>) + Sync), instances: Vec<PathBuf>) -> Self {
        Ctx { shared, emit, emit_log, instances, log_file: Mutex::new(None) }
    }

    fn log(&self, line: impl Into<String>) {
        let line = line.into();
        if let Some(f) = self.log_file.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{line}");
        }
        {
            let mut l = self.shared.log.lock().unwrap();
            l.push(line.clone());
            let n = l.len();
            if n > 4000 {
                l.drain(..n - 3000);
            }
        }
        (self.emit_log)(vec![line]);
    }

    fn set(&self, step: &str, fraction: f64, message: impl Into<String>) {
        let mut p = self.shared.progress.lock().unwrap();
        p.state = "running".into();
        p.step = step.into();
        p.step_index = STEPS.iter().position(|s| *s == step).unwrap_or(0);
        p.fraction = fraction;
        p.message = message.into();
        (self.emit)(p.clone());
    }

    fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::SeqCst)
    }

    /// Runs a command, its output into the log line by line. `on_line` sees each line (progress parsing).
    fn run(&self, mut c: Command, on_line: &(dyn Fn(&str) + Sync)) -> Result<(), String> {
        let shown = format!("$ {:?}", c.get_program());
        self.log(shown);
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        let mut child = c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("{:?}: {e}", c.get_program()))?;
        self.shared.pid.store(child.id(), Ordering::SeqCst);
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| {
                for l in BufReader::new(err).lines().map_while(Result::ok) {
                    if !l.trim().is_empty() && !l.contains("MESA-EGL") && !l.starts_with("pci id for fd") {
                        self.log(format!("  {l}"));
                    }
                }
            });
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                on_line(&l);
                if !l.trim().is_empty() {
                    self.log(format!("  {l}"));
                }
            }
        });
        let st = child.wait().map_err(|e| e.to_string())?;
        self.shared.pid.store(0, Ordering::SeqCst);
        if self.cancelled() {
            return Err("cancelled".into());
        }
        if !st.success() {
            return Err(format!("exit {}", st.code().unwrap_or(-1)));
        }
        Ok(())
    }
}

/// Stops the running step: its process group, and the Wine server of the prefix (python.exe and its children).
pub fn kill(shared: &Shared, prefix: Option<&Path>) {
    let pid = shared.pid.load(Ordering::SeqCst);
    if pid > 0 {
        let _ = host_cmd("kill").arg("-TERM").arg(format!("-{pid}")).status();
    }
    if let Some(p) = prefix.filter(|p| p.is_dir()) {
        let _ = host_cmd("wineserver").arg("-k").env("WINEPREFIX", p).status();
    }
}

fn wine_cmd(req: &Request, inst: &Path) -> Result<Command, String> {
    let wine = crate::launch::wine(&req.wine).ok_or("no-wine")?;
    let mut c = host_cmd(wine);
    c.env("WINEPREFIX", inst.join("prefix")).env("WINEDEBUG", "-all").env("WINEDLLOVERRIDES", crate::deps::WINE_OVERRIDES);
    Ok(c)
}

/// The engine's arguments for the dump the player chose.
fn input_args(source: &Path) -> Vec<String> {
    if source.is_file() {
        return vec!["--nup".into(), win(source)];
    }
    if source.join("exefs").is_dir() && source.join("romfs").is_dir() {
        return vec!["--exefs".into(), win(&source.join("exefs")), "--romfs".into(), win(&source.join("romfs"))];
    }
    if source.join("main").is_file() {
        return vec!["--exefs".into(), win(source)];
    }
    vec!["--dump".into(), win(source)]
}

fn engine(ctx: &Ctx, req: &Request, inst: &Path, source: &Path, step: &str, skip: &str) -> Result<(), String> {
    let pkg = instance::pkg(inst);
    let py = pkg.join("tools/python/python.exe");
    if !py.is_file() {
        return Err(format!("{}: not found", py.display()));
    }
    let mut c = wine_cmd(req, inst)?;
    // UTF-8 everywhere: some Wine versions fail the console write of a character outside the code page (OSError 22)
    c.env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8").env("PYTHONUNBUFFERED", "1");
    c.arg(&py)
        .arg(win(&pkg.join("engine/xc2_setup.py")))
        .args(["--repo", &win(&pkg), "--install", &win(&pkg)])
        .args(input_args(source))
        .args(["--skip", skip, "--jobs", &req.jobs.max(1).to_string()])
        .current_dir(&pkg);
    let step_s = step.to_string();
    ctx.run(c, &|l: &str| {
        // "@progress <phase> <done> <total>" lines from the engine
        if let Some(rest) = l.strip_prefix("@progress ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 3 && parts[0] != "setup" {
                if let (Ok(d), Ok(t)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                    if t > 0.0 {
                        ctx.set(&step_s, d / t, format!("{} {}/{}", parts[0], parts[1], parts[2]));
                    }
                }
            }
        } else if let Some(s) = l.split("STEP: ").nth(1) {
            ctx.set(&step_s, -1.0, s.trim().to_string());
        }
    })
}

/// The dump as the engine sees it: when its path has characters outside ASCII (the .nup names carry an en dash, "–"), a link
/// with a plain name inside the instance (`source.nup`, or `source-dump/` for a folder) stands in for it. Nothing is copied.
fn ascii_source(inst: &Path, source: &Path) -> Result<PathBuf, String> {
    if source.to_string_lossy().is_ascii() {
        return Ok(source.to_path_buf());
    }
    let link = inst.join(if source.is_file() { "source.nup" } else { "source-dump" });
    if let Ok(m) = fs::symlink_metadata(&link) {
        if !m.file_type().is_symlink() {
            return Err(format!("{}: exists and is not a link", link.display()));
        }
        fs::remove_file(&link).map_err(|e| format!("{}: {e}", link.display()))?;
    }
    std::os::unix::fs::symlink(source, &link).map_err(|e| format!("{}: {e}", link.display()))?;
    Ok(link)
}

/// The engine links the game files in with `mklink /J`: under Wine those are Windows junctions (reparse points) that only
/// Wine reads, and that show up on Linux as entries ending in "?". They become plain Linux links here, the same as a
/// hand-made install: pkg/exefs and pkg/romfs to the dump, game/romfs-301 to ../romfs, and the add-on content (DLC, unpacked
/// from the .nup into dump/dlc/<title id>/romfs) as pkg/dlc and game/dlc-301. Wine, Proton and the launcher then all see the
/// same files. Only links and empty leftovers are replaced; a real folder with files in it is left alone.
pub fn native_links(log: &dyn Fn(String), pkg: &Path, input: &Path, dlc: &Path) {
    use std::os::unix::fs::symlink;
    let replace = |link: &Path, target: &Path, ok: &dyn Fn(&Path) -> bool| {
        // a relative target counts from the link's own folder (which has to exist for the path to resolve)
        if let Some(d) = link.parent() {
            let _ = fs::create_dir_all(d);
        }
        let resolved = if target.is_relative() { link.parent().map(|d| d.join(target)).unwrap_or_else(|| target.to_path_buf()) } else { target.to_path_buf() };
        if !resolved.exists() {
            return;
        }
        // the Wine leftover beside it ("exefs?")
        let mut odd = link.as_os_str().to_owned();
        odd.push("?");
        let odd = PathBuf::from(odd);
        if let Ok(m) = fs::symlink_metadata(&odd) {
            if m.is_dir() { let _ = fs::remove_dir(&odd); } else { let _ = fs::remove_file(&odd); }
        }
        if let Ok(m) = fs::symlink_metadata(link) {
            if m.file_type().is_symlink() {
                if fs::read_link(link).map(|t| t == target).unwrap_or(false) {
                    return;
                }
                let _ = fs::remove_file(link);
            } else if m.is_dir() {
                if ok(link) || fs::read_dir(link).map(|mut r| r.next().is_some()).unwrap_or(true) {
                    return; // a real folder with the files: keep it
                }
                let _ = fs::remove_dir(link);
            } else {
                let _ = fs::remove_file(link);
            }
        }
        if let Some(d) = link.parent() {
            let _ = fs::create_dir_all(d);
        }
        match symlink(target, link) {
            Ok(()) => log(format!("linked {} -> {}", link.display(), target.display())),
            Err(e) => log(format!("cannot link {}: {e}", link.display())),
        }
    };
    replace(&pkg.join("exefs"), &input.join("exefs"), &|p| p.join("main").is_file());
    replace(&pkg.join("romfs"), &input.join("romfs"), &|p| p.join("bf2.ard").is_file());
    replace(&pkg.join("game/romfs-301"), Path::new("../romfs"), &|p| p.join("bf2.ard").is_file());
    let has_dlc = fs::read_dir(dlc).map(|mut r| r.next().is_some()).unwrap_or(false);
    if has_dlc {
        replace(&pkg.join("dlc"), dlc, &|p| fs::read_dir(p).map(|mut r| r.next().is_some()).unwrap_or(false));
        replace(&pkg.join("game/dlc-301"), Path::new("../dlc"), &|p| fs::read_dir(p).map(|mut r| r.next().is_some()).unwrap_or(false));
        log(format!("dlc: {}", fs::read_dir(dlc).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>().join(", ")).unwrap_or_default()));
    }
}

/// The same links for an instance that is already built (before v0.1.6, or by hand): called before the game starts.
pub fn repair_links(inst: &Path) {
    let m = instance::read_marker(inst);
    let src = PathBuf::from(&m.source);
    let input = if src.is_file() || inst.join("dump/.nup_done").is_file() {
        match pick_dump(&inst.join("dump")) {
            Ok(d) => d,
            Err(_) => return,
        }
    } else if src.is_dir() {
        src
    } else {
        return;
    };
    native_links(&|_| {}, &instance::pkg(inst), &input, &inst.join("dump/dlc"));
}

/// The folder of an unpacked .nup the game runs from: the newest update that has both exefs/main and romfs/bf2.ard, else
/// the base game.
fn pick_dump(out: &Path) -> Result<PathBuf, String> {
    let complete = |d: &Path| d.join("exefs/main").is_file() && d.join("romfs/bf2.ard").is_file();
    let mut updates: Vec<(u64, PathBuf)> = fs::read_dir(out.join("updates"))
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_string_lossy().parse::<u64>().ok().map(|v| (v, e.path()))).collect())
        .unwrap_or_default();
    updates.sort();
    if let Some((_, d)) = updates.into_iter().rev().find(|(_, d)| complete(d)) {
        return Ok(d);
    }
    let base = out.join("base");
    if complete(&base) {
        return Ok(base);
    }
    Err(format!("{}: no exefs/main with romfs/bf2.ard in the package", out.display()))
}

/// A `.nup` the engine started extracting and did not finish (the launcher was closed, the PC went off) leaves `pkg/exefs`
/// complete and `pkg/romfs` partial, and the engine then takes the game files as installed. When the romfs has no `bf2.ard`
/// the launcher's own extraction is thrown away so the engine extracts again. Only for a `.nup` source: a folder source is
/// linked, never extracted, and nothing of the player's dump is touched.
fn clear_partial_extraction(ctx: &Ctx, pkg: &Path, source: &Path) {
    if !source.is_file() {
        return;
    }
    let romfs = pkg.join("romfs");
    let exefs = pkg.join("exefs");
    let linked = |p: &Path| fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if !exefs.exists() || linked(&romfs) || linked(&exefs) || romfs.join("bf2.ard").is_file() {
        return;
    }
    ctx.log("the game files from the .nup were not completely extracted: extracting them again");
    let _ = fs::remove_dir_all(&romfs);
    let _ = fs::remove_dir_all(&exefs);
}

fn has_fma() -> bool {
    fs::read_to_string("/proc/cpuinfo").map(|t| t.lines().any(|l| l.starts_with("flags") && l.split_whitespace().any(|f| f == "fma"))).unwrap_or(true)
}

/// compilar-linux.sh, in Rust: every lifted .cpp compiled (the ones newer than their object), then linked with the runtime.
fn link(ctx: &Ctx, zig: &Path, inst: &Path, jobs: usize) -> Result<(), String> {
    let pkg = instance::pkg(inst);
    let inc = pkg.join("third_party/nxrecomp/recomp/include");
    let stub = pkg.join("xc2/src/xc2_stub.cpp");
    let cache = inst.join("zig-cache");
    let mcpu = if has_fma() { "-mcpu=x86_64+cx16+fma" } else { "-mcpu=x86_64+cx16" };
    let flags = [
        "-target", "x86_64-windows-gnu", mcpu, "-std=c++20", "-O1", "-w", "-fno-exceptions", "-fno-rtti", "-fno-strict-aliasing",
        "-ffp-contract=off", "-fno-math-errno", "-fno-threadsafe-statics", "-DNX_INLINE_INDIRECT",
    ];
    let obj = pkg.join("build/obj-zig");
    let mut sources: Vec<PathBuf> = Vec::new();
    for m in fs::read_dir(pkg.join("build/lift")).map_err(|e| format!("build/lift: {e}"))?.flatten() {
        if let Ok(rd) = fs::read_dir(m.path()) {
            sources.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "cpp")));
        }
    }
    sources.sort();
    if sources.is_empty() {
        return Err("build/lift: no sources".into());
    }
    let mut rsp = String::new();
    let mut todo: Vec<(PathBuf, PathBuf)> = Vec::new();
    for s in &sources {
        let module = s.parent().and_then(|d| d.file_name()).unwrap().to_owned();
        let o = obj.join(module).join(s.file_stem().unwrap()).with_extension("o");
        let rel = o.strip_prefix(&pkg).unwrap_or(&o).display().to_string();
        rsp.push_str(&format!("\"{rel}\"\n"));
        if !(o.is_file() && util::mtime(&o) >= util::mtime(s)) {
            todo.push((s.clone(), o));
        }
    }
    fs::create_dir_all(&obj).map_err(|e| e.to_string())?;
    fs::write(obj.join("objects.rsp"), &rsp).map_err(|e| e.to_string())?;
    let total = todo.len();
    ctx.log(format!("  {} of {} to compile", total, sources.len()));
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed: Mutex<Option<String>> = Mutex::new(None);
    std::thread::scope(|sc| {
        for _ in 0..jobs.max(1) {
            sc.spawn(|| loop {
                if ctx.cancelled() || failed.lock().unwrap().is_some() {
                    return;
                }
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((s, o)) = todo.get(i) else { return };
                let _ = fs::create_dir_all(o.parent().unwrap());
                let out = host_cmd(zig)
                    .arg("c++")
                    .args(flags)
                    .arg(format!("-I{}", inc.display()))
                    .arg(format!("-I{}", s.parent().unwrap().display()))
                    .arg("-c")
                    .arg(s)
                    .arg("-o")
                    .arg(o)
                    .env("ZIG_GLOBAL_CACHE_DIR", &cache)
                    .env("ZIG_LOCAL_CACHE_DIR", &cache)
                    .current_dir(&pkg)
                    .output();
                match out {
                    Ok(r) if r.status.success() => {
                        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                        if n % 10 == 0 || n == total {
                            ctx.set("link", n as f64 / total.max(1) as f64, format!("{n}/{total}"));
                        }
                    }
                    Ok(r) => {
                        *failed.lock().unwrap() = Some(format!("{}: {}", s.display(), String::from_utf8_lossy(&r.stderr).lines().take(6).collect::<Vec<_>>().join(" | ")));
                    }
                    Err(e) => *failed.lock().unwrap() = Some(format!("zig: {e}")),
                }
            });
        }
    });
    if ctx.cancelled() {
        return Err("cancelled".into());
    }
    if let Some(e) = failed.into_inner().unwrap() {
        return Err(e);
    }
    ctx.set("link", 1.0, "xc2_boot.exe");
    let mut c = host_cmd(zig);
    c.args(["c++", "-target", "x86_64-windows-gnu", "-O1", "-w", "-fno-exceptions", "-fno-rtti", "-DXC2_MODULES_7", "@build/obj-zig/objects.rsp"])
        .arg(&stub)
        .arg(format!("-I{}", inc.display()))
        .arg(format!("-I{}", stub.parent().unwrap().display()))
        .arg("runtime/xc2_rt.lib")
        .args(["-o", "xc2_boot.exe"])
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .current_dir(&pkg);
    ctx.run(c, &|_| {})?;
    for e in fs::read_dir(pkg.join("runtime")).map_err(|e| e.to_string())?.flatten() {
        if e.path().extension().is_some_and(|x| x == "dll") {
            fs::copy(e.path(), pkg.join(e.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The game's icon from the player's dump, when one is next to it (an image the dump tool saved): never shipped.
fn copy_cover(source: &Path, inst: &Path) {
    if instance::cover_path(inst).is_some() {
        return;
    }
    let dir = if source.is_file() { source.parent().map(|d| d.to_path_buf()) } else { Some(source.to_path_buf()) };
    let Some(dir) = dir else { return };
    let mut stack = vec![(dir, 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_lowercase();
            if p.is_dir() {
                if depth < 2 && n != "romfs" && n != "exefs" {
                    stack.push((p, depth + 1));
                }
            } else if (n.ends_with(".jpg") || n.ends_with(".jpeg") || (n.starts_with("icon_") && n.ends_with(".dat")))
                && (n.contains("icon") || n.contains("cover") || n.contains("portada"))
            {
                let _ = fs::copy(&p, inst.join("cover.jpg"));
                return;
            }
        }
    }
}

pub fn run(ctx: &Ctx, req: &Request) -> Progress {
    let inst = PathBuf::from(&req.instance);
    let res = run_inner(ctx, req, &inst);
    let mut p = ctx.shared.progress.lock().unwrap().clone();
    p.instance = req.instance.clone();
    match res {
        Ok(()) => {
            p.state = "done".into();
            p.step = "done".into();
            p.step_index = STEPS.len() - 1;
            p.fraction = 1.0;
            p.message.clear();
            ctx.log("listo / ready");
        }
        Err(e) if e == "cancelled" => {
            p.state = "cancelled".into();
            p.message = e;
        }
        Err(e) => {
            ctx.log(format!("ERROR: {e}"));
            p.state = "error".into();
            p.message = e;
        }
    }
    *ctx.shared.progress.lock().unwrap() = p.clone();
    p
}

fn run_inner(ctx: &Ctx, req: &Request, inst: &Path) -> Result<(), String> {
    let pkg = instance::pkg(inst);
    fs::create_dir_all(pkg.join("setup")).map_err(|e| format!("{}: {e}", pkg.display()))?;
    *ctx.log_file.lock().unwrap() = fs::OpenOptions::new().create(true).append(true).open(inst.join("launcher-setup.log")).ok();
    let mut marker = instance::read_marker(inst);
    let source = if req.source.trim().is_empty() { PathBuf::from(&marker.source) } else { PathBuf::from(req.source.trim()) };
    if !source.exists() {
        return Err(format!("source: {}", source.display()));
    }
    ctx.log(format!("instance {}", inst.display()));
    ctx.log(format!("source   {}", source.display()));

    // 1. the package
    ctx.set("package", -1.0, "");
    let have_pkg = pkg.join("engine/xc2_setup.py").is_file();
    let wanted = req.release.as_ref().map(|r| r.version.clone());
    let need_pkg = !have_pkg || req.mode == "update" || req.local_zip.is_some();
    if need_pkg {
        let zip = if let Some(z) = req.local_zip.as_ref().filter(|z| !z.is_empty()) {
            PathBuf::from(z)
        } else {
            let r = req.release.as_ref().ok_or("release")?;
            if r.asset_url.is_empty() {
                return Err(format!("{}: no package", r.tag));
            }
            let z = util::cache_dir().join(&r.asset_name);
            if !(z.is_file() && z.metadata().map(|m| m.len()).unwrap_or(0) == r.asset_size) {
                net::download(&r.asset_url, &z, r.asset_size, &ctx.shared.cancel, &|d, t| {
                    ctx.set("package", if t > 0 { d as f64 / t as f64 } else { -1.0 }, format!("{:.0} / {:.0} MB", d as f64 / 1e6, t as f64 / 1e6))
                })?;
            }
            if !r.sha256.is_empty() && net::sha256(&z)? != r.sha256 {
                let _ = fs::remove_file(&z);
                return Err(format!("{}: SHA-256", r.asset_name));
            }
            z
        };
        ctx.log(format!("unzip {}", zip.display()));
        net::unzip(&zip, &pkg, &|n| Some(PathBuf::from(n)), &ctx.shared.cancel, &|d, t| ctx.set("package", d as f64 / t.max(1) as f64, ""))?;
        if req.local_zip.is_none() {
            let _ = fs::remove_file(&zip);
        }
        if req.mode == "update" {
            // a new runtime: the exe is linked again (its objects are kept when their source did not change)
            let _ = fs::remove_file(pkg.join("xc2_boot.exe"));
        }
    }
    marker.tag = wanted.map(|v| format!("v{v}")).or_else(|| instance::package_version(inst).map(|v| format!("v{v}"))).unwrap_or_default();
    marker.channel = req.channel.clone();
    marker.source = source.display().to_string();
    instance::write_marker(inst, &marker)?;

    // 2. Wine: the launcher's portable Wine 11.22 (downloaded once), and the prefix prepared without any window
    ctx.set("prefix", -1.0, "");
    if req.wine.trim().is_empty() && crate::deps::own_wine().is_none() {
        ctx.log(format!("downloading Wine {}", crate::deps::WINE_VERSION));
        crate::deps::install_wine(&ctx.shared.cancel, &|_, d, t| {
            ctx.set("prefix", if t > 0 { d as f64 / t as f64 } else { -1.0 }, format!("Wine {} · {:.0} / {:.0} MB", crate::deps::WINE_VERSION, d as f64 / 1e6, t as f64 / 1e6))
        })?;
    }
    let wine = crate::launch::wine(&req.wine).ok_or("no-wine")?;
    ctx.log(format!("wine {}", wine.display()));
    crate::deps::prepare_prefix(&wine, &inst.join("prefix"))?;

    // 3. a .nup is unpacked here, natively and file by file (the engine's own extractor, Windows Python under Wine, holds
    //    the package in memory: over 20 GB, and the system's OOM killer ends it on a 16 GB PC). The engine then gets the
    //    folder, which it links into pkg/ (no second copy).
    clear_partial_extraction(ctx, &pkg, &source);
    let input = if source.is_file() {
        ctx.set("extract", 0.0, "");
        let out = inst.join("dump");
        let mut last = 0u64;
        crate::nup::extract(
            &source,
            &out,
            &mut |done, total, _file| {
                if total > 0 && (done == total || done.saturating_sub(last) >= total / 400) {
                    last = done;
                    ctx.set("extract", done as f64 / total as f64, format!("{:.1} / {:.1} GB", done as f64 / 1e9, total as f64 / 1e9));
                }
            },
            &|| ctx.cancelled(),
        )?;
        ctx.log(format!("unpacked {}", out.display()));
        pick_dump(&out)?
    } else {
        source.clone()
    };

    // 4. lift (the engine gets a plain-named link when the dump's path has characters outside ASCII)
    ctx.set("lift", -1.0, "");
    let engine_source = ascii_source(inst, &input)?;
    if engine_source != source {
        ctx.log(format!("source link {} -> {}", engine_source.display(), source.display()));
    }
    engine(ctx, req, inst, &engine_source, "lift", "link,shaders,done")?;
    native_links(&|l| ctx.log(l), &pkg, &input, &inst.join("dump/dlc"));

    // 4-5. zig and the native link
    if !pkg.join("xc2_boot.exe").is_file() {
        ctx.set("zig", -1.0, "");
        let zig = net::zig(&ctx.instances, &ctx.shared.cancel, &|_, d, t| ctx.set("zig", d as f64 / t.max(1) as f64, format!("{:.0} MB", d as f64 / 1e6)))?;
        ctx.log(format!("zig {}", zig.display()));
        ctx.set("link", 0.0, "");
        link(ctx, &zig, inst, req.jobs)?;
    } else {
        ctx.log("xc2_boot.exe is already built");
    }

    // 6. shaders, nxrt.ini, launch.env
    ctx.set("shaders", -1.0, "");
    engine(ctx, req, inst, &engine_source, "shaders", "lift,link")?;
    native_links(&|l| ctx.log(l), &pkg, &input, &inst.join("dump/dlc"));

    // 7. done
    ctx.set("done", -1.0, "");
    if !inst.join("opciones.conf").is_file() {
        instance::write_options(inst, &instance::Options::default())?;
    }
    copy_cover(&source, inst);
    marker.built = util::now();
    instance::write_marker(inst, &marker)?;
    Ok(())
}

pub fn shared() -> Arc<Shared> {
    Arc::new(Shared::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wine_junctions_become_linux_links() {
        let d = std::env::temp_dir().join(format!("xc2l-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let input = d.join("dump/base");
        fs::create_dir_all(input.join("exefs")).unwrap();
        fs::write(input.join("exefs/main"), b"m").unwrap();
        fs::create_dir_all(input.join("romfs")).unwrap();
        fs::write(input.join("romfs/bf2.ard"), b"a").unwrap();
        fs::create_dir_all(d.join("dump/dlc/0100E95004039001/romfs")).unwrap();
        let pkg = d.join("pkg");
        // what Wine's mklink /J leaves: an empty "exefs?" and an empty romfs
        fs::create_dir_all(pkg.join("exefs?")).unwrap();
        fs::create_dir_all(pkg.join("romfs")).unwrap();
        native_links(&|_| {}, &pkg, &input, &d.join("dump/dlc"));
        assert!(pkg.join("exefs/main").is_file());
        assert!(!pkg.join("exefs?").exists());
        assert!(pkg.join("romfs/bf2.ard").is_file());
        assert!(pkg.join("game/romfs-301/bf2.ard").is_file());
        assert!(pkg.join("dlc/0100E95004039001/romfs").is_dir());
        assert!(pkg.join("game/dlc-301/0100E95004039001/romfs").is_dir());
        // a real folder with the files is kept
        let _ = fs::remove_file(pkg.join("romfs"));
        fs::create_dir_all(pkg.join("romfs")).unwrap();
        fs::write(pkg.join("romfs/bf2.ard"), b"real").unwrap();
        native_links(&|_| {}, &pkg, &input, &d.join("dump/dlc"));
        assert!(!fs::symlink_metadata(pkg.join("romfs")).unwrap().file_type().is_symlink());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn non_ascii_sources_get_a_plain_link() {
        let d = std::env::temp_dir().join(format!("xc2l-src-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("inst")).unwrap();
        let nup = d.join("Xenoblade Chronicles 2 – Nintendo Switch 2 Edition [3.0.1].nup");
        fs::write(&nup, b"NUP\x01").unwrap();
        let plain = d.join("plain.nup");
        fs::write(&plain, b"x").unwrap();
        assert_eq!(ascii_source(&d.join("inst"), &plain).unwrap(), plain);
        let l = ascii_source(&d.join("inst"), &nup).unwrap();
        assert_eq!(l, d.join("inst/source.nup"));
        assert!(l.to_string_lossy().is_ascii());
        assert_eq!(fs::read(&l).unwrap(), b"NUP\x01");
        // a second run replaces the link
        assert!(ascii_source(&d.join("inst"), &nup).is_ok());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn input_args_by_layout() {
        let d = std::env::temp_dir().join(format!("xc2l-in-{}", std::process::id()));
        fs::create_dir_all(d.join("exefs")).unwrap();
        fs::create_dir_all(d.join("romfs")).unwrap();
        let a = input_args(&d);
        assert_eq!(a[0], "--exefs");
        assert!(a[1].ends_with("\\exefs"));
        assert_eq!(a[2], "--romfs");
        let _ = fs::remove_dir_all(&d);
    }
}
