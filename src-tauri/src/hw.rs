// SPDX-License-Identifier: GPL-3.0-or-later
//! What this PC has: the monitors' modes (for the output resolution and the window size) and the game controllers (to pick one).

use crate::util::host_cmd;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    /// the mode in use, "WxH"
    pub current: String,
    /// every resolution it offers, largest first, "WxH"
    pub modes: Vec<String>,
}

fn sorted(set: BTreeSet<(u32, u32)>) -> Vec<String> {
    let mut v: Vec<(u32, u32)> = set.into_iter().collect();
    v.sort_by(|a, b| (b.0 * b.1, b.0).cmp(&(a.0 * a.1, a.0)));
    v.into_iter().map(|(w, h)| format!("{w}x{h}")).collect()
}

/// KDE (Wayland and X11): `kscreen-doctor -j`.
fn kscreen() -> Option<Vec<Display>> {
    let o = host_cmd("kscreen-doctor").arg("-j").output().ok()?;
    if !o.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).ok()?;
    let mut out = Vec::new();
    for d in v["outputs"].as_array()? {
        if !d["enabled"].as_bool().unwrap_or(false) {
            continue;
        }
        let mut set = BTreeSet::new();
        let mut current = String::new();
        let cur_id = d["currentModeId"].as_str().map(String::from).or_else(|| d["currentModeId"].as_u64().map(|n| n.to_string()));
        for m in d["modes"].as_array().cloned().unwrap_or_default() {
            let (w, h) = (m["size"]["width"].as_u64().unwrap_or(0) as u32, m["size"]["height"].as_u64().unwrap_or(0) as u32);
            if w == 0 || h == 0 {
                continue;
            }
            set.insert((w, h));
            let id = m["id"].as_str().map(String::from).or_else(|| m["id"].as_u64().map(|n| n.to_string()));
            if id.is_some() && id == cur_id {
                current = format!("{w}x{h}");
            }
        }
        out.push(Display { name: d["name"].as_str().unwrap_or("?").to_string(), current, modes: sorted(set) });
    }
    (!out.is_empty()).then_some(out)
}

/// Everything else: `xrandr` (X11, or XWayland, where it lists at least the current mode).
pub fn parse_xrandr(text: &str) -> Vec<Display> {
    let mut out: Vec<Display> = Vec::new();
    for line in text.lines() {
        if !line.starts_with(' ') {
            let mut w = line.split_whitespace();
            if let (Some(name), Some("connected")) = (w.next(), w.next()) {
                out.push(Display { name: name.to_string(), current: String::new(), modes: Vec::new() });
            } else {
                out.push(Display { name: String::new(), current: String::new(), modes: Vec::new() });
            }
            continue;
        }
        let Some(d) = out.last_mut() else { continue };
        if d.name.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(mode) = parts.next() else { continue };
        let Some((w, h)) = mode.split_once('x') else { continue };
        let h: String = h.chars().take_while(|c| c.is_ascii_digit()).collect();
        if w.parse::<u32>().is_err() || h.parse::<u32>().is_err() {
            continue;
        }
        let m = format!("{w}x{h}");
        if parts.any(|r| r.contains('*')) {
            d.current = m.clone();
        }
        if !d.modes.contains(&m) {
            d.modes.push(m);
        }
    }
    out.retain(|d| !d.name.is_empty() && !d.modes.is_empty());
    out
}

pub fn displays() -> Vec<Display> {
    if let Some(d) = kscreen() {
        return d;
    }
    host_cmd("xrandr").arg("--query").output().ok().map(|o| parse_xrandr(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Pad {
    pub name: String,
    /// "vvvv:pppp" (USB vendor and product, hex): what MANDO= holds
    pub id: String,
}

/// Game controllers from /proc/bus/input/devices (the ones with a jsN handler).
pub fn parse_input_devices(text: &str) -> Vec<Pad> {
    let mut out: Vec<Pad> = Vec::new();
    for block in text.split("\n\n") {
        let is_pad = block.lines().any(|l| l.starts_with("H: Handlers=") && l.split_whitespace().any(|h| h.starts_with("js")));
        if !is_pad {
            continue;
        }
        let mut name = String::new();
        let (mut vid, mut pid) = (String::new(), String::new());
        for l in block.lines() {
            if let Some(n) = l.strip_prefix("N: Name=") {
                name = n.trim_matches('"').to_string();
            } else if let Some(i) = l.strip_prefix("I: ") {
                for kv in i.split_whitespace() {
                    if let Some(v) = kv.strip_prefix("Vendor=") {
                        vid = v.to_lowercase();
                    } else if let Some(p) = kv.strip_prefix("Product=") {
                        pid = p.to_lowercase();
                    }
                }
            }
        }
        let id = format!("{vid}:{pid}");
        if !vid.is_empty() && !out.iter().any(|p| p.id == id) {
            out.push(Pad { name, id });
        }
    }
    out
}

pub fn pads() -> Vec<Pad> {
    std::fs::read_to_string("/proc/bus/input/devices").map(|t| parse_input_devices(&t)).unwrap_or_default()
}

/// SDL's variable that makes the game see only the chosen controller ("vvvv:pppp" -> "0xvvvv/0xpppp").
pub fn sdl_only(id: &str) -> Option<String> {
    let (v, p) = id.split_once(':')?;
    let hex = |s: &str| s.len() == 4 && s.chars().all(|c| c.is_ascii_hexdigit());
    (hex(v) && hex(p)).then(|| format!("0x{v}/0x{p}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xrandr_modes() {
        let t = "Screen 0: minimum 16 x 16\nHDMI-A-2 connected primary 3840x2160+0+0\n   3840x2160     60.00*+  30.00\n   2560x1440     59.95\n   1920x1080i    60.00\nDP-1 disconnected\n";
        let d = parse_xrandr(t);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].current, "3840x2160");
        assert_eq!(d[0].modes, vec!["3840x2160", "2560x1440", "1920x1080"]);
    }

    #[test]
    fn joysticks_only() {
        let t = "I: Bus=0003 Vendor=045e Product=028e Version=0120\nN: Name=\"Microsoft X-Box 360 pad\"\nH: Handlers=event20 js0\n\nI: Bus=0003 Vendor=046d Product=c52b Version=0111\nN: Name=\"Logitech Mouse\"\nH: Handlers=mouse0 event3\n";
        let p = parse_input_devices(t);
        assert_eq!(p, vec![Pad { name: "Microsoft X-Box 360 pad".into(), id: "045e:028e".into() }]);
        assert_eq!(sdl_only("045e:028e").as_deref(), Some("0x045e/0x028e"));
        assert_eq!(sdl_only("auto"), None);
    }
}
