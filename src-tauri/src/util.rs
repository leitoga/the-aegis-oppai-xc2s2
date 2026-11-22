// SPDX-License-Identifier: GPL-3.0-or-later
//! Small helpers: commands that run on the host (not in the AppImage's environment), Wine paths, folders.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The user's home folder.
pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_DATA_HOME/aegis-oppai`: the zig toolchain and downloads that are not part of an instance.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
    base.join("aegis-oppai")
}

/// `$XDG_CACHE_HOME/aegis-oppai`: downloads in progress.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache"));
    base.join("aegis-oppai")
}

/// `~/...` for paths under the home folder, as the UI shows them.
pub fn tilde(p: &Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

/// A Linux path as Wine sees it on its Z: drive.
pub fn win(p: &Path) -> String {
    format!("Z:{}", p.display().to_string().replace('/', "\\"))
}

/// The first executable called `name` on the host's PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = host_var("PATH").unwrap_or_else(|| "/usr/local/bin:/usr/bin:/bin".into());
    path.split(':').map(|d| Path::new(d).join(name)).find(|p| is_exec(p))
}

pub fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

fn appdir() -> Option<String> {
    std::env::var("APPDIR").ok().filter(|s| !s.is_empty())
}

/// A host variable with the AppImage's own entries taken out (PATH-like lists keep the host's part).
pub fn host_var(key: &str) -> Option<String> {
    let v = std::env::var(key).ok()?;
    let Some(app) = appdir() else { return Some(v) };
    if !v.contains(&app) {
        return Some(v);
    }
    let kept: Vec<&str> = v.split(':').filter(|s| !s.is_empty() && !s.contains(&app)).collect();
    (!kept.is_empty()).then(|| kept.join(":"))
}

/// Variables the AppImage's AppRun (and the GTK hooks inside it) set for the launcher's own WebKit, which must not reach
/// Wine, Proton, Python or zig.
const APPIMAGE_ONLY: &[&str] = &[
    "APPDIR", "APPIMAGE", "ARGV0", "OWD", "GDK_PIXBUF_MODULE_FILE", "GDK_PIXBUF_MODULEDIR", "GIO_EXTRA_MODULES", "GIO_MODULE_DIR",
    "GTK_PATH", "GTK_EXE_PREFIX", "GTK_DATA_PREFIX", "GTK_IM_MODULE_FILE", "GTK_THEME", "GSETTINGS_SCHEMA_DIR", "PYTHONHOME",
    "PYTHONPATH", "PERLLIB", "QT_PLUGIN_PATH", "WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE",
    "LIBGL_DRIVERS_PATH", "GST_PLUGIN_SYSTEM_PATH", "GST_PLUGIN_SYSTEM_PATH_1_0", "GST_PLUGIN_PATH", "GST_PLUGIN_SCANNER",
];

/// A command with the host's environment: the AppImage's variables removed, its folders taken out of the PATH-like ones.
pub fn host_cmd<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let mut c = Command::new(program);
    for k in APPIMAGE_ONLY {
        c.env_remove(k);
    }
    for k in ["PATH", "LD_LIBRARY_PATH", "LD_PRELOAD", "XDG_DATA_DIRS", "XDG_CONFIG_DIRS"] {
        match host_var(k) {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    c
}

/// Opens a folder or file with the desktop's default application.
pub fn open_path(p: &Path) -> Result<(), String> {
    host_cmd("xdg-open").arg(p).spawn().map(|_| ()).map_err(|e| format!("xdg-open: {e}"))
}

/// Seconds since the epoch of a file's modification time.
pub fn mtime(p: &Path) -> Option<u64> {
    p.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Total size of a folder tree in bytes (symlinks are not followed: the instance links its romfs to the dump).
pub fn dir_size(p: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else if let Ok(m) = e.metadata() {
                use std::os::unix::fs::MetadataExt;
                total += m.blocks() * 512;
            }
        }
    }
    total
}

/// Whether a process with exactly this name runs (`pgrep -x`, never `-f`).
pub fn process_running(name: &str) -> bool {
    host_cmd("pgrep").arg("-x").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Writes a file through a temporary file and a rename.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let tmp = path.with_extension("tmp-xc2l");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wine_paths() {
        assert_eq!(win(Path::new("/home/a/b c/pkg")), "Z:\\home\\a\\b c\\pkg");
    }
}
