//! Names on the monotone world: first and middle names, surnames, and
//! marriages, as pure functions of `(seed, id, t)` over its kin lookups.
//! Practice and sources: the pack's `names.ron` and `unions.ron`
//! (`research/2026-10-01-heritage-and-names.md`); ported from the ledger
//! world's naming (archived in `.scratch/archive/2026-10-03-old-worlds/`).
//!
//! - **First names.** A person is named from their birth year's SSA names,
//!   split across heritage groups by `P(group | name)` from Census 2020,
//!   raked (IPF) to the world's own births by group that year, so names have
//!   SSA's frequencies overall and each group's leanings. The draw is exact
//!   rejection from the year's table (shared by every group): a name drawn
//!   by its count is kept with probability its group share over the group's
//!   largest share, so only a year's rake factors and envelopes are stored.
//!   A child of parents in different groups is named in one parent's
//!   tradition.
//! - **Surnames.** Founders draw one from their group. A child takes the
//!   father's, the mother's or both, by whether the parents were married at
//!   the birth; Hispanic families sometimes give two. A partner may take the
//!   other's surname at the wedding and may revert after a separation, so a
//!   surname is a function of time. A surname walks one line of ancestors
//!   (a few lookups a generation).
//! - **Marriages.** Unions are marriages and cohabitations together; a
//!   union marries at its start, later, or never, by era.
//!
//! The world's name tables are built lazily (a year and sex's rake on the
//! first name of that year, a group's surname table on its first draw).

use std::sync::OnceLock;

use procedural_core::fit::rake_columns_dense;
use procedural_core::key::Key;
use procedural_core::sample::exp1_by_inversion;
use procedural_core::stream::year_of;
use procedural_core::table::CumTable;

use super::{Mono, Pid, Union, YEAR};
use crate::names::NameData;
use crate::params::{Heritage, Sex};

const T_FIRST: u64 = 20;
const T_MIDDLE: u64 = 21;
const T_SURNAME: u64 = 22;
const T_CHILD: u64 = 23;
const T_DOUBLE: u64 = 24;
const T_MARRIAGE: u64 = 25;
const T_WEDDING: u64 = 26;
const T_REVERT: u64 = 27;
const T_TRADITION: u64 = 28;

/// Rejection attempts before a draw takes its last candidate (each attempt
/// accepts with probability at least the group's share of the year's
/// births over its largest name share; a few dozen at worst).
const MAX_ATTEMPTS: u64 = 4096;

/// A year and sex's first-name factors.
struct FirstYear {
    /// The world's births by group that year (shares): names outside the
    /// census take this profile.
    pi: Vec<f64>,
    /// Rake factors: a name's share in group `h` is `q_h b_h / Σ_g q_g b_g`.
    b: Vec<f64>,
    /// Per group, the largest share of any of the year's names (the
    /// rejection envelope).
    env: Vec<f64>,
}

/// The world's lazily built name tables.
pub(super) struct NameCache {
    /// `[(year − first_year) * 2 + sex]`.
    first: Vec<OnceLock<FirstYear>>,
    /// Surnames per group, then the whole population's.
    surnames: OnceLock<Vec<Option<CumTable<u32>>>>,
}

impl NameCache {
    pub(super) fn new(data: &NameData) -> Self {
        let years = (data.last_year - data.first_year + 1).max(0) as usize;
        NameCache { first: (0..years * 2).map(|_| OnceLock::new()).collect(), surnames: OnceLock::new() }
    }

    pub(super) fn heap_bytes(&self) -> usize {
        let first: usize = self.first.iter().filter_map(|f| f.get()).map(|f| (f.pi.len() + f.b.len() + f.env.len()) * 8).sum();
        let sur: usize = self.surnames.get().map_or(0, |t| t.iter().flatten().map(|t| t.len() * 8).sum());
        first + sur + self.first.len() * std::mem::size_of::<OnceLock<FirstYear>>()
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
        Surname { first, second: None, hyphen: false }
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
    /// The wife (`true`) or the husband takes the other's surname.
    Takes(bool),
    /// The wife (`true`) or the husband hyphenates their surname with the
    /// other's.
    Hyphenates(bool),
}

impl<'a> Mono<'a> {
    pub(super) fn name_data(&self) -> &'a NameData {
        let p: &'a crate::Params = self.p;
        &p.name_data
    }

    /// A person's key for tag `tag`.
    #[inline]
    pub(super) fn pkey(&self, x: Pid, tag: u64) -> Key {
        self.cell(x.cell).key.with3(tag, x.y as u64, x.i)
    }

    // --- marriages ----------------------------------------------------------------------

    /// The date union `u` becomes a marriage, or `None` if it stays a
    /// cohabitation: some unions begin as marriages; some cohabitations
    /// marry later if the couple is still together, by era (`unions.ron`).
    pub fn marriage_of(&self, u: &Union) -> Option<i64> {
        let m = &self.p.unions.marriage;
        let k = self.pkey(u.wife, T_MARRIAGE);
        let year = year_of(u.start);
        let date = if k.unit() < m.at_start.at(year) {
            u.start
        } else if k.with(2).unit() < m.later.at(year) {
            u.start + (exp1_by_inversion(k.with(3).unit()) * m.delay_years * YEAR) as i64
        } else {
            return None;
        };
        (date < u.end).then_some(date)
    }

    /// The date `x`'s union becomes a marriage (see [`Self::marriage_of`]),
    /// `None` if it stays a cohabitation or `x` never partners.
    pub fn marriage_date(&self, x: Pid) -> Option<i64> {
        self.marriage_of(&self.union_of(x)?)
    }

    /// Whether `x` is married at `t`.
    pub fn married_at(&self, x: Pid, t: i64) -> bool {
        self.union_of(x).is_some_and(|u| t < u.end && self.marriage_of(&u).is_some_and(|m| m <= t))
    }

    // --- first and middle names ------------------------------------------------------

    /// The heritage group whose naming tradition `x` was named in: their own,
    /// or for a child of parents in different groups, one parent's (`None`
    /// in a heritage-blind world: the whole population's).
    pub fn naming_group(&self, x: Pid) -> Option<Heritage> {
        self.naming_group_with(x, self.parents(x).and_then(|p| p.2).map(|u| u.husband))
    }

    fn naming_group_with(&self, x: Pid, father: Option<Pid>) -> Option<Heritage> {
        let own = self.heritage_of(x)?;
        let Some(f) = father else { return Some(own) };
        let fh = self.heritage_of(f)?;
        if fh == own || self.pkey(x, T_TRADITION).unit() < self.p.names.first_names.mixed_mother {
            Some(own)
        } else {
            Some(fh)
        }
    }

    /// `x`'s first name id (see [`Self::first_name`]).
    pub fn first_name_id(&self, x: Pid) -> u32 {
        self.given_name(x, self.naming_group(x), self.pkey(x, T_FIRST), None)
    }

    /// `x`'s first name.
    pub fn first_name(&self, x: Pid) -> &'a str {
        &self.name_data().first[self.first_name_id(x) as usize]
    }

    /// A given name for `x` from tradition `h`, their birth year and sex,
    /// drawn with `key`; `avoid` is redrawn (a few times) if drawn.
    fn given_name(&self, x: Pid, h: Option<Heritage>, key: Key, avoid: Option<u32>) -> u32 {
        let sex = self.sex(x);
        for attempt in 0..4u64 {
            let id = self.draw_first(x.y, sex, h, key.with(attempt));
            if Some(id) != avoid {
                return id;
            }
        }
        avoid.unwrap_or(0)
    }

    /// A first name of `year` and `sex` in group `h`'s tradition: the
    /// year's names by count, each kept with probability its share in `h`
    /// over `h`'s envelope (exact rejection), with keyed attempts.
    fn draw_first(&self, year: i32, sex: Sex, h: Option<Heritage>, key: Key) -> u32 {
        let d = self.name_data();
        let rows = d.ssa_year(year, sex);
        let Some(&(_, total)) = rows.last() else { return 0 };
        let fy = h.map(|_| self.first_year_factors(year, sex));
        let mut name = 0;
        for k in 0..MAX_ATTEMPTS {
            let kk = key.with2(0x51, k);
            let v = kk.below(total as u64) as u32;
            name = rows[rows.partition_point(|r| r.1 <= v)].0;
            let (Some(h), Some(fy)) = (h, fy) else { return name };
            let s = self.name_share(fy, name, h.0 as usize);
            if kk.with(1).unit() * fy.env[h.0 as usize] < s {
                return name;
            }
        }
        name
    }

    /// The share of first name `name` that falls to group `h` under a
    /// year's rake: `q_h b_h / Σ_g q_g b_g`, `q` the name's census profile
    /// over the world's groups (the year's births by group if unlisted).
    fn name_share(&self, fy: &FirstYear, name: u32, h: usize) -> f64 {
        let d = self.name_data();
        let cols = self.group_columns();
        let c = d.census_of(name);
        let raw: f64 = c.map_or(0.0, |c| cols.iter().map(|&col| c[col] as f64).sum());
        let q = |g: usize| if raw > 0.0 { c.unwrap()[cols[g]] as f64 / raw } else { fy.pi[g] };
        let z: f64 = (0..cols.len()).map(|g| q(g) * fy.b[g]).sum();
        if z > 0.0 { q(h) * fy.b[h] / z } else { 0.0 }
    }

    /// Each group's column in the name data.
    fn group_columns(&self) -> Vec<usize> {
        let p = self.p;
        (0..p.heritage_count()).map(|h| p.names.column(&p.heritage, self.name_data(), Heritage(h as u8))).collect()
    }

    /// A year and sex's rake factors and envelopes, built on first use.
    fn first_year_factors(&self, year: i32, sex: Sex) -> &FirstYear {
        let d = self.name_data();
        let y = year.clamp(d.first_year, d.last_year);
        self.names.first[(y - d.first_year) as usize * 2 + sex as usize].get_or_init(|| {
            let cols = self.group_columns();
            let groups = cols.len();
            // The world's births by group in `year` (the data's year is the
            // nearest; outside the world's years, even shares).
            let births: Vec<f64> = (0..self.cells()).map(|c| self.cohort_n(c, year) as f64).collect();
            let total_births: f64 = births.iter().sum();
            let pi: Vec<f64> = if total_births > 0.0 && births.len() == groups {
                births.iter().map(|b| b / total_births).collect()
            } else {
                vec![1.0 / groups as f64; groups]
            };
            let rows = d.ssa_year(y, sex);
            let counts: Vec<f64> = rows.iter().enumerate().map(|(i, r)| (r.1 - if i > 0 { rows[i - 1].1 } else { 0 }) as f64).collect();
            let mut q = vec![0.0f64; rows.len() * groups];
            for (i, &(name, _)) in rows.iter().enumerate() {
                let c = d.census_of(name);
                let raw: f64 = c.map_or(0.0, |c| cols.iter().map(|&col| c[col] as f64).sum());
                for g in 0..groups {
                    q[i * groups + g] = if raw > 0.0 { c.unwrap()[cols[g]] as f64 / raw } else { pi[g] };
                }
            }
            let all: f64 = counts.iter().sum();
            let targets: Vec<f64> = pi.iter().map(|&s| s * all).collect();
            let (b, _) = rake_columns_dense(&counts, &q, &targets, 200, 1e-9);
            let mut env = vec![0.0f64; groups];
            for i in 0..rows.len() {
                let qi = &q[i * groups..(i + 1) * groups];
                let z: f64 = (0..groups).map(|g| qi[g] * b[g]).sum();
                if z > 0.0 {
                    for g in 0..groups {
                        env[g] = env[g].max(qi[g] * b[g] / z);
                    }
                }
            }
            FirstYear { pi, b, env }
        })
    }

    /// `x`'s middle name, if any.
    pub fn middle_name_of(&self, x: Pid) -> Option<Middle> {
        let parents = self.parents(x);
        let h = self.naming_group_with(x, parents.and_then(|p| p.2).map(|u| u.husband));
        let first = self.given_name(x, h, self.pkey(x, T_FIRST), None);
        self.middle_with(x, h, parents.map(|p| p.0), first)
    }

    /// [`Self::middle_name_of`] given the tradition, the mother and the
    /// first name.
    fn middle_with(&self, x: Pid, h: Option<Heritage>, mother: Option<Pid>, first: u32) -> Option<Middle> {
        let p = self.p;
        let mn = &p.names.middle_names;
        let female = self.sex(x) == Sex::Female;
        let share = h
            .and_then(|h| mn.by_group.iter().find(|g| p.heritage.find(&g.group) == Some(h)))
            .map_or(&mn.default, |g| &g.share)
            .get(female)
            .at(x.y);
        let k = self.pkey(x, T_MIDDLE);
        if k.unit() >= share {
            return None;
        }
        if k.with(1).unit() < mn.family_surname.get(female).at(x.y) {
            if let Some(m) = mother {
                return Some(Middle::Family(self.birth_surname(m).first));
            }
        }
        Some(Middle::Given(self.given_name(x, h, k.with(2), Some(first))))
    }

    /// `x`'s middle name as text.
    pub fn middle_name(&self, x: Pid) -> Option<&'a str> {
        let d = self.name_data();
        self.middle_name_of(x).map(|m| match m {
            Middle::Given(i) => &*d.first[i as usize],
            Middle::Family(i) => &*d.surnames[i as usize],
        })
    }

    // --- surnames -------------------------------------------------------------------------

    /// The surname tables: per group, census counts in the group's column
    /// (surnames the group holds), the unlisted tail spread over rare names;
    /// then the whole population's (every column).
    pub(super) fn surname_table(&self, h: Option<Heritage>) -> Option<&CumTable<u32>> {
        let tables = self.names.surnames.get_or_init(|| {
            let d = self.name_data();
            let ncol = d.columns.len();
            let rare = self.p.names.surnames.rare_below as u64;
            let total = |s: usize| -> u64 { d.surname_counts[s * ncol..(s + 1) * ncol].iter().map(|&c| c as u64).sum() };
            let cols: Vec<Vec<usize>> = self.group_columns().into_iter().map(|c| vec![c]).chain(std::iter::once((0..ncol).collect())).collect();
            cols.iter()
                .map(|cs| {
                    let count = |s: usize| -> u64 { cs.iter().map(|&c| d.surname_counts[s * ncol + c] as u64).sum() };
                    let tail: u64 = cs.iter().map(|&c| d.surname_tail[c]).sum();
                    let rare_mass: u64 = (0..d.surnames.len()).filter(|&s| total(s) < rare).map(count).sum();
                    let lift = if rare_mass > 0 { 1.0 + tail as f64 / rare_mass as f64 } else { 1.0 };
                    CumTable::new((0..d.surnames.len()).map(|s| {
                        let w = count(s) as f64;
                        (s as u32, if total(s) < rare { w * lift } else { w })
                    }))
                })
                .collect()
        });
        tables[h.map_or(tables.len() - 1, |h| h.0 as usize)].as_ref()
    }

    /// `x`'s surname at birth.
    pub fn birth_surname(&self, x: Pid) -> Surname {
        self.birth_surname_with(x, self.parents(x))
    }

    /// [`Self::birth_surname`] given `x`'s [`Self::parents`].
    fn birth_surname_with(&self, x: Pid, parents: Option<(Pid, i64, Option<Union>)>) -> Surname {
        let rules = &self.p.names.surnames;
        let k = self.pkey(x, T_CHILD);
        let Some((mother, birth, union)) = parents else {
            // A founder.
            return Surname::one(self.surname_table(self.heritage_of(x)).map_or(0, |t| t.draw(self.pkey(x, T_SURNAME))));
        };
        let Some(u) = union else {
            return self.surname(mother, birth);
        };
        let father = u.husband;
        // Which parent's surname, decided before either is computed, so a
        // surname walks up one line of ancestors, not every line. A child
        // taking a parent's surname takes it as it is at the birth; the
        // mother's part of a double or joint surname is her own (birth)
        // line, not a name she took at a wedding.
        // The father's surname at the birth, his union with the mother known.
        let line_father = || self.surname_in(father, birth, Some(u)).first;
        let own_line = |p: Pid| self.birth_surname(p).first;
        // Spanish double surnames.
        let double = &rules.double;
        if let Some(dg) = self.p.heritage.find(&double.group) {
            let share = double.share.at(year_of(birth))
                * match (self.heritage_of(father) == Some(dg), self.heritage_of(mother) == Some(dg)) {
                    (true, true) => 1.0,
                    (true, false) => 0.5,
                    _ => 0.0,
                };
            if k.with(T_DOUBLE).unit() < share {
                return Surname { first: line_father(), second: Some(own_line(mother)), hyphen: k.with2(T_DOUBLE, 1).unit() < double.hyphenated };
            }
        }
        // Married at the birth? (The father is the mother's partner.)
        let married = self.marriage_of(&u).is_some_and(|m| m <= birth);
        let shares = if married { rules.child.married } else { rules.child.cohabiting };
        let r = k.unit();
        if r < shares.father {
            Surname::one(line_father())
        } else if r < shares.father + shares.mother {
            Surname::one(self.surname_in(mother, birth, Some(u)).first)
        } else {
            Surname { first: line_father(), second: Some(own_line(mother)), hyphen: true }
        }
    }

    /// `x`'s surname at `t`: the birth surname, changed at a wedding and
    /// perhaps reverted after a separation (`names.ron`).
    pub fn surname(&self, x: Pid, t: i64) -> Surname {
        self.surname_in(x, t, self.union_of(x))
    }

    /// [`Self::surname`] given `x`'s union (`None`: none).
    fn surname_in(&self, x: Pid, t: i64, union: Option<Union>) -> Surname {
        match union {
            None => self.birth_surname(x),
            Some(_) => self.surname_from(x, t, self.birth_surname(x), union),
        }
    }

    /// [`Self::surname`] given `x`'s birth surname `s` and union.
    fn surname_from(&self, x: Pid, t: i64, s: Surname, union: Option<Union>) -> Surname {
        let Some(u) = union else { return s };
        let Some(m) = self.marriage_of(&u).filter(|&m| m <= t) else { return s };
        let partner = u.partner(x);
        let wife = x == u.wife;
        let changed = match self.wedding_change(&u, m) {
            // The partner's surname before the wedding: their first union
            // is this one, so their birth surname.
            Change::Takes(w) if w == wife => self.birth_surname(partner),
            Change::Hyphenates(w) if w == wife => Surname { first: s.first, second: Some(self.birth_surname(partner).first), hyphen: true },
            _ => return s,
        };
        // A separation (not a death) may undo the change.
        let reverts = self.p.names.surnames.separation_reverts;
        if u.how == 0 && u.end <= t && self.pkey(u.wife, T_REVERT).with(wife as u64).unit() < reverts {
            return s;
        }
        changed
    }

    /// What the wedding of union `u` (on `wedding`) does to surnames: the
    /// same from both partners' sides.
    fn wedding_change(&self, u: &Union, wedding: i64) -> Change {
        let p = self.p;
        let w = &p.names.surnames.wedding;
        let r = self.pkey(u.wife, T_WEDDING).unit();
        let year = year_of(wedding);
        let woman = u.wife;
        let takes = w.woman_takes.at(year);
        let hyphen = w.woman_hyphenates.at(year);
        let mut keeps = (1.0 - takes - hyphen).max(0.0);
        if year - woman.y >= w.older_bride_age {
            keeps *= w.older_bride_keeps;
        }
        if let Some(h) = self.heritage_of(woman) {
            if let Some(f) = w.keeps_more.iter().find(|g| p.heritage.find(&g.group) == Some(h)) {
                keeps *= f.factor;
            }
        }
        let keeps = keeps.min(1.0);
        // Scale taking and hyphenating to what keeping leaves.
        let rest = (1.0 - keeps) / (takes + hyphen).max(1e-12);
        let (takes, hyphen) = (takes * rest, hyphen * rest);
        let man_takes = w.man_takes.at(year);
        if r < man_takes {
            Change::Takes(false)
        } else if r < man_takes + (1.0 - man_takes) * takes {
            Change::Takes(true)
        } else if r < man_takes + (1.0 - man_takes) * (takes + hyphen) {
            Change::Hyphenates(true)
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
            Some(b) => format!("{first}{}{}", if s.hyphen { "-" } else { " " }, d.surnames[b as usize]),
        }
    }

    /// `x`'s full name at `t`: first, middle (if any) and surname.
    pub fn full_name(&self, x: Pid, t: i64) -> String {
        let d = self.name_data();
        let parents = self.parents(x);
        let h = self.naming_group_with(x, parents.and_then(|p| p.2).map(|u| u.husband));
        let first = self.given_name(x, h, self.pkey(x, T_FIRST), None);
        let mut out = d.first[first as usize].to_string();
        if let Some(m) = self.middle_with(x, h, parents.map(|p| p.0), first) {
            out.push(' ');
            out.push_str(match m {
                Middle::Given(i) => &d.first[i as usize],
                Middle::Family(i) => &d.surnames[i as usize],
            });
        }
        out.push(' ');
        let s = self.surname_from(x, t, self.birth_surname_with(x, parents), self.union_of(x));
        out.push_str(&self.surname_text(s));
        out
    }
}
