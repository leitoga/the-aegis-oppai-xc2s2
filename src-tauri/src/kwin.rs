// SPDX-License-Identifier: GPL-3.0-or-later
//! KDE Plasma: once the game draws, Proton's console window (the game is a console program) is minimized and the game's
//! window is focused. Done with a KWin script over D-Bus (Plasma 5 and 6); elsewhere nothing is done.

use crate::util::{self, host_cmd};
use std::fs;

const RELEASE: &str = "aegis-oppai-release";

pub fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").map(|d| d.to_uppercase().contains("KDE")).unwrap_or(false)
}

const IS_GAME: &str = r#"
function isGameish(w) {
    if (!w) return false;
    var c = String(w.resourceClass).toLowerCase(), n = String(w.resourceName).toLowerCase(), t = String(w.caption).toLowerCase();
    return c.indexOf("xc2_boot") >= 0 || n.indexOf("xc2_boot") >= 0 || c.indexOf("steam_app_") == 0 || t.indexOf("xenoblade") >= 0;
}
// Proton opens a console window for the game (its title is the exe's path): kept minimized
function isConsole(w) { return isGameish(w) && /\.exe$/i.test(String(w.caption)); }
function isGame(w) { return isGameish(w) && !isConsole(w); }
function windows() { return workspace.windowList ? workspace.windowList() : workspace.clientList(); }
function activate(w) { if (workspace.activeWindow !== undefined) workspace.activeWindow = w; else workspace.activeClient = w; }
"#;

fn dbus(args: &[&str]) -> Option<String> {
    let o = host_cmd("dbus-send").args(["--session", "--print-reply", "--dest=org.kde.KWin"]).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn unload(name: &str) {
    let _ = dbus(&["/Scripting", "org.kde.kwin.Scripting.unloadScript", &format!("string:{name}")]);
}

fn run(name: &str, js: &str) {
    let path = util::data_dir().join(format!("{name}.js"));
    if util::write_atomic(&path, js).is_err() {
        return;
    }
    unload(name);
    let Some(reply) = dbus(&["/Scripting", "org.kde.kwin.Scripting.loadScript", &format!("string:{}", path.display()), &format!("string:{name}")]) else {
        return;
    };
    let Some(id) = reply.split_whitespace().skip_while(|w| *w != "int32").nth(1) else { return };
    // Plasma 6 names the object /Scripting/Script<N>, Plasma 5 /<N>
    if dbus(&[&format!("/Scripting/Script{id}"), "org.kde.kwin.Script.run"]).is_none() {
        let _ = dbus(&[&format!("/{id}"), "org.kde.kwin.Script.run"]);
    }
}

/// The game draws: the console minimized, the game's window in front and focused.
pub fn release_game() {
    if !is_kde() {
        return;
    }
    let js = format!(
        r#"{IS_GAME}
var l = windows();
for (var i = 0; i < l.length; i++) {{
    var w = l[i];
    if (isConsole(w)) {{ w.minimized = true; continue; }}
    if (!isGame(w)) continue;
    w.keepBelow = false;
    activate(w);
}}
"#
    );
    run(RELEASE, &js);
    std::thread::sleep(std::time::Duration::from_millis(400));
    unload(RELEASE);
    let _ = fs::remove_file(util::data_dir().join(format!("{RELEASE}.js")));
}
