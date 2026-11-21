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

pub const STEPS: [&str; 7] = ["package", "prefix", "lift", "zig", "link", "shaders", "done"];

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
    c.env("WINEPREFIX", inst.join("prefix")).env("WINEDEBUG", "-all").env("WINEDLLOVERRIDES", "mscoree,mshtml=");
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

    // 2. the Wine prefix
    ctx.set("prefix", -1.0, "");
    if !inst.join("prefix/system.reg").is_file() {
        let mut c = wine_cmd(req, inst)?;
        c.arg("wineboot").arg("-u");
        ctx.run(c, &|_| {})?;
    }

    // 3. lift
    ctx.set("lift", -1.0, "");
    engine(ctx, req, inst, &source, "lift", "link,shaders,done")?;

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
    engine(ctx, req, inst, &source, "shaders", "lift,link")?;

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
