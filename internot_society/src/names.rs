//! Names (N1): first and middle names, surnames, and marriages, as pure
//! functions of `(seed, id, t)` over the world's kin lookups. Plan:
//! `docs/superpowers/plans/2026-10-01-n1-heritage-and-names.md`; practice
//! and sources: the pack's `names.ron`.
//!
//! - **First names.** A person born in the world (or a founder) is named
//!   from the year's SSA names, split across heritage groups by
//!   `P(group | name)` from Census 2020, raked (IPF) to the world's own births
//!   by group that year: the world's names have SSA's frequencies overall
//!   and each group's leanings. Immigrants were named abroad: a share get one
//!   of the data's foreign names, the rest a name of their birth year. A
//!   child of parents in different groups is named in one parent's
//!   tradition.
//! - **Surnames.** Founders and immigrants draw one from their group. A
//!   child takes the father's, the mother's or both, by whether the parents
//!   were married at the birth; Hispanic families sometimes give two. A
//!   partner may take the other's surname at the wedding and may revert after
//!   a separation, so a surname is a function of time.
//! - **Marriages.** Unions are marriages and cohabitations together; each
//!   union marries at its start, later, or never, by era
//!   ([`World::marriage_date`]).
//!
//! The name tables load once per process per data file ([`NameData`]); the
//! sampling tables are built lazily per world, since their margins are the
//! world's own births.

use std::sync::{Arc, Mutex, OnceLock};

use procedural_core::dmath::ln;
use procedural_core::key::{label, Key};
use procedural_core::stream::{year_of, year_start};

use crate::params::{Heritage, Sex};
use crate::world::{PersonId, Union, World};

const TAG_FIRST: u64 = label("names/first");
const TAG_FOREIGN: u64 = label("names/foreign");
const TAG_TRADITION: u64 = label("names/tradition");
const TAG_MIDDLE: u64 = label("names/middle");
const TAG_SURNAME: u64 = label("names/surname");
const TAG_CHILD: u64 = label("names/child-surname");
const TAG_DOUBLE: u64 = label("names/double");
const TAG_MARRIAGE: u64 = label("names/marriage");
const TAG_WEDDING: u64 = label("names/wedding");
const TAG_REVERT: u64 = label("names/revert");

const YEAR: f64 = 365.2425 * 86_400.0;

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
    in_ssa: Vec<bool>,
    /// First and last SSA years.
    pub first_year: i32,
    pub last_year: i32,
    /// SSA births per year and sex (`[(year − first_year) * 2 + sex]`): `(name,
    /// count)` by name.
    ssa: Vec<Vec<(u32, u32)>>,
    /// Display forms of surnames.
    pub surnames: Vec<Box<str>>,
    /// `columns.len()` counts per surname.
    surname_counts: Vec<u32>,
    /// The census "all other names" tail per column.
    surname_tail: Vec<u64>,
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
    fn census_of(&self, i: u32) -> Option<&[u32]> {
        let row = self.census_row[i as usize];
        let w = self.columns.len() + 2;
        (row != u32::MAX).then(|| &self.census[row as usize * w..(row as usize + 1) * w])
    }

    /// SSA births of `year` (clamped to the data) and sex.
    fn ssa_year(&self, year: i32, sex: Sex) -> &[(u32, u32)] {
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

/// Cumulative weights in fixed point (`u32`, the last entry `u32::MAX`) over
/// a list of ids; a draw is a binary search.
#[derive(Debug)]
struct Table {
    ids: Vec<u32>,
    cum: Vec<u32>,
}

impl Table {
    fn new(items: impl Iterator<Item = (u32, f64)>) -> Option<Table> {
        let items: Vec<(u32, f64)> = items.filter(|x| x.1 > 0.0).collect();
        let total: f64 = items.iter().map(|x| x.1).sum();
        if items.is_empty() || total.is_nan() || total <= 0.0 {
            return None;
        }
        let mut acc = 0.0;
        let mut cum = Vec::with_capacity(items.len());
        for &(_, w) in &items {
            acc += w;
            cum.push(((acc / total) * u32::MAX as f64).round() as u32);
        }
        *cum.last_mut().unwrap() = u32::MAX;
        Some(Table {
            ids: items.iter().map(|x| x.0).collect(),
            cum,
        })
    }

    fn draw(&self, key: Key) -> u32 {
        let u = (key.bits() >> 32) as u32;
        let i = self.cum.partition_point(|&c| c < u).min(self.ids.len() - 1);
        self.ids[i]
    }
}

/// One year and sex of first names: a table per heritage group.
type YearTables = Vec<Option<Table>>;

/// A world's lazily built sampling tables.
#[derive(Default)]
pub(crate) struct NameTables {
    /// `[(year − first_year) * 2 + sex]`.
    first: Vec<OnceLock<YearTables>>,
    /// Foreign names per `sex * groups + group`.
    foreign: OnceLock<Vec<Option<Table>>>,
    /// Surnames per group.
    surnames: OnceLock<Vec<Option<Table>>>,
}

impl std::fmt::Debug for NameTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let built = self.first.iter().filter(|t| t.get().is_some()).count();
        write!(
            f,
            "NameTables({built} of {} year tables built)",
            self.first.len()
        )
    }
}

impl NameTables {
    pub(crate) fn new(data: &NameData) -> Self {
        let years = (data.last_year - data.first_year + 1).max(0) as usize;
        NameTables {
            first: (0..years * 2).map(|_| OnceLock::new()).collect(),
            foreign: OnceLock::new(),
            surnames: OnceLock::new(),
        }
    }
}

/// A surname: one or two listed surnames, joined by a space or a hyphen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Surname {
    /// The line surname: what a child takes from this parent.
    pub first: u32,
    pub second: Option<u32>,
    pub hyphen: bool,
}

impl Surname {
    fn one(first: u32) -> Self {
        Surname {
            first,
            second: None,
            hyphen: false,
        }
    }
}

/// A middle name: a given name, or a family surname.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Middle {
    Given(u32),
    Family(u32),
}

/// What a wedding does to the partners' surnames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Neither,
    /// `person` takes the other's surname.
    Takes(PersonId),
    /// `person` hyphenates their surname with the other's.
    Hyphenates(PersonId),
}

impl World {
    fn name_data(&self) -> &NameData {
        &self.ledger().params.name_data
    }

    // --- marriages ----------------------------------------------------------------------

    /// The date union `u` (seen from partner `x`) becomes a marriage, or
    /// `None` if it stays a cohabitation. A pure function of the union, the
    /// same from both partners' sides: some unions begin as marriages, some
    /// cohabitations marry later if the couple is still together, by era;
    /// same-sex couples can marry once it is legal for them (`unions.ron`).
    pub fn marriage_date(&self, x: PersonId, u: &Union) -> Option<i64> {
        let m = &self.ledger().params.unions.marriage;
        let k = u.key.with(TAG_MARRIAGE);
        let year = year_of(u.start);
        let delay = |k: Key| (-ln(1.0 - k.unit()) * m.delay_years * YEAR) as i64;
        let date = if self.sex(x) == self.sex(u.partner) {
            // The couple's legal date: the first year the legal share passes
            // their keyed draw.
            let v = k.with(1).unit();
            let (lo, hi) = (m.same_sex_legal.0[0].0, m.same_sex_legal.0.last()?.0);
            let legal = (lo..=hi).find(|&y| m.same_sex_legal.at(y) > v)?;
            let earliest = u.start.max(year_start(legal));
            if earliest == u.start && k.unit() < m.at_start.at(year) {
                earliest
            } else if k.with(2).unit() < m.later.at(year) {
                earliest + delay(k.with(3))
            } else {
                return None;
            }
        } else if k.unit() < m.at_start.at(year) {
            u.start
        } else if k.with(2).unit() < m.later.at(year) {
            u.start + delay(k.with(3))
        } else {
            return None;
        };
        (date < u.end).then_some(date)
    }

    /// Whether `x` is married at `t`.
    pub fn married_at(&self, x: PersonId, t: i64) -> bool {
        self.unions(x)
            .into_iter()
            .flatten()
            .any(|u| u.start <= t && t < u.end && self.marriage_date(x, &u).is_some_and(|m| m <= t))
    }

    // --- first and middle names ------------------------------------------------------

    /// The heritage group whose naming tradition `x` was named in: their own,
    /// or for a child of parents in different groups, one parent's.
    pub fn naming_group(&self, x: PersonId) -> Heritage {
        let own = self.heritage(x);
        let Some(f) = self.father(x) else {
            return own;
        };
        let fh = self.heritage(f);
        if fh == own {
            return own;
        }
        let mother_share = self.ledger().params.names.first_names.mixed_mother;
        if self.key().with2(TAG_TRADITION, x as u64).unit() < mother_share {
            own
        } else {
            fh
        }
    }

    /// `x`'s first name id (see [`Self::first_name`]).
    pub fn first_name_id(&self, x: PersonId) -> u32 {
        self.given_name(x, self.key().with2(TAG_FIRST, x as u64), None)
    }

    /// `x`'s first name.
    pub fn first_name(&self, x: PersonId) -> &str {
        &self.name_data().first[self.first_name_id(x) as usize]
    }

    /// A given name for `x` from their tradition, year and sex, drawn with
    /// `key`; `avoid` is redrawn (a few times) if drawn.
    fn given_name(&self, x: PersonId, key: Key, avoid: Option<u32>) -> u32 {
        let p = &self.ledger().params;
        let h = self.naming_group(x);
        let sex = self.sex(x);
        let foreign = self.is_immigrant(x)
            && key.with(TAG_FOREIGN).unit()
                < p.names
                    .first_names
                    .foreign
                    .iter()
                    .find(|f| p.heritage.find(&f.group) == Some(h))
                    .map_or(0.0, |f| f.share);
        for attempt in 0..4u64 {
            let k = key.with(attempt);
            let id = if foreign {
                self.foreign_table(sex, h).map(|t| t.draw(k))
            } else {
                None
            }
            .or_else(|| {
                self.first_table(self.birth_year(x), sex, h)
                    .map(|t| t.draw(k))
            })
            .unwrap_or(0);
            if Some(id) != avoid {
                return id;
            }
        }
        avoid.unwrap_or(0)
    }

    /// `x`'s middle name, if any.
    pub fn middle_name_of(&self, x: PersonId) -> Option<Middle> {
        let p = &self.ledger().params;
        let mn = &p.names.middle_names;
        let h = self.naming_group(x);
        let female = self.sex(x).is_female();
        let year = self.birth_year(x);
        let share = mn
            .by_group
            .iter()
            .find(|g| p.heritage.find(&g.group) == Some(h))
            .map_or(&mn.default, |g| &g.share)
            .get(female)
            .at(year);
        let k = self.key().with2(TAG_MIDDLE, x as u64);
        if k.unit() >= share {
            return None;
        }
        if k.with(1).unit() < mn.family_surname.get(female).at(year) {
            if let Some(m) = self.mother(x) {
                return Some(Middle::Family(self.birth_surname(m).first));
            }
        }
        Some(Middle::Given(self.given_name(
            x,
            k.with(2),
            Some(self.first_name_id(x)),
        )))
    }

    /// `x`'s middle name as text.
    pub fn middle_name(&self, x: PersonId) -> Option<&str> {
        let d = self.name_data();
        self.middle_name_of(x).map(|m| match m {
            Middle::Given(i) => &*d.first[i as usize],
            Middle::Family(i) => &*d.surnames[i as usize],
        })
    }

    /// The first-name table of `year` (clamped to the data), sex and group.
    fn first_table(&self, year: i32, sex: Sex, h: Heritage) -> Option<&Table> {
        let d = self.name_data();
        let y = year.clamp(d.first_year, d.last_year);
        let slot = &self.name_tables().first[(y - d.first_year) as usize * 2 + sex as usize];
        slot.get_or_init(|| self.build_first_tables(y, sex))[h.index()].as_ref()
    }

    /// A year and sex's tables: SSA counts split across groups by `P(group |
    /// name)`, raked (IPF) so each group's total is its share of the world's
    /// births that year.
    fn build_first_tables(&self, year: i32, sex: Sex) -> YearTables {
        let p = &self.ledger().params;
        let d = self.name_data();
        let groups = p.heritage_count();
        let cols: Vec<usize> = (0..groups)
            .map(|h| p.names.column(&p.heritage, d, Heritage(h as u8)))
            .collect();
        // The world's births by group in `year` (natives; the nearest year
        // with births if none).
        let mut births = vec![0.0; groups];
        for b in self.ledger().blocks_of_year(year) {
            let block = &self.ledger().blocks[b as usize];
            births[block.heritage.index()] += block.cohorts[0].size as f64;
        }
        let total_births: f64 = births.iter().sum();
        let pi: Vec<f64> = if total_births > 0.0 {
            births.iter().map(|b| b / total_births).collect()
        } else {
            vec![1.0 / groups as f64; groups]
        };
        let rows = d.ssa_year(year, sex);
        // P(group | name) over the world's groups; unknown names take the
        // margin.
        let q: Vec<f64> = rows
            .iter()
            .flat_map(|&(name, _)| {
                let c = d.census_of(name);
                let counts: Vec<f64> = cols
                    .iter()
                    .map(|&col| c.map_or(0.0, |c| c[col] as f64))
                    .collect();
                let s: f64 = counts.iter().sum();
                (0..groups).map(move |h| if s > 0.0 { counts[h] / s } else { f64::NAN })
            })
            .collect();
        let q = |i: usize, h: usize| {
            let v = q[i * groups + h];
            if v.is_nan() {
                pi[h]
            } else {
                v
            }
        };
        let total: f64 = rows.iter().map(|r| r.1 as f64).sum();
        // Column factors `b` with w_ih = c_i q_ih b_h / Σ_h' q_ih' b_h'.
        let mut b = vec![1.0; groups];
        for _ in 0..60 {
            let mut col = vec![0.0; groups];
            for (i, &(_, c)) in rows.iter().enumerate() {
                let z: f64 = (0..groups).map(|h| q(i, h) * b[h]).sum();
                if z > 0.0 {
                    for (h, col) in col.iter_mut().enumerate() {
                        *col += c as f64 * q(i, h) * b[h] / z;
                    }
                }
            }
            for h in 0..groups {
                if col[h] > 0.0 && pi[h] > 0.0 {
                    b[h] *= pi[h] * total / col[h];
                }
            }
        }
        (0..groups)
            .map(|h| {
                Table::new(rows.iter().enumerate().map(|(i, &(name, c))| {
                    let z: f64 = (0..groups).map(|g| q(i, g) * b[g]).sum();
                    (
                        name,
                        if z > 0.0 {
                            c as f64 * q(i, h) * b[h] / z
                        } else {
                            0.0
                        },
                    )
                }))
            })
            .collect()
    }

    /// Foreign first names (in the census, never 5+ US births a year) of a
    /// sex and group, weighted by the group's count and the name's sex share.
    fn foreign_table(&self, sex: Sex, h: Heritage) -> Option<&Table> {
        let p = &self.ledger().params;
        let groups = p.heritage_count();
        let tables = self.name_tables().foreign.get_or_init(|| {
            let d = self.name_data();
            let ncol = d.columns.len();
            [Sex::Female, Sex::Male]
                .into_iter()
                .flat_map(|sex| (0..groups).map(move |h| (sex, h)))
                .map(|(sex, h)| {
                    let col = p.names.column(&p.heritage, d, Heritage(h as u8));
                    Table::new((0..d.first.len() as u32).filter_map(|i| {
                        if d.in_ssa[i as usize] {
                            return None;
                        }
                        let c = d.census_of(i)?;
                        let (male, female) = (c[ncol] as f64, c[ncol + 1] as f64);
                        let sex_share = match sex {
                            Sex::Female => female / (male + female).max(1.0),
                            Sex::Male => male / (male + female).max(1.0),
                        };
                        Some((i, c[col] as f64 * sex_share))
                    }))
                })
                .collect()
        });
        tables[sex as usize * groups + h.index()].as_ref()
    }

    // --- surnames -------------------------------------------------------------------------

    /// A surname drawn from group `h` (founders and immigrants): census counts
    /// in the group's column, the unlisted tail spread over rare names.
    fn surname_table(&self, h: Heritage) -> Option<&Table> {
        let p = &self.ledger().params;
        let tables = self.name_tables().surnames.get_or_init(|| {
            let d = self.name_data();
            let ncol = d.columns.len();
            let rare = p.names.surnames.rare_below as u64;
            (0..p.heritage_count())
                .map(|h| {
                    let col = p.names.column(&p.heritage, d, Heritage(h as u8));
                    let count = |s: usize| d.surname_counts[s * ncol + col] as u64;
                    let total = |s: usize| -> u64 {
                        d.surname_counts[s * ncol..(s + 1) * ncol]
                            .iter()
                            .map(|&c| c as u64)
                            .sum()
                    };
                    let rare_mass: u64 = (0..d.surnames.len())
                        .filter(|&s| total(s) < rare)
                        .map(count)
                        .sum();
                    let lift = if rare_mass > 0 {
                        1.0 + d.surname_tail[col] as f64 / rare_mass as f64
                    } else {
                        1.0
                    };
                    Table::new((0..d.surnames.len()).map(|s| {
                        let w = count(s) as f64;
                        (s as u32, if total(s) < rare { w * lift } else { w })
                    }))
                })
                .collect()
        });
        tables[h.index()].as_ref()
    }

    /// `x`'s surname at birth.
    pub fn birth_surname(&self, x: PersonId) -> Surname {
        let p = &self.ledger().params;
        let rules = &p.names.surnames;
        let k = self.key().with2(TAG_CHILD, x as u64);
        let Some(mother) = self.mother(x) else {
            // A founder or an immigrant arriving as an adult.
            let s = self
                .surname_table(self.heritage(x))
                .map_or(0, |t| t.draw(self.key().with2(TAG_SURNAME, x as u64)));
            return Surname::one(s);
        };
        let birth = self.birth(x);
        let Some(father) = self.father(x) else {
            return self.surname(mother, birth);
        };
        // Which parent's surname, decided before either is computed, so a
        // surname walks up one line of ancestors, not every line. A child
        // taking a parent's surname takes it as it is at the birth; the
        // mother's part of a double or joint surname is her own (birth)
        // line, not a name she took at a wedding.
        let line = |p: PersonId| self.surname(p, birth).first;
        let own_line = |p: PersonId| self.birth_surname(p).first;
        // Spanish double surnames.
        let double = &rules.double;
        if let Some(dg) = p.heritage.find(&double.group) {
            let share = double.share.at(year_of(birth))
                * match (self.heritage(father) == dg, self.heritage(mother) == dg) {
                    (true, true) => 1.0,
                    (true, false) => 0.5,
                    _ => 0.0,
                };
            if k.with(TAG_DOUBLE).unit() < share {
                return Surname {
                    first: line(father),
                    second: Some(own_line(mother)),
                    hyphen: k.with2(TAG_DOUBLE, 1).unit() < double.hyphenated,
                };
            }
        }
        // Married at the birth? The mother's union with the father then.
        let married = self
            .unions(mother)
            .into_iter()
            .flatten()
            .filter(|u| u.partner == father && u.start <= birth)
            .any(|u| self.marriage_date(mother, &u).is_some_and(|m| m <= birth));
        let shares = if married {
            rules.child.married
        } else {
            rules.child.cohabiting
        };
        let u = k.unit();
        if u < shares.father {
            Surname::one(line(father))
        } else if u < shares.father + shares.mother {
            Surname::one(line(mother))
        } else {
            Surname {
                first: line(father),
                second: Some(own_line(mother)),
                hyphen: true,
            }
        }
    }

    /// `x`'s surname at `t`: the birth surname, changed at weddings and
    /// perhaps reverted after separations (`names.ron`).
    pub fn surname(&self, x: PersonId, t: i64) -> Surname {
        let mut s = self.birth_surname(x);
        let reverts = self.ledger().params.names.surnames.separation_reverts;
        let mut unions: Vec<Union> = self.unions(x).into_iter().flatten().collect();
        unions.sort_by_key(|u| u.start);
        for u in unions {
            let Some(m) = self.marriage_date(x, &u).filter(|&m| m <= t) else {
                continue;
            };
            let change = self.wedding_change(x, &u, m);
            let partner_before = || self.surname(u.partner, m - 1);
            match change {
                Change::Takes(p) if p == x => s = partner_before(),
                Change::Hyphenates(p) if p == x => {
                    s = Surname {
                        first: s.first,
                        second: Some(partner_before().first),
                        hyphen: true,
                    }
                }
                _ => continue,
            }
            // A separation (not a death) may undo the change.
            if u.separation.is_some_and(|sep| sep == u.end && sep <= t)
                && u.key.with2(TAG_REVERT, x as u64).unit() < reverts
            {
                s = self.birth_surname(x);
            }
        }
        s
    }

    /// What the wedding of union `u` (on `wedding`) does to surnames: the
    /// same from both partners' sides.
    fn wedding_change(&self, x: PersonId, u: &Union, wedding: i64) -> Change {
        let p = &self.ledger().params;
        let w = &p.names.surnames.wedding;
        let k = u.key.with(TAG_WEDDING);
        let r = k.unit();
        let year = year_of(wedding);
        let (a, b) = (x.min(u.partner), x.max(u.partner));
        if self.sex(a) == self.sex(b) {
            let who = if k.with(1).unit() < 0.5 { a } else { b };
            return if r < w.same_sex_takes {
                Change::Takes(who)
            } else if r < w.same_sex_takes + w.same_sex_hyphenates {
                Change::Hyphenates(who)
            } else {
                Change::Neither
            };
        }
        let (woman, man) = if self.sex(a).is_female() {
            (a, b)
        } else {
            (b, a)
        };
        let takes = w.woman_takes.at(year);
        let hyphen = w.woman_hyphenates.at(year);
        let mut keeps = (1.0 - takes - hyphen).max(0.0);
        if year - self.birth_year(woman) >= w.older_bride_age {
            keeps *= w.older_bride_keeps;
        }
        let h = self.heritage(woman);
        if let Some(f) = w
            .keeps_more
            .iter()
            .find(|g| p.heritage.find(&g.group) == Some(h))
        {
            keeps *= f.factor;
        }
        let keeps = keeps.min(1.0);
        // Scale taking and hyphenating to what keeping leaves.
        let rest = (1.0 - keeps) / (takes + hyphen).max(1e-12);
        let (takes, hyphen) = (takes * rest, hyphen * rest);
        let man_takes = w.man_takes.at(year);
        if r < man_takes {
            Change::Takes(man)
        } else if r < man_takes + (1.0 - man_takes) * takes {
            Change::Takes(woman)
        } else if r < man_takes + (1.0 - man_takes) * (takes + hyphen) {
            Change::Hyphenates(woman)
        } else {
            Change::Neither
        }
    }

    /// A surname as text: "Garcia", "Garcia Lopez" or "Smith-Jones".
    pub fn surname_text(&self, s: Surname) -> String {
        let d = self.name_data();
        let first = &d.surnames[s.first as usize];
        match s.second {
            None => first.to_string(),
            Some(b) => format!(
                "{first}{}{}",
                if s.hyphen { "-" } else { " " },
                d.surnames[b as usize]
            ),
        }
    }

    /// `x`'s full name at `t`: first, middle (if any) and surname.
    pub fn full_name(&self, x: PersonId, t: i64) -> String {
        let mut out = self.first_name(x).to_string();
        if let Some(m) = self.middle_name(x) {
            out.push(' ');
            out.push_str(m);
        }
        out.push(' ');
        out.push_str(&self.surname_text(self.surname(x, t)));
        out
    }
}
