//! A small editor for the base game's save (`bf2savefile.sav`, the 2.x/3.0 layout of XC2 that the Switch 2 Edition keeps):
//! money, the three core crystals you can stack (common, rare, legendary) and Overdrive Protocols. Offsets and the item record come from the
//! community editor XC2SaveNETThingy (its 3.0 fork); the file is plain data, with no checksum.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const FILE: &str = "bf2savefile.sav";
const SIZE: usize = 0x126D90;
const MAGIC: [u8; 4] = [0x5B, 0xA1, 0x03, 0x00];
const MONEY: usize = 0x10;
const ITEM_BOX: usize = 0xE98E8;
const SERIAL_MASK: u32 = (1 << 26) - 1;
pub const MAX_ITEMS: u32 = 99;
pub const MAX_MONEY: u32 = 99_999_999;

/// One pouch of the item box: where its 12-byte records start, how many, the item type the records carry, and which of the
/// box's "next serial" counters is its own.
struct Pouch {
    at: usize,
    slots: usize,
    kind: u32,
    counter: usize,
}

/// core crystals (ids stored minus 45000)
const CRYSTAL_POUCH: Pouch = Pouch { at: ITEM_BOX + 0xC990, slots: 200, kind: 11, counter: 11 };
/// key items (ids stored minus 25000)
const KEY_POUCH: Pouch = Pouch { at: ITEM_BOX + 0x5460, slots: 500, kind: 6, counter: 4 };
const CRYSTALS: usize = ITEM_BOX + 0xC990;
const CRYSTAL_SERIAL: usize = ITEM_BOX + 0x122A0 + 11 * 4;
pub const COMMON: u32 = 11;
pub const RARE: u32 = 12;
pub const LEGENDARY: u32 = 13;
/// Overdrive Protocol (25408)
pub const OVERDRIVE: u32 = 408;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Values {
    pub money: u32,
    pub common: u32,
    pub rare: u32,
    pub legendary: u32,
    #[serde(default)]
    pub overdrive: u32,
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

#[cfg(test)]
fn slot(i: usize) -> usize {
    CRYSTALS + i * 12
}

fn id(word: u32) -> u32 {
    word & 0x1FFF
}

fn qty(word: u32) -> u32 {
    (word >> 19) & 0x3FF
}

impl Pouch {
    fn rec(&self, i: usize) -> usize {
        self.at + i * 12
    }
    fn serial_counter(&self) -> usize {
        ITEM_BOX + 0x122A0 + self.counter * 4
    }
    fn count(&self, d: &[u8], want: u32) -> u32 {
        (0..self.slots).map(|i| u32_at(d, self.rec(i))).filter(|&w| w != 0 && id(w) == want).map(qty).sum()
    }
    /// Sets one item's amount: the first record of it holds the amount (others of the same item are emptied); a new record
    /// takes a free slot and a fresh serial, as the game does.
    fn set(&self, d: &mut [u8], want: u32, n: u32) -> Result<(), String> {
        let n = n.min(MAX_ITEMS);
        let mut first = None;
        for i in 0..self.slots {
            let o = self.rec(i);
            let w = u32_at(d, o);
            if w != 0 && id(w) == want {
                if first.is_none() {
                    first = Some(o);
                } else {
                    d[o..o + 12].fill(0);
                }
            }
        }
        match (first, n) {
            (Some(o), 0) => d[o..o + 12].fill(0),
            (Some(o), _) => {
                let w = u32_at(d, o);
                put(d, o, (w & !(0x3FF << 19)) | (n << 19));
            }
            (None, 0) => {}
            (None, _) => {
                let recs: Vec<usize> = (0..self.slots).map(|i| self.rec(i)).collect();
                let free = *recs.iter().find(|&&o| u32_at(d, o) == 0).ok_or("save-full")?;
                // the time it was obtained: the same as another item's of the pouch (or none)
                let time = recs.iter().find(|&&o| u32_at(d, o) != 0).map(|&o| u32_at(d, o + 4)).unwrap_or(0);
                let used = recs.iter().filter(|&&o| u32_at(d, o) != 0).map(|&o| u32_at(d, o + 8) & SERIAL_MASK).max();
                let serial = u32_at(d, self.serial_counter()).max(used.map_or(1, |u| u + 1)) & SERIAL_MASK;
                put(d, free, want | (self.kind << 13) | (n << 19));
                put(d, free + 4, time);
                put(d, free + 8, serial);
                put(d, self.serial_counter(), serial + 1);
            }
        }
        Ok(())
    }
}

pub fn read_bytes(d: &[u8]) -> Result<Values, String> {
    check(d)?;
    let c = &CRYSTAL_POUCH;
    Ok(Values {
        money: u32_at(d, MONEY),
        common: c.count(d, COMMON),
        rare: c.count(d, RARE),
        legendary: c.count(d, LEGENDARY),
        overdrive: KEY_POUCH.count(d, OVERDRIVE),
    })
}

pub fn write_bytes(d: &mut [u8], v: &Values) -> Result<(), String> {
    check(d)?;
    put(d, MONEY, v.money.min(MAX_MONEY));
    CRYSTAL_POUCH.set(d, COMMON, v.common)?;
    CRYSTAL_POUCH.set(d, RARE, v.rare)?;
    CRYSTAL_POUCH.set(d, LEGENDARY, v.legendary)?;
    KEY_POUCH.set(d, OVERDRIVE, v.overdrive)?;
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
        assert_eq!(read_bytes(&d).unwrap(), Values { money: 38880, common: 25, rare: 0, legendary: 0, overdrive: 0 });
        write_bytes(&mut d, &Values { money: 1_000_000, common: 3, rare: 0, legendary: 10, overdrive: 12 }).unwrap();
        assert_eq!(read_bytes(&d).unwrap(), Values { money: 1_000_000, common: 3, rare: 0, legendary: 10, overdrive: 12 });
        // the new crystal took the free slot 1 with the next serial, and the counter moved on
        let w = u32_at(&d, slot(1));
        assert_eq!((id(w), (w >> 13) & 63, qty(w)), (LEGENDARY, 11, 10));
        assert_eq!(u32_at(&d, slot(1) + 8), 7);
        assert_eq!(u32_at(&d, slot(1) + 4), 0x9d8f);
        assert_eq!(u32_at(&d, CRYSTAL_SERIAL), 8);
        // the other crystal is untouched; removing one empties its record
        assert_eq!(qty(u32_at(&d, slot(2))), 1);
        write_bytes(&mut d, &Values { money: u32::MAX, common: 0, rare: 0, legendary: 500, overdrive: 0 }).unwrap();
        assert_eq!(u32_at(&d, slot(0)), 0);
        assert_eq!(read_bytes(&d).unwrap(), Values { money: MAX_MONEY, common: 0, rare: 0, legendary: 99, overdrive: 0 });
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
        let want = super::Values { money: before.money + 1000, common: before.common, rare: 5, legendary: 20, overdrive: before.overdrive + 2 };
        let after = super::write(&dir, &want).unwrap();
        println!("después {after:?}");
        assert_eq!(after, want);
    }
}
