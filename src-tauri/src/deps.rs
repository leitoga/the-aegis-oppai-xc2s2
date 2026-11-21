// SPDX-License-Identifier: GPL-3.0-or-later
//! What the game needs from the system, installed for the player: Wine with the distribution's own package manager (the
//! system's password window through pkexec, or a terminal running sudo when there is no polkit agent), and GE-Proton for DLSS,
//! downloaded into the user's own folders (no password).
//!
//! The launcher never sees the password: pkexec's window or sudo in the terminal asks for it.

use crate::util::{self, host_cmd};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Distro {
    /// arch, debian, fedora, suse, unknown
    pub family: String,
    pub name: String,
    /// the command that installs Wine (and gamemode), with sudo
    pub command: String,
    #[serde(skip)]
    ubuntu_codename: Option<String>,
}

pub fn family_of(os_release: &str) -> (String, String) {
    let field = |k: &str| {
        os_release.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_lowercase()).unwrap_or_default()
    };
    let id = field("ID");
    let like = field("ID_LIKE");
    let name = os_release
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| id.clone());
    let all = format!("{id} {like}");
    let has = |w: &str| all.split_whitespace().any(|x| x == w);
    let fam = if has("arch") || has("cachyos") || has("manjaro") || has("endeavouros") {
        "arch"
    } else if has("debian") || has("ubuntu") {
        "debian"
    } else if has("fedora") || has("rhel") || has("centos") || has("nobara") {
        "fedora"
    } else if has("suse") || has("opensuse") || id.starts_with("opensuse") {
        "suse"
    } else {
        "unknown"
    };
    (fam.to_string(), name)
}

/// The package manager's arguments (without sudo/pkexec), non-interactive.
fn install_args(family: &str) -> Option<Vec<&'static str>> {
    Some(match family {
        "arch" => vec!["pacman", "-S", "--needed", "--noconfirm", "wine", "gamemode"],
        _ => return None,
    })
}

pub fn distro() -> Distro {
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let (family, name) = family_of(&release);
    let field = |key: &str| release.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).map(|v| v.trim_matches('"').to_string()).unwrap_or_default();
    let id = field("ID").to_lowercase();
    let ubuntu_code = field("UBUNTU_CODENAME");
    let ubuntu_like = id == "ubuntu" || !ubuntu_code.is_empty() || field("ID_LIKE").to_lowercase().split_whitespace().any(|x| x == "ubuntu");
    let codename = if !ubuntu_code.is_empty() { ubuntu_code } else { field("VERSION_CODENAME") };
    let ubuntu_codename = (family == "debian" && ubuntu_like && !codename.is_empty()
        && codename.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).then_some(codename);
    let command = if ubuntu_codename.is_some() { "sudo apt-get install winehq-stable (Wine 11)".into() }
        else { install_args(&family).map(|a| format!("sudo {}", a.join(" "))).unwrap_or_default() };
    Distro { family, name, command, ubuntu_codename }
}

/// Reads the version from the same Wine executable that the launcher will use.
pub fn wine_version(path: &str) -> Option<String> {
    let out = Command::new(path).arg("--version").output().ok()?;
    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    Some(version.to_string())
}

pub fn is_wine_11_or_newer(version: Option<&str>) -> bool {
    version.and_then(|v| v.split('.').next()).and_then(|v| v.parse::<u32>().ok()).is_some_and(|major| major >= 11)
}

fn ubuntu_wine11_command(codename: &str) -> String {
    format!("export DEBIAN_FRONTEND=noninteractive; apt-get update && apt-get install -y ca-certificates wget && dpkg --add-architecture i386 && install -d -m 0755 /etc/apt/keyrings && wget -qO /etc/apt/keyrings/winehq-archive.key https://dl.winehq.org/wine-builds/winehq.key && wget -qO /etc/apt/sources.list.d/winehq-{codename}.sources https://dl.winehq.org/wine-builds/ubuntu/dists/{codename}/winehq-{codename}.sources && apt-get update && apt-get install -y --install-recommends winehq-stable gamemode")
}

fn installer_args(d: &Distro) -> Option<Vec<String>> {
    if let Some(codename) = &d.ubuntu_codename {
        Some(vec!["sh".into(), "-c".into(), ubuntu_wine11_command(codename)])
    } else {
        install_args(&d.family).map(|args| args.iter().map(|s| s.to_string()).collect())
    }
}

/// The program names of the running processes (from /proc/<pid>/cmdline: `comm` and `pgrep -x` stop at 15 characters, and
/// the agents' names are longer).
fn running_programs() -> Vec<String> {
    let mut v = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else { return v };
    for e in rd.flatten() {
        if !e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(cmd) = fs::read(e.path().join("cmdline")) else { continue };
        let first = cmd.split(|b| *b == 0).next().unwrap_or_default();
        let name = String::from_utf8_lossy(first);
        if let Some(base) = name.rsplit('/').next() {
            if !base.is_empty() {
                v.push(base.to_string());
            }
        }
    }
    v
}

/// Whether a polkit agent (the window that asks for the password) runs in this session: KDE, GNOME (built into the shell),
/// XFCE, MATE, LXQt, Cinnamon, Hyprland...
pub fn polkit_agent(programs: &[String]) -> bool {
    programs.iter().any(|p| {
        let p = p.to_lowercase();
        (p.contains("polkit") && (p.contains("agent") || p.contains("auth"))) || p == "gnome-shell" || p.contains("policykit") || p == "cinnamon"
    })
}

/// pkexec with a polkit agent running.
fn graphical_auth() -> bool {
    util::which("pkexec").is_some() && polkit_agent(&running_programs())
}

/// Installs Wine with the system's password window (pkexec). Lines of the package manager go to `log`. Err("no-graphical")
/// when there is no password window, Err("unknown-distro") when the distribution is not known.
pub fn install_wine_graphical(log: &(dyn Fn(String) + Sync)) -> Result<(), String> {
    let d = distro();
    let args = installer_args(&d).ok_or("unknown-distro")?;
    if !graphical_auth() {
        return Err("no-graphical".into());
    }
    let mut c = host_cmd("pkexec");
    c.args(&args);
    c.env("DEBIAN_FRONTEND", "noninteractive").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("pkexec: {e}"))?;
    let err = child.stderr.take().unwrap();
    let out = child.stdout.take().unwrap();
    std::thread::scope(|s| {
        s.spawn(|| {
            for l in BufReader::new(err).lines().map_while(Result::ok) {
                log(l);
            }
        });
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            log(l);
        }
    });
    let st = child.wait().map_err(|e| e.to_string())?;
    match st.code() {
        Some(0) => Ok(()),
        // pkexec: 126 the password window was cancelled, 127 not authorized
        Some(126) | Some(127) => Err("cancelled".into()),
        Some(n) => Err(format!("exit {n}")),
        None => Err("killed".into()),
    }
}

/// Terminal emulators and how each runs a command, most common desktops first.
fn terminal() -> Option<(PathBuf, Vec<&'static str>)> {
    let list: [(&str, &[&str]); 12] = [
        ("konsole", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("kgx", &["--"]),
        ("ptyxis", &["--"]),
        ("xfce4-terminal", &["-x"]),
        ("mate-terminal", &["-x"]),
        ("tilix", &["-e"]),
        ("alacritty", &["-e"]),
        ("kitty", &[]),
        ("foot", &[]),
        ("wezterm", &["start", "--"]),
        ("xterm", &["-e"]),
    ];
    list.iter().find_map(|(t, a)| util::which(t).map(|p| (p, a.to_vec())))
}

/// Plan B: a terminal with the install command; sudo asks for the password there.
pub fn install_wine_terminal(lang_es: bool) -> Result<String, String> {
    let d = distro();
    let args = installer_args(&d).ok_or("unknown-distro")?;
    let (term, pre) = terminal().ok_or("no-terminal")?;
    let cmd = if d.ubuntu_codename.is_some() { format!("sudo bash -c '{}'", args[2].replace('\'', "'\\''")) }
        else { format!("sudo {}", args.join(" ")) };
    let (hello, done, fail) = if lang_es {
        ("Se va a instalar Wine 11 (y gamemode) para The Aegis’s Oppai. Escribe tu contraseña cuando la pida.", "Listo. Ya puedes cerrar esta ventana y volver al launcher.", "No se pudo instalar. Revisa el mensaje de arriba.")
    } else {
        ("Installing Wine 11 (and gamemode) for The Aegis’s Oppai. Type your password when asked.", "Done. You can close this window and go back to the launcher.", "The install failed. See the message above.")
    };
    let script = format!("echo '{hello}'; echo; echo \"$ {cmd}\"; if {cmd}; then echo; echo '{done}'; else echo; echo '{fail}'; fi; echo; read -r -p '[Enter]' _");
    let mut c = host_cmd(&term);
    c.args(&pre).args(["bash", "-c", &script]);
    c.spawn().map_err(|e| format!("{}: {e}", term.display()))?;
    Ok(term.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
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

pub fn graphical_available() -> bool {
    graphical_auth()
}

pub fn terminal_name() -> Option<String> {
    terminal().and_then(|(p, _)| p.file_name().map(|n| n.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn families() {
        assert_eq!(family_of("ID=cachyos\nID_LIKE=arch\n").0, "arch");
        assert_eq!(family_of("ID=ubuntu\nID_LIKE=debian\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n"), ("debian".into(), "Ubuntu 24.04 LTS".into()));
        assert_eq!(family_of("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n").0, "debian");
        assert_eq!(family_of("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n").0, "suse");
        assert_eq!(family_of("ID=fedora\n").0, "fedora");
        assert_eq!(family_of("ID=nixos\n").0, "unknown");
        assert!(install_args("debian").is_none(), "only Ubuntu-based systems use WineHQ's Wine 11 package");
        let ps = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(polkit_agent(&ps(&["systemd", "polkit-kde-authentication-agent-1"])));
        assert!(polkit_agent(&ps(&["gnome-shell"])));
        assert!(polkit_agent(&ps(&["lxqt-policykit-agent"])));
        assert!(!polkit_agent(&ps(&["polkitd", "bash"])), "the daemon alone asks nobody");
    }
}
