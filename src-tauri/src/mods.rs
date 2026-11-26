//! Mods: one folder per mod in `<pkg>/mods`, which the runtime lays over the game's files at start (`[fs] mods`; see
//! the port's docs/MODS.md). Switches live in nxrt.ini as `[mods] disabled = a,b` (folder names), the runtime's own key.
//!
//! Most of the game's files are inside bf2.ard. The runtime serves a replacement for such a file only from records
//! prepared beforehand, which the port builds with tools/mods/ard_pack.py (Python, not in the release package). This
//! module does that work in the launcher, after a mod is added and again before a start when the mod or the
//! game's archive index changed. Every replaced file goes into `<mod>/.ard/append.bin` (raw entries as they are,
//! the others as an xbc1 record: header + zlib), and a copy of bf2.arh with those entries pointing past the archive is
//! written to `<romfs>/bf2.arh`; the runtime moves each mod's tail after the ones before it, so every mod is prepared
//! as if it were alone.
//!
//! No `.rec` files (ard_pack.py's in-place records): the runtime looks a `.rec` up by the name in the original record's
//! header (`bl000201.wimdo`, no folders, at most 27 bytes) while ard_pack.py files it under the full path
//! (`model/bl/bl000201.wimdo.rec`), so such a record was never served (seen 2026-11-26: a model's .wimdo kept the
//! original while its appended .wismt was the mod's). A repointed index entry is exact and has no name to match.
//! Loose romfs files (stream/, sound/, ...) need nothing: the runtime overlays them by path.

use flate2::{write::ZlibEncoder, Compression};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The application id an emulator-layout mod files base-game content under (docs/MODS.md).
const APP_ID: &str = "0100E95004038000";
/// Written into `<mod>/.ard` after a preparation: what it was made from, so a start only redoes it when needed.
const STAMP: &str = "prepared.txt";

#[derive(Serialize, Clone)]
pub struct Mod {
    pub name: String,
    pub enabled: bool,
    /// files the mod replaces or adds (its romfs tree, without the prepared index)
    pub files: u32,
    pub size: u64,
    /// romfs folder found (plain or emulator layout)
    pub romfs: bool,
    /// code patches (exefs) or cheats: the launcher does not apply those
    pub code: bool,
    pub cheats: bool,
    /// the last preparation is still valid
    pub prepared: bool,
    /// files of the last preparation that could not be used
    pub skipped: Vec<String>,
}

#[derive(Serialize, Default, Clone)]
pub struct Prepared {
    pub records: u32,
    pub appended: u32,
    pub loose: u32,
    pub skipped: Vec<String>,
}

pub fn dir(pkg: &Path) -> PathBuf {
    pkg.join("mods")
}

fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('.') || name.contains('/') || name.contains('\\') || name.contains(',') {
        return Err("bad-mod-name".into());
    }
    Ok(())
}

/// The mod's romfs tree, in the order the runtime looks (fs_romfs.cpp mod_romfs_root).
pub fn romfs_root(m: &Path) -> Option<PathBuf> {
    let mut c = vec![m.join("romfs")];
    for id in [APP_ID.to_string(), APP_ID.to_lowercase()] {
        c.push(m.join("atmosphere/contents").join(&id).join("romfs"));
        c.push(m.join("contents").join(&id).join("romfs"));
        c.push(m.join(&id).join("romfs"));
    }
    c.into_iter().find(|p| p.is_dir())
}

fn has_part(m: &Path, part: &str) -> bool {
    if m.join(part).is_dir() {
        return true;
    }
    [APP_ID.to_string(), APP_ID.to_lowercase()].iter().any(|id| {
        m.join("atmosphere/contents").join(id).join(part).is_dir() || m.join("contents").join(id).join(part).is_dir() || m.join(id).join(part).is_dir()
    }) || (part == "exefs" && m.join("atmosphere/exefs_patches").is_dir())
}

// ---- nxrt.ini [mods] disabled ----

fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    let header = format!("[{section}]");
    let mut in_sec = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            in_sec = t == header;
        } else if in_sec {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn disabled(pkg: &Path) -> Vec<String> {
    let t = fs::read_to_string(pkg.join("nxrt.ini")).unwrap_or_default();
    ini_get(&t, "mods", "disabled").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

fn write_disabled(pkg: &Path, list: &[String]) -> Result<(), String> {
    let ini = pkg.join("nxrt.ini");
    let t = fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let n = crate::launch::set_ini(&t, "mods", "disabled", &list.join(","));
    if n != t {
        fs::write(&ini, n).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn set_enabled(pkg: &Path, name: &str, on: bool) -> Result<(), String> {
    safe_name(name)?;
    let mut d = disabled(pkg);
    d.retain(|x| x != name);
    if !on {
        d.push(name.to_string());
    }
    write_disabled(pkg, &d)
}

// ---- listing ----

fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(root) else { return };
    let mut v: Vec<_> = rd.flatten().collect();
    v.sort_by_key(|e| e.file_name());
    for e in v {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk(&e.path(), out);
        } else if ft.is_file() {
            out.push(e.path());
        }
    }
}

/// The mod's own files: its romfs tree without the bf2.arh this module writes there.
fn mod_files(root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    walk(root, &mut v);
    v.retain(|p| !(p.parent() == Some(root) && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("bf2.arh"))));
    v
}

pub fn list(pkg: &Path) -> Vec<Mod> {
    // the folder the runtime reads ([fs] mods), there from the start so "Open folder" has somewhere to go
    if pkg.is_dir() {
        let _ = fs::create_dir_all(dir(pkg));
    }
    let off = disabled(pkg);
    let Ok(rd) = fs::read_dir(dir(pkg)) else { return vec![] };
    let mut out: Vec<Mod> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let m = e.path();
            let root = romfs_root(&m);
            let files = root.as_deref().map(mod_files).unwrap_or_default();
            let size = files.iter().filter_map(|p| p.metadata().ok()).map(|md| md.len()).sum();
            // the first line of the stamp file is what it was made from; the lines after it list the files left out
            let made = fs::read_to_string(m.join(".ard").join(STAMP)).ok().and_then(|t| t.lines().next().map(str::to_string));
            let prepared = root.as_deref().is_some_and(|r| stamp(pkg, r).is_some_and(|s| made.as_deref() == Some(s.as_str())));
            let skipped = fs::read_to_string(m.join(".ard").join(STAMP))
                .ok()
                .map(|t| t.lines().skip(1).filter_map(|l| l.strip_prefix("skipped ").map(str::to_string)).collect())
                .unwrap_or_default();
            Some(Mod {
                enabled: !off.contains(&name),
                files: files.len() as u32,
                size,
                romfs: root.is_some(),
                code: has_part(&m, "exefs"),
                cheats: has_part(&m, "cheats"),
                prepared,
                skipped,
                name,
            })
        })
        .collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    out
}

// ---- adding and removing ----

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let ft = e.file_type().map_err(|e| e.to_string())?;
        let dest = to.join(e.file_name());
        if ft.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else if ft.is_file() {
            fs::copy(e.path(), &dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// A downloaded archive often wraps the mod in one more folder: the folder that holds romfs/ (or an emulator layout).
fn unwrap_root(d: &Path) -> PathBuf {
    let mut cur = d.to_path_buf();
    for _ in 0..3 {
        if romfs_root(&cur).is_some() || has_part(&cur, "exefs") || has_part(&cur, "cheats") {
            break;
        }
        let kids: Vec<_> = fs::read_dir(&cur).map(|r| r.flatten().collect()).unwrap_or_default();
        if kids.len() == 1 && kids[0].file_type().is_ok_and(|t| t.is_dir()) {
            cur = kids[0].path();
        } else {
            break;
        }
    }
    cur
}

/// Copies a mod folder or unpacks a .zip into `<pkg>/mods/<name>` and prepares it. Returns the mod's name.
pub fn add(pkg: &Path, src: &Path) -> Result<(String, Prepared), String> {
    let mods = dir(pkg);
    fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
    let is_zip = src.is_file() && src.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    let stem = if is_zip { src.file_stem() } else { src.file_name() };
    let name: String = stem.map(|s| s.to_string_lossy().replace(',', " ").trim().to_string()).unwrap_or_default();
    safe_name(&name)?;
    let dest = mods.join(&name);
    if dest.exists() {
        return Err("mod-exists".into());
    }
    let tmp = mods.join(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let res = (|| {
        if is_zip {
            let f = fs::File::open(src).map_err(|e| e.to_string())?;
            let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
            for i in 0..z.len() {
                let mut e = z.by_index(i).map_err(|e| e.to_string())?;
                let Some(rel) = e.enclosed_name() else { continue };
                let out = tmp.join(rel);
                if e.is_dir() {
                    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                } else {
                    if let Some(p) = out.parent() {
                        fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                    let mut w = fs::File::create(&out).map_err(|e| e.to_string())?;
                    std::io::copy(&mut e, &mut w).map_err(|e| e.to_string())?;
                }
            }
        } else if src.is_dir() {
            copy_tree(src, &tmp)?;
        } else {
            return Err("not-a-mod".into());
        }
        let root = unwrap_root(&tmp);
        if romfs_root(&root).is_none() && !has_part(&root, "exefs") && !has_part(&root, "cheats") {
            return Err("not-a-mod".into());
        }
        fs::rename(&root, &dest).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_dir_all(&tmp);
    res?;
    let p = prepare(pkg, &name)?;
    Ok((name, p))
}

pub fn remove(pkg: &Path, name: &str) -> Result<(), String> {
    safe_name(name)?;
    let m = dir(pkg).join(name);
    if !m.is_dir() {
        return Err("no-mod".into());
    }
    fs::remove_dir_all(&m).map_err(|e| e.to_string())?;
    let mut d = disabled(pkg);
    if d.iter().any(|x| x == name) {
        d.retain(|x| x != name);
        write_disabled(pkg, &d)?;
    }
    Ok(())
}

// ---- the game's archive index (bf2.arh) ----

fn archive_dir(pkg: &Path) -> Option<PathBuf> {
    [pkg.join("game/romfs-301"), pkg.join("romfs")].into_iter().find(|d| d.join("bf2.arh").is_file() && d.join("bf2.ard").is_file())
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn xor4(data: &[u8], key: u32) -> Vec<u8> {
    let k = key.to_le_bytes();
    data.iter().enumerate().map(|(i, b)| b ^ k[i % 4]).collect()
}

/// Full logical path (lower case, no leading slash) -> entry indexes, from the radix trie and string table of an arh1
/// file (the port's tools/mods/arh_index.py, same walk).
pub fn arh_paths(a: &[u8]) -> HashMap<String, Vec<u32>> {
    let mut out: HashMap<String, Vec<u32>> = HashMap::new();
    if a.len() < 0x28 || &a[..4] != b"arh1" {
        return out;
    }
    let (str_off, str_sz, trie_off, trie_sz) = (u32_at(a, 0x0C) as usize, u32_at(a, 0x10) as usize, u32_at(a, 0x14) as usize, u32_at(a, 0x18) as usize);
    let (ent_off, ent_cnt, key) = (u32_at(a, 0x1C) as usize, u32_at(a, 0x20), u32_at(a, 0x24));
    if str_off < 0x28 || str_sz < 4 || trie_off < 0x28 || trie_sz < 8 || str_off + str_sz > a.len() || trie_off + trie_sz > a.len() || ent_off + ent_cnt as usize * 24 > a.len() {
        return out;
    }
    const TRIE_XOR: u32 = 0x0C0C_ACAC;
    let s = xor4(&a[str_off..str_off + str_sz], key ^ TRIE_XOR ^ 0xFFFF_FFFF);
    let tb = xor4(&a[trie_off..trie_off + trie_sz], key ^ TRIE_XOR);
    let trie: Vec<(i64, i64)> = tb
        .chunks_exact(8)
        .map(|c| (i32::from_le_bytes(c[0..4].try_into().unwrap()) as i64, i32::from_le_bytes(c[4..8].try_into().unwrap()) as i64))
        .collect();
    let n = trie.len() as i64;
    let edge_of = |child: i64, parent: i64| -> Option<i64> {
        if !(0..n).contains(&parent) {
            return None;
        }
        let pa = trie[parent as usize].0;
        if pa >= 0 {
            return None;
        }
        let e = (-pa - 1) ^ child;
        (0..256).contains(&e).then_some(e)
    };
    for node in 1..n {
        let (av, b) = trie[node as usize];
        if b >= 0 {
            continue;
        }
        let Some(edge) = edge_of(node, -b - 1) else { continue };
        let (tail, idx): (&[u8], u32) = if av > 0 {
            let start = av as usize + 1;
            if start > s.len() {
                continue;
            }
            let Some(nul) = s[start..].iter().position(|&c| c == 0).map(|p| p + start) else { continue };
            if nul + 5 > s.len() {
                continue;
            }
            (&s[start..nul], u32_at(&s, nul + 1))
        } else if edge == 0 && av < 0 {
            (&[], (-av - 1) as u32)
        } else {
            continue;
        };
        if idx >= ent_cnt {
            continue;
        }
        let mut prefix = Vec::new();
        let mut cur = node;
        let mut ok = true;
        let mut steps = 0;
        while cur > 0 {
            steps += 1;
            let cb = trie[cur as usize].1;
            if steps > 1024 || cb >= 0 {
                ok = false;
                break;
            }
            let p = -cb - 1;
            let Some(ch) = edge_of(cur, p) else {
                ok = false;
                break;
            };
            if ch != 0 {
                prefix.push(ch as u8);
            }
            cur = p;
        }
        if !ok {
            continue;
        }
        prefix.reverse();
        prefix.extend_from_slice(tail);
        let path = String::from_utf8_lossy(&prefix).trim_start_matches('/').to_lowercase();
        out.entry(path).or_default().push(idx);
    }
    out
}

struct Entry {
    idx: u32,
    offset: u64,
    comp: u32,
    unc: u32,
    flags: u32,
}

fn entry(a: &[u8], idx: u32) -> Entry {
    let p = u32_at(a, 0x1C) as usize + idx as usize * 24;
    Entry {
        idx,
        offset: u64::from_le_bytes(a[p..p + 8].try_into().unwrap()),
        comp: u32_at(a, p + 8),
        unc: u32_at(a, p + 12),
        flags: u32_at(a, p + 16),
    }
}

fn repoint(a: &mut [u8], idx: u32, offset: u64, comp: u32, unc: u32) {
    let p = u32_at(a, 0x1C) as usize + idx as usize * 24;
    a[p..p + 8].copy_from_slice(&offset.to_le_bytes());
    a[p + 8..p + 12].copy_from_slice(&comp.to_le_bytes());
    a[p + 12..p + 16].copy_from_slice(&unc.to_le_bytes());
}

fn xbc1_header(name: &str, unc: u32, comp: u32, checksum: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(0x30);
    h.extend_from_slice(b"xbc1");
    h.extend_from_slice(&1u32.to_le_bytes());
    h.extend_from_slice(&unc.to_le_bytes());
    h.extend_from_slice(&comp.to_le_bytes());
    h.extend_from_slice(&checksum.to_le_bytes());
    let mut nb = name.as_bytes()[..name.len().min(27)].to_vec();
    nb.resize(28, 0);
    h.extend_from_slice(&nb);
    h
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

fn mtime(p: &Path) -> u64 {
    p.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// What a preparation depends on: the game's archive index and the mod's files (path, size, time).
fn stamp(pkg: &Path, root: &Path) -> Option<String> {
    let ad = archive_dir(pkg)?;
    let arh = ad.join("bf2.arh");
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in mod_files(root) {
        p.strip_prefix(root).ok()?.to_string_lossy().hash(&mut h);
        p.metadata().map(|m| m.len()).unwrap_or(0).hash(&mut h);
        mtime(&p).hash(&mut h);
    }
    Some(format!("v2 arh {} {} ard {} files {:016x}", arh.metadata().ok()?.len(), mtime(&arh), ad.join("bf2.ard").metadata().ok()?.len(), h.finish()))
}

/// Builds the mod's archive records (see the module comment).
pub fn prepare(pkg: &Path, name: &str) -> Result<Prepared, String> {
    safe_name(name)?;
    let m = dir(pkg).join(name);
    let mut rep = Prepared::default();
    let Some(root) = romfs_root(&m) else { return Ok(rep) };
    let ad = archive_dir(pkg).ok_or("no-archive")?;
    let arh_orig = fs::read(ad.join("bf2.arh")).map_err(|e| e.to_string())?;
    let ard_size = ad.join("bf2.ard").metadata().map_err(|e| e.to_string())?.len();
    let paths = arh_paths(&arh_orig);
    if paths.is_empty() {
        return Err("bad-arh".into());
    }
    let out_dir = m.join(".ard");
    let _ = fs::remove_dir_all(&out_dir);
    let _ = fs::remove_file(root.join("bf2.arh"));
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let mut ard = fs::File::open(ad.join("bf2.ard")).map_err(|e| e.to_string())?;
    let mut arh = arh_orig.clone();
    let mut tail: Vec<u8> = Vec::new();
    for p in mod_files(&root) {
        let rel = p.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let key = rel.to_lowercase();
        let Some(idxs) = paths.get(&key) else {
            rep.loose += 1;
            continue;
        };
        let ents: Vec<Entry> = idxs.iter().map(|&i| entry(&arh_orig, i)).collect();
        let data = fs::read(&p).map_err(|e| e.to_string())?;
        let Ok(len) = u32::try_from(data.len()) else {
            rep.skipped.push(rel);
            continue;
        };
        let new_off = ard_size + tail.len() as u64;
        if ents.iter().all(|e| e.flags == 0 && e.unc == 0) {
            // stored raw: the bytes themselves go after the archive
            for e in &ents {
                repoint(&mut arh, e.idx, new_off, len, 0);
            }
            tail.extend_from_slice(&data);
            rep.appended += 1;
            continue;
        }
        let mut uncs: Vec<u32> = ents.iter().map(|e| e.unc).collect();
        uncs.dedup();
        if uncs.len() > 1 {
            rep.skipped.push(rel);
            continue;
        }
        let e = ents.iter().min_by_key(|e| e.comp).unwrap();
        let mut h = [0u8; 0x30];
        ard.seek(SeekFrom::Start(e.offset)).and_then(|_| ard.read_exact(&mut h)).map_err(|e| e.to_string())?;
        let checksum = if &h[..4] == b"xbc1" { u32_at(&h, 0x10) } else { 0 };
        let z = zlib(&data);
        if z.len() > data.len() {
            rep.skipped.push(rel);
            continue;
        }
        let zl = z.len() as u32;
        for e in &ents {
            repoint(&mut arh, e.idx, new_off, zl, len);
        }
        tail.extend_from_slice(&xbc1_header(&rel, len, zl, checksum));
        tail.extend_from_slice(&z);
        rep.appended += 1;
    }
    if !tail.is_empty() {
        fs::write(out_dir.join("append.bin"), &tail).map_err(|e| e.to_string())?;
        fs::write(root.join("bf2.arh"), &arh).map_err(|e| e.to_string())?;
    }
    let mut s = stamp(pkg, &root).unwrap_or_default();
    for k in &rep.skipped {
        s.push_str(&format!("\nskipped {k}"));
    }
    fs::write(out_dir.join(STAMP), &s).map_err(|e| e.to_string())?;
    Ok(rep)
}

/// Before a start: prepares every enabled mod whose preparation is missing or out of date (a new mod file, or the
/// game's archive index changed with an update). Failures are left to the runtime's own report.
pub fn prepare_stale(pkg: &Path) {
    for m in list(pkg) {
        if m.enabled && m.romfs && !m.prepared {
            let _ = prepare(pkg, &m.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads a file back the way the runtime would: the (possibly repointed) arh entry, with offsets past bf2.ard served from append.bin. Raw entries are the bytes; others xbc1 + zlib.
    fn read_back(pkg: &Path, m: &Path, root: &Path, key: &str) -> Vec<u8> {
        let unz = |d: &[u8]| {
            let mut o = Vec::new();
            flate2::read::ZlibDecoder::new(d).read_to_end(&mut o).unwrap();
            o
        };
        let ad = archive_dir(pkg).unwrap();
        let arh = fs::read(root.join("bf2.arh")).unwrap_or_else(|_| fs::read(ad.join("bf2.arh")).unwrap());
        let ard_size = ad.join("bf2.ard").metadata().unwrap().len();
        let e = entry(&arh, arh_paths(&arh)[key][0]);
        let tail = fs::read(m.join(".ard/append.bin")).unwrap_or_default();
        let bytes = |off: u64, n: usize| -> Vec<u8> {
            if off >= ard_size {
                let o = (off - ard_size) as usize;
                tail[o..o + n].to_vec()
            } else {
                let mut f = fs::File::open(ad.join("bf2.ard")).unwrap();
                f.seek(SeekFrom::Start(off)).unwrap();
                let mut b = vec![0; n];
                f.read_exact(&mut b).unwrap();
                b
            }
        };
        if e.flags == 0 && e.unc == 0 {
            return bytes(e.offset, e.comp as usize);
        }
        let rec = bytes(e.offset, 0x30 + e.comp as usize);
        assert_eq!(&rec[..4], b"xbc1");
        unz(&rec[0x30..])
    }

    /// On a real install: XC2_PKG=<pkg> XC2_MOD=<a mod folder>. The mod's files are copied to a scratch package,
    /// prepared, and read back through the prepared records.
    #[test]
    #[ignore]
    fn prepared_files_read_back() {
        let (Ok(real), Ok(src)) = (std::env::var("XC2_PKG"), std::env::var("XC2_MOD")) else { return };
        let (real, src) = (PathBuf::from(real), PathBuf::from(src));
        let tmp = std::env::temp_dir().join(format!("xc2l-mods-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("game")).unwrap();
        std::os::unix::fs::symlink(archive_dir(&real).unwrap(), tmp.join("game/romfs-301")).unwrap();
        fs::write(tmp.join("nxrt.ini"), "[fs]\n").unwrap();
        let (name, r) = add(&tmp, &src).unwrap();
        eprintln!("{name}: records {} appended {} loose {} skipped {:?}", r.records, r.appended, r.loose, r.skipped);
        let m = dir(&tmp).join(&name);
        let root = romfs_root(&m).unwrap();
        let paths = arh_paths(&fs::read(archive_dir(&tmp).unwrap().join("bf2.arh")).unwrap());
        let mut checked = 0;
        for p in mod_files(&root) {
            let key = p.strip_prefix(&root).unwrap().to_string_lossy().to_lowercase();
            if paths.contains_key(&key) && !r.skipped.iter().any(|s| s.to_lowercase() == key) {
                assert!(read_back(&tmp, &m, &root, &key) == fs::read(&p).unwrap(), "{key}");
                checked += 1;
            }
        }
        eprintln!("{checked} file(s) read back identical");
        let mut ard_files = Vec::new();
        walk(&m.join(".ard"), &mut ard_files);
        assert!(!ard_files.iter().any(|p| p.extension().is_some_and(|e| e == "rec")), "no .rec records");
        assert!(checked > 0);
        let l = list(&tmp);
        assert!(l[0].prepared && l[0].enabled);
        set_enabled(&tmp, &name, false).unwrap();
        assert!(!list(&tmp)[0].enabled);
        assert!(fs::read_to_string(tmp.join("nxrt.ini")).unwrap().contains(&format!("disabled = {name}")));
        set_enabled(&tmp, &name, true).unwrap();
        assert!(list(&tmp)[0].enabled);
        remove(&tmp, &name).unwrap();
        assert!(list(&tmp).is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }
}
