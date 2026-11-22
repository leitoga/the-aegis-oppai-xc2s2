//! A small editor for the base game's save (`bf2savefile.sav`, the 2.x/3.0 layout of XC2 that the Switch 2 Edition keeps):
//! money and the three core crystals you can stack (common, rare, legendary). Offsets and the item record come from the
//! community editor XC2SaveNETThingy (its 3.0 fork); the file is plain data, with no checksum.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const FILE: &str = "bf2savefile.sav";
const SIZE: usize = 0x126D90;
const MAGIC: [u8; 4] = [0x5B, 0xA1, 0x03, 0x00];
const MONEY: usize = 0x10;
const ITEM_BOX: usize = 0xE98E8;
const CRYSTALS: usize = ITEM_BOX + 0xC990; // 200 records of 12 bytes
const CRYSTAL_SLOTS: usize = 200;
const CRYSTAL_TYPE: u32 = 11;
/// the item box keeps one "next serial" counter per pouch; the crystals' is the 12th
const CRYSTAL_SERIAL: usize = ITEM_BOX + 0x122A0 + 11 * 4;
const SERIAL_MASK: u32 = (1 << 26) - 1;
/// item ids are stored minus 45000 for crystals
pub const COMMON: u32 = 11;
pub const RARE: u32 = 12;
pub const LEGENDARY: u32 = 13;
pub const MAX_CRYSTALS: u32 = 99;
pub const MAX_MONEY: u32 = 99_999_999;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Values {
    pub money: u32,
    pub common: u32,
    pub rare: u32,
    pub legendary: u32,
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn put(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn check(d: &[u8]) -> Result<(), String> {
    if d.len() != SIZE || d[..4] != MAGIC {
        return Err("save-format".into());
    }
    Ok(())
}

fn slot(i: usize) -> usize {
    CRYSTALS + i * 12
}

fn id(word: u32) -> u32 {
    word & 0x1FFF
}

fn qty(word: u32) -> u32 {
    (word >> 19) & 0x3FF
}

fn count(d: &[u8], want: u32) -> u32 {
    (0..CRYSTAL_SLOTS).map(|i| u32_at(d, slot(i))).filter(|&w| w != 0 && id(w) == want).map(qty).sum()
}

pub fn read_bytes(d: &[u8]) -> Result<Values, String> {
    check(d)?;
    Ok(Values { money: u32_at(d, MONEY), common: count(d, COMMON), rare: count(d, RARE), legendary: count(d, LEGENDARY) })
}

/// Sets one crystal's amount: the first record of it holds the amount (others of the same crystal are emptied); a new record
/// takes a free slot and a fresh serial, as the game does.
fn set_crystal(d: &mut [u8], want: u32, n: u32) -> Result<(), String> {
    let n = n.min(MAX_CRYSTALS);
    let mut first = None;
    for i in 0..CRYSTAL_SLOTS {
        let w = u32_at(d, slot(i));
        if w != 0 && id(w) == want {
            if first.is_none() {
                first = Some(i);
            } else {
                d[slot(i)..slot(i) + 12].fill(0);
            }
        }
    }
    match (first, n) {
        (Some(i), 0) => d[slot(i)..slot(i) + 12].fill(0),
        (Some(i), _) => {
            let w = u32_at(d, slot(i));
            put(d, slot(i), (w & !(0x3FF << 19)) | (n << 19));
        }
        (None, 0) => {}
        (None, _) => {
            let free = (0..CRYSTAL_SLOTS).find(|&i| u32_at(d, slot(i)) == 0).ok_or("save-full")?;
            // the time it was obtained: the same as another crystal's (or none)
            let time = (0..CRYSTAL_SLOTS).map(|i| slot(i)).find(|&o| u32_at(d, o) != 0).map(|o| u32_at(d, o + 4)).unwrap_or(0);
            let used = (0..CRYSTAL_SLOTS).map(|i| slot(i)).filter(|&o| u32_at(d, o) != 0).map(|o| u32_at(d, o + 8) & SERIAL_MASK).max();
            let serial = u32_at(d, CRYSTAL_SERIAL).max(used.map_or(1, |u| u + 1)) & SERIAL_MASK;
            put(d, slot(free), want | (CRYSTAL_TYPE << 13) | (n << 19));
            put(d, slot(free) + 4, time);
            put(d, slot(free) + 8, serial);
            put(d, CRYSTAL_SERIAL, serial + 1);
        }
    }
    Ok(())
}

pub fn write_bytes(d: &mut [u8], v: &Values) -> Result<(), String> {
    check(d)?;
    put(d, MONEY, v.money.min(MAX_MONEY));
    set_crystal(d, COMMON, v.common)?;
    set_crystal(d, RARE, v.rare)?;
    set_crystal(d, LEGENDARY, v.legendary)?;
    Ok(())
}

pub fn read(dir: &Path) -> Result<Values, String> {
    let d = fs::read(dir.join(FILE)).map_err(|_| "no-save".to_string())?;
    read_bytes(&d)
}

/// Writes the new values (the caller makes the backup first). Written to a temporary file and renamed over the save.
pub fn write(dir: &Path, v: &Values) -> Result<Values, String> {
    let p = dir.join(FILE);
    let mut d = fs::read(&p).map_err(|_| "no-save".to_string())?;
    write_bytes(&mut d, v)?;
    let tmp = dir.join(format!("{FILE}.nuevo"));
    fs::write(&tmp, &d).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    read_bytes(&d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Vec<u8> {
        let mut d = vec![0u8; SIZE];
        d[..4].copy_from_slice(&MAGIC);
        put(&mut d, MONEY, 38880);
        // 25 common crystals (serial 2) and a single-copy one (id 16, serial 6), as in a real save
        put(&mut d, slot(0), COMMON | (11 << 13) | (25 << 19));
        put(&mut d, slot(0) + 4, 0x9d8f);
        put(&mut d, slot(0) + 8, 2);
        put(&mut d, slot(2), 16 | (11 << 13) | (1 << 19));
        put(&mut d, slot(2) + 8, 6);
        put(&mut d, CRYSTAL_SERIAL, 7);
        d
    }

    #[test]
    fn reads_and_edits() {
        let mut d = blank();
        assert_eq!(read_bytes(&d).unwrap(), Values { money: 38880, common: 25, rare: 0, legendary: 0 });
        write_bytes(&mut d, &Values { money: 1_000_000, common: 3, rare: 0, legendary: 10 }).unwrap();
        assert_eq!(read_bytes(&d).unwrap(), Values { money: 1_000_000, common: 3, rare: 0, legendary: 10 });
        // the new crystal took the free slot 1 with the next serial, and the counter moved on
        let w = u32_at(&d, slot(1));
        assert_eq!((id(w), (w >> 13) & 63, qty(w)), (LEGENDARY, 11, 10));
        assert_eq!(u32_at(&d, slot(1) + 8), 7);
        assert_eq!(u32_at(&d, slot(1) + 4), 0x9d8f);
        assert_eq!(u32_at(&d, CRYSTAL_SERIAL), 8);
        // the other crystal is untouched; removing one empties its record
        assert_eq!(qty(u32_at(&d, slot(2))), 1);
        write_bytes(&mut d, &Values { money: u32::MAX, common: 0, rare: 0, legendary: 500 }).unwrap();
        assert_eq!(u32_at(&d, slot(0)), 0);
        assert_eq!(read_bytes(&d).unwrap(), Values { money: MAX_MONEY, common: 0, rare: 0, legendary: 99 });
    }

    #[test]
    fn refuses_other_files() {
        assert!(read_bytes(&[0u8; 100]).is_err());
        let mut d = blank();
        d[0] = 0;
        assert!(read_bytes(&d).is_err());
    }
}

#[cfg(test)]
mod real_save {
    /// `XC2_SAVE_DIR=<a copy of an account folder> cargo test real_save -- --ignored`: edits that copy and reads it back
    #[test]
    #[ignore]
    fn edits_a_copy() {
        let dir = std::path::PathBuf::from(std::env::var("XC2_SAVE_DIR").unwrap());
        let before = super::read(&dir).unwrap();
        println!("antes {before:?}");
        let want = super::Values { money: before.money + 1000, common: before.common, rare: 5, legendary: 20 };
        let after = super::write(&dir, &want).unwrap();
        println!("después {after:?}");
        assert_eq!(after, want);
    }
}
