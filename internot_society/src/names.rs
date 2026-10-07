//! The pack's name data ([`NameData`]): SSA first names by year and sex,
//! Census 2020 first names and surnames by heritage group, and foreign
//! names, decoded once per process per data file. Format: the docstring of
//! `internot_society/data/distill_names.py`.
//!
//! People are named on the monotone world in `mono::naming`.

use std::sync::{Arc, Mutex, OnceLock};

use crate::params::Sex;

// --- the data ----------------------------------------------------------------------

/// A pack's name tables, decoded from its data file (format: the docstring of
/// `internot_society/data/distill_names.py`).
pub struct NameData {
    /// Group columns, e.g. `["white", "black", "aian", "asian", "multi",
    /// "hispanic"]`.
    pub columns: Vec<String>,
    /// Display forms of first names; indices are name ids.
    pub first: Vec<Box<str>>,
    /// Per first name: its row in `census` (counts by column, then by sex
    /// `[male, female]`), if it is in the census lists.
    census_row: Vec<u32>,
    /// `columns.len() + 2` counts per census first name.
    census: Vec<u32>,
    /// Whether a first name was ever given to 5+ US babies in a year (SSA).
    pub in_ssa: Vec<bool>,
    /// First and last SSA years.
    pub first_year: i32,
    pub last_year: i32,
    /// SSA births per year and sex (`[(year − first_year) * 2 + sex]`): `(name,
    /// cumulative count through this name)` by name, so a draw is a search.
    ssa: Vec<Vec<(u32, u32)>>,
    /// Display forms of surnames.
    pub surnames: Vec<Box<str>>,
    /// `columns.len()` counts per surname.
    pub surname_counts: Vec<u32>,
    /// The census "all other names" tail per column.
    pub surname_tail: Vec<u64>,
}

impl std::fmt::Debug for NameData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NameData({} first names, SSA {}–{}, {} surnames)",
            self.first.len(),
            self.first_year,
            self.last_year,
            self.surnames.len()
        )
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let x = *self.b.get(self.p).ok_or("truncated name data")?;
        self.p += 1;
        Ok(x)
    }

    fn v(&mut self) -> Result<u64, String> {
        let (mut x, mut s) = (0u64, 0);
        loop {
            let c = self.byte()?;
            x |= ((c & 0x7f) as u64) << s;
            if c < 0x80 {
                return Ok(x);
            }
            s += 7;
            if s > 63 {
                return Err("bad varint in name data".into());
            }
        }
    }

    fn z(&mut self) -> Result<i64, String> {
        let u = self.v()?;
        Ok((u >> 1) as i64 ^ -((u & 1) as i64))
    }

    fn name(&mut self) -> Result<Box<str>, String> {
        let n = self.byte()? as usize;
        let s = self
            .b
            .get(self.p..self.p + n)
            .ok_or("truncated name data")?;
        self.p += n;
        Ok(std::str::from_utf8(s)
            .map_err(|_| "a name is not UTF-8")?
            .into())
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(self.byte()? as u16 | (self.byte()? as u16) << 8)
    }
}

impl NameData {
    /// No names: a placeholder before a pack's data is read.
    pub(crate) fn empty() -> Self {
        NameData {
            columns: Vec::new(),
            first: Vec::new(),
            census_row: Vec::new(),
            census: Vec::new(),
            in_ssa: Vec::new(),
            first_year: 0,
            last_year: 0,
            ssa: Vec::new(),
            surnames: Vec::new(),
            surname_counts: Vec::new(),
            surname_tail: Vec::new(),
        }
    }

    /// Decode `bytes`, or reuse the copy already decoded in this process.
    pub fn cached(bytes: &[u8]) -> Result<Arc<NameData>, String> {
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        let hash = content_hash(bytes);
        let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
        if let Some((_, d)) = cache.lock().unwrap().iter().find(|(h, _)| *h == hash) {
            return Ok(d.clone());
        }
        let d = Arc::new(Self::decode(bytes)?);
        cache.lock().unwrap().push((hash, d.clone()));
        Ok(d)
    }

    fn decode(bytes: &[u8]) -> Result<NameData, String> {
        if bytes.len() < 9 || &bytes[..8] != b"INTNAMES" || bytes[8] != 2 {
            return Err("not a version-2 name data file".into());
        }
        let mut r = Reader { b: bytes, p: 9 };
        let ncol = r.v()? as usize;
        let columns = (0..ncol)
            .map(|_| r.name().map(String::from))
            .collect::<Result<Vec<_>, _>>()?;
        let n = r.v()? as usize;
        let first = (0..n).map(|_| r.name()).collect::<Result<Vec<_>, _>>()?;
        let mut census_row = vec![u32::MAX; n];
        let mut census = Vec::new();
        for row in census_row.iter_mut() {
            if r.byte()? & 1 == 1 {
                *row = (census.len() / (ncol + 2)) as u32;
                for _ in 0..ncol + 2 {
                    census.push(r.v()? as u32);
                }
            }
        }
        for _ in 0..ncol {
            r.v()?;
        }
        let first_year = r.u16()? as i32;
        let last_year = r.u16()? as i32;
        let years = (last_year - first_year + 1) as usize;
        let mut ssa: Vec<Vec<(u32, u32)>> = vec![Vec::new(); years * 2];
        let mut in_ssa = vec![false; n];
        let series = r.v()?;
        let mut name = 0u64;
        for _ in 0..series {
            name += r.v()?;
            let sex = r.byte()? as usize;
            let start = r.v()? as usize;
            let len = r.v()? as usize;
            let mut count = 0i64;
            for k in 0..len {
                count += r.z()?;
                if count > 0 {
                    ssa[(start + k) * 2 + sex].push((name as u32, count as u32));
                }
            }
            in_ssa[name as usize] = true;
        }
        for rows in &mut ssa {
            let mut acc = 0u32;
            for r in rows.iter_mut() {
                acc = acc.checked_add(r.1).ok_or("a year's SSA births overflow 32 bits")?;
                r.1 = acc;
            }
        }
        let ns = r.v()? as usize;
        let mut surnames = Vec::with_capacity(ns);
        let mut surname_counts = Vec::with_capacity(ns * ncol);
        for _ in 0..ns {
            surnames.push(r.name()?);
            for _ in 0..ncol {
                surname_counts.push(r.v()? as u32);
            }
        }
        let surname_tail = (0..ncol).map(|_| r.v()).collect::<Result<Vec<_>, _>>()?;
        Ok(NameData {
            columns,
            first,
            census_row,
            census,
            in_ssa,
            first_year,
            last_year,
            ssa,
            surnames,
            surname_counts,
            surname_tail,
        })
    }

    /// The census counts of first name `i` (by column, then `[male,
    /// female]`), if listed.
    pub fn census_of(&self, i: u32) -> Option<&[u32]> {
        let row = self.census_row[i as usize];
        let w = self.columns.len() + 2;
        (row != u32::MAX).then(|| &self.census[row as usize * w..(row as usize + 1) * w])
    }

    /// SSA births of `year` (clamped to the data) and sex: `(name,
    /// cumulative count)`; a name's count is the step from the row before.
    pub fn ssa_year(&self, year: i32, sex: Sex) -> &[(u32, u32)] {
        let y = year.clamp(self.first_year, self.last_year) - self.first_year;
        &self.ssa[y as usize * 2 + sex as usize]
    }
}

/// Decoded name data by content hash.
type Cache = Vec<(u64, Arc<NameData>)>;

/// A content hash of the data file, for the process cache (a multiply-rotate
/// over 8-byte words).
fn content_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ bytes.len() as u64;
    for c in bytes.chunks(8) {
        let mut w = [0u8; 8];
        w[..c.len()].copy_from_slice(c);
        h ^= u64::from_le_bytes(w);
        h = h.wrapping_mul(0x0000_0100_0000_01b3).rotate_left(23);
    }
    h
}

// --- sampling tables ---------------------------------------------------------------

