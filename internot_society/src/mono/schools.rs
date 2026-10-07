//! Schools and colleges (L5 institutions) on the monotone world: which
//! school a child attends at `t`, and which college and graduate school a
//! student chose, as pure functions of `(seed, id, t)` over residence and
//! education. Pack `education.ron` (`data`, `private_share`, `college`);
//! data `data/schools.bin` (`internot_society/data/distill_schools.py`).
//!
//! - **K–12:** the nearest school offering the child's grade to their home
//!   that September (attendance zones are close to nearest-school
//!   partitions), public, or private with the era's share (a keyed choice
//!   per person and school level). A grid index over the schools answers
//!   nearest queries in a few cells.
//! - **College:** drawn by enrollment times a distance decay from home the
//!   day before entry, in the home state with the pack's share; those who
//!   stop at some college or an associate degree attend a two-year college
//!   with the era's share. Graduate school likewise, by graduate enrollment
//!   with a weaker decay.
//!
//! Debt: today's institutions serve every era.

use procedural_core::key::Key;
use procedural_core::stream::{year_of, year_start, DAY};

use super::education::{EducationPath, Level};
use super::residence::{Strings, TRACT};
use super::{Mono, Pid};

const T_PRIVATE: u64 = 90;
const T_COLLEGE: u64 = 91;
const T_GRAD: u64 = 92;
const T_TWO_YEAR: u64 = 93;

/// Grid cells of 0.25° for the nearest-school index.
const CELL: f64 = 0.25;

/// A K–12 school or a college.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Institution {
    School(u32),
    College(u32),
}

/// Kinds of institution, as served.
#[derive(Clone, Debug)]
pub struct InstitutionInfo {
    pub name: String,
    pub city: String,
    pub state: &'static str,
    /// "public school", "private school", "four-year college", …
    pub kind: &'static str,
}

/// The schools and colleges.
pub(super) struct Institutions {
    s_kind: Vec<u8>,
    s_lo: Vec<u8>,
    s_hi: Vec<u8>,
    s_at: Vec<[f32; 2]>,
    s_state: Vec<u8>,
    s_name: Strings,
    s_city: Strings,
    /// Grid: cell `(i, j)` (latitude, longitude rows from `lat0`, `lon0`)
    /// holds `cell_items[cell_at[c]..cell_at[c + 1]]`.
    lat0: f64,
    lon0: f64,
    nlat: usize,
    nlon: usize,
    cell_at: Vec<u32>,
    cell_items: Vec<u32>,
    c_level: Vec<u8>,
    c_control: Vec<u8>,
    c_at: Vec<[f32; 2]>,
    c_ug: Vec<u32>,
    c_gr: Vec<u32>,
    c_state: Vec<u8>,
    c_name: Strings,
    c_city: Strings,
    /// Colleges by state FIPS.
    by_state: Vec<Vec<u32>>,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("schools data is truncated")?;
        self.i += n;
        Ok(s)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn leb(&mut self) -> Result<u64, String> {
        let (mut n, mut shift) = (0u64, 0);
        loop {
            let x = self.byte()?;
            n |= ((x & 0x7f) as u64) << shift;
            if x < 0x80 {
                return Ok(n);
            }
            shift += 7;
            if shift > 63 {
                return Err("schools data: bad varint".into());
            }
        }
    }

    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn text(&mut self) -> Result<String, String> {
        let n = self.byte()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

/// Miles between two points, flat-earth (for ranking near points).
#[inline]
fn miles(a: [f32; 2], lat: f64, lon: f64) -> f64 {
    let (la, lo) = (a[0] as f64, a[1] as f64);
    let dy = (la - lat) * 69.09;
    let dx = (lo - lon) * 69.17 * procedural_core::dmath::cos(((la + lat) * 0.5).to_radians());
    (dx * dx + dy * dy).sqrt()
}

impl Institutions {
    pub(super) fn from_data(b: &[u8]) -> Result<Self, String> {
        let mut r = Reader { b, i: 0 };
        if r.take(8)? != b"INTSCHOL" || r.byte()? != 1 {
            return Err("not a version-1 schools file".into());
        }
        let n = r.leb()? as usize;
        let (mut s_kind, mut s_lo, mut s_hi, mut s_at, mut s_state) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        let (mut s_name, mut s_city) = (Strings::default(), Strings::default());
        for _ in 0..n {
            s_kind.push(r.byte()?);
            s_lo.push(r.byte()?);
            s_hi.push(r.byte()?);
            s_at.push([r.f32()?, r.f32()?]);
            let _students = r.leb()?;
            s_state.push(r.byte()?);
            s_name.push(&r.text()?);
            s_city.push(&r.text()?);
        }
        let m = r.leb()? as usize;
        let (mut c_level, mut c_control, mut c_at, mut c_ug, mut c_gr, mut c_state) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (mut c_name, mut c_city) = (Strings::default(), Strings::default());
        for _ in 0..m {
            c_level.push(r.byte()?);
            c_control.push(r.byte()?);
            let _grad = r.byte()?;
            c_at.push([r.f32()?, r.f32()?]);
            c_ug.push(r.leb()?.min(u32::MAX as u64) as u32);
            c_gr.push(r.leb()?.min(u32::MAX as u64) as u32);
            c_state.push(r.byte()?);
            c_name.push(&r.text()?);
            c_city.push(&r.text()?);
        }
        // The grid over the schools' extent.
        let (mut la0, mut la1, mut lo0, mut lo1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for a in &s_at {
            la0 = la0.min(a[0] as f64);
            la1 = la1.max(a[0] as f64);
            lo0 = lo0.min(a[1] as f64);
            lo1 = lo1.max(a[1] as f64);
        }
        let nlat = ((la1 - la0) / CELL) as usize + 1;
        let nlon = ((lo1 - lo0) / CELL) as usize + 1;
        let cell_of = |a: [f32; 2]| ((a[0] as f64 - la0) / CELL) as usize * nlon + ((a[1] as f64 - lo0) / CELL) as usize;
        let mut counts = vec![0u32; nlat * nlon + 1];
        for &a in &s_at {
            counts[cell_of(a) + 1] += 1;
        }
        for c in 0..nlat * nlon {
            counts[c + 1] += counts[c];
        }
        let mut fill = counts.clone();
        let mut cell_items = vec![0u32; n];
        for (i, &a) in s_at.iter().enumerate() {
            let c = cell_of(a);
            cell_items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        let mut by_state = vec![Vec::new(); 64];
        for (i, &st) in c_state.iter().enumerate() {
            by_state[st as usize].push(i as u32);
        }
        Ok(Institutions {
            s_kind,
            s_lo,
            s_hi,
            s_at,
            s_state,
            s_name,
            s_city,
            lat0: la0,
            lon0: lo0,
            nlat,
            nlon,
            cell_at: counts,
            cell_items,
            c_level,
            c_control,
            c_at,
            c_ug,
            c_gr,
            c_state,
            c_name,
            c_city,
            by_state,
        })
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.s_kind.capacity() * 4
            + self.s_at.capacity() * 8
            + self.s_name.heap_bytes()
            + self.s_city.heap_bytes()
            + (self.cell_at.capacity() + self.cell_items.capacity()) * 4
            + self.c_at.capacity() * 19
            + self.c_name.heap_bytes()
            + self.c_city.heap_bytes()
            + self.by_state.iter().map(|v| v.capacity() * 4 + 24).sum::<usize>()
    }

    /// The nearest school of `kind` (0 public, 1 private) offering `grade`
    /// to `(lat, lon)`, searching grid rings outward.
    pub(super) fn nearest_school(&self, lat: f64, lon: f64, grade: u8, kind: u8) -> Option<u32> {
        let ci = (((lat - self.lat0) / CELL).floor() as i64).clamp(0, self.nlat as i64 - 1);
        let cj = (((lon - self.lon0) / CELL).floor() as i64).clamp(0, self.nlon as i64 - 1);
        let mut best: Option<(f64, u32)> = None;
        let max_ring = self.nlat.max(self.nlon) as i64;
        for ring in 0..max_ring {
            // Everything in this ring is at least (ring − 1) cells away.
            if let Some((d, _)) = best {
                if d < (ring - 1).max(0) as f64 * CELL * 69.0 * 0.5 {
                    break;
                }
            }
            for i in ci - ring..=ci + ring {
                for j in cj - ring..=cj + ring {
                    if (i - ci).abs() != ring && (j - cj).abs() != ring {
                        continue;
                    }
                    if i < 0 || j < 0 || i >= self.nlat as i64 || j >= self.nlon as i64 {
                        continue;
                    }
                    let c = i as usize * self.nlon + j as usize;
                    for &s in &self.cell_items[self.cell_at[c] as usize..self.cell_at[c + 1] as usize] {
                        let su = s as usize;
                        if self.s_kind[su] != kind || grade < self.s_lo[su] || grade > self.s_hi[su] {
                            continue;
                        }
                        let d = miles(self.s_at[su], lat, lon);
                        if best.is_none_or(|b| d < b.0 || (d == b.0 && s < b.1)) {
                            best = Some((d, s));
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// A college drawn for a student at `(lat, lon)` in `state`: enrollment
    /// (graduate or undergraduate, of the wanted level) times the distance
    /// decay, in the state with share `in_state`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_college(&self, lat: f64, lon: f64, state: u8, level: Option<u8>, graduate: bool, in_state: f64, scale: f64, decay: f64, key: Key) -> Option<u32> {
        let weight = |c: u32| -> f64 {
            let cu = c as usize;
            if level.is_some_and(|l| self.c_level[cu] != l) {
                return 0.0;
            }
            let n = if graduate { self.c_gr[cu] } else { self.c_ug[cu] } as f64;
            if n <= 0.0 {
                return 0.0;
            }
            n * procedural_core::dmath::pow(1.0 + miles(self.c_at[cu], lat, lon) / scale, -decay)
        };
        let pick = |cands: &mut dyn Iterator<Item = u32>, u: f64| -> Option<u32> {
            let ws: Vec<(u32, f64)> = cands.map(|c| (c, weight(c))).filter(|w| w.1 > 0.0).collect();
            let total: f64 = ws.iter().map(|w| w.1).sum();
            if total <= 0.0 {
                return None;
            }
            let mut v = u * total;
            for &(c, w) in &ws {
                v -= w;
                if v < 0.0 {
                    return Some(c);
                }
            }
            ws.last().map(|w| w.0)
        };
        let u = key.with(1).unit();
        if key.unit() < in_state {
            if let Some(c) = pick(&mut self.by_state[state as usize].iter().copied(), u) {
                return Some(c);
            }
        }
        pick(&mut (0..self.c_at.len() as u32), u)
    }

    pub(super) fn info(&self, i: Institution) -> InstitutionInfo {
        match i {
            Institution::School(s) => {
                let su = s as usize;
                InstitutionInfo {
                    name: self.s_name.get(su).to_string(),
                    city: self.s_city.get(su).to_string(),
                    state: super::residence::state_abbr(self.s_state[su] as u32),
                    kind: if self.s_kind[su] == 0 { "public school" } else { "private school" },
                }
            }
            Institution::College(c) => {
                let cu = c as usize;
                InstitutionInfo {
                    name: self.c_name.get(cu).to_string(),
                    city: self.c_city.get(cu).to_string(),
                    state: super::residence::state_abbr(self.c_state[cu] as u32),
                    kind: match (self.c_level[cu], self.c_control[cu]) {
                        (2, _) => "two-year college",
                        (_, 1) => "public university",
                        (_, 3) => "for-profit college",
                        _ => "private college",
                    },
                }
            }
        }
    }
}

/// A stretch of someone's schooling at one institution.
#[derive(Clone, Debug)]
pub struct Stint {
    pub institution: Institution,
    pub from: i64,
    pub to: i64,
    /// Grades attended (K–12: first and last; 0 is kindergarten), or the
    /// degree earned (`None`: left without one).
    pub grades: Option<(u8, u8)>,
    pub degree: Option<Level>,
}

impl<'a> Mono<'a> {
    pub(super) fn institutions(&self) -> &Institutions {
        self.institutions.get_or_init(|| Institutions::from_data(&self.p.school_data).expect("the pack's schools data is valid"))
    }

    /// Facts about an institution.
    pub fn institution(&self, i: Institution) -> InstitutionInfo {
        self.institutions().info(i)
    }

    /// The point and state of `x`'s home at `t` (tract centre).
    pub(super) fn home_point(&self, x: Pid, t: i64) -> Option<(f64, f64, u8)> {
        let pos = self.address_of(x, t)?;
        let places = self.places();
        let c = places.centre(TRACT, pos[TRACT]);
        let (_, fips) = places.county(pos[super::COUNTY]);
        Some((c.lat, c.lon, (fips / 1000) as u8))
    }

    /// The K–12 school `x` attends for `grade` from home at `t`.
    fn school_for(&self, x: Pid, grade: u8, t: i64) -> Option<u32> {
        let (lat, lon, _) = self.home_point(x, t)?;
        let level = match grade {
            0..=5 => 0u64,
            6..=8 => 1,
            _ => 2,
        };
        let private = self.pkey(x, T_PRIVATE).with(level).unit() < self.p.education.private_share.at(x.y + 6 + grade as i32);
        let inst = self.institutions();
        inst.nearest_school(lat, lon, grade, private as u8).or_else(|| inst.nearest_school(lat, lon, grade, 0))
    }

    /// The school (K–12) `x` attends at `t`, if in school then.
    pub fn school_at(&self, x: Pid, t: i64) -> Option<Institution> {
        let grade = match self.schooling_at(x, t) {
            super::Schooling::Kindergarten => 0,
            super::Schooling::School { grade } => grade,
            _ => return None,
        };
        self.school_for(x, grade, t).map(Institution::School)
    }

    /// The college `x` attends (or attended), if they went.
    pub fn college_of(&self, x: Pid) -> Option<Institution> {
        let path = self.education_path(x);
        self.college_in(x, &path).map(Institution::College)
    }

    fn college_in(&self, x: Pid, path: &EducationPath) -> Option<u32> {
        let (start, _) = path.college?;
        let c = &self.p.education.college;
        let (lat, lon, state) = self.home_point(x, (start - DAY).max(self.birth(x)))?;
        let two_year = matches!(path.level, Level::SomeCollege | Level::Associate) && self.pkey(x, T_TWO_YEAR).unit() < c.two_year.at(year_of(start));
        let level = Some(if two_year { 2 } else { 1 });
        self.institutions().draw_college(lat, lon, state, level, false, c.in_state, c.miles_scale, c.decay, self.pkey(x, T_COLLEGE))
    }

    /// The graduate school `x` attends (or attended), if any.
    pub fn graduate_school_of(&self, x: Pid) -> Option<Institution> {
        let path = self.education_path(x);
        let (start, _) = path.graduate?;
        let c = &self.p.education.college;
        let (lat, lon, state) = self.home_point(x, (start - DAY).max(self.birth(x)))?;
        self.institutions().draw_college(lat, lon, state, Some(1), true, c.graduate_in_state, c.miles_scale, c.graduate_decay, self.pkey(x, T_GRAD)).map(Institution::College)
    }

    /// `x`'s schooling as stints at institutions: each school (by school
    /// year, from home that September; a move or a new level changes
    /// school), then college and graduate school. Up to `until` (death
    /// ends it).
    pub fn education_history(&self, x: Pid, until: i64) -> Vec<Stint> {
        let path = self.education_path(x);
        let until = until.min(self.death(x));
        let mut out: Vec<Stint> = Vec::new();
        let first = path.kindergarten.unwrap_or(path.grade1);
        let first_grade: u8 = if path.kindergarten.is_some() { 0 } else { 1 };
        let mut sept = first;
        let mut grade = first_grade;
        while sept < path.school_end && sept < until && grade <= 12 {
            let next = year_start(year_of(sept) + 1) + 243 * DAY;
            let to = next.min(path.school_end).min(until);
            if let Some(s) = self.school_for(x, grade, sept) {
                let inst = Institution::School(s);
                match out.last_mut() {
                    Some(last) if last.institution == inst && last.to >= sept - 120 * DAY => {
                        last.to = to;
                        if let Some(g) = last.grades.as_mut() {
                            g.1 = grade;
                        }
                    }
                    _ => out.push(Stint { institution: inst, from: sept, to, grades: Some((grade, grade)), degree: None }),
                }
            }
            sept = next;
            grade += 1;
        }
        if let (Some((s, e)), Some(c)) = (path.college, self.college_in(x, &path)) {
            if s < until {
                let degree = (e <= until && path.level >= Level::Associate).then(|| if path.level == Level::Associate { Level::Associate } else { Level::Bachelor });
                out.push(Stint { institution: Institution::College(c), from: s, to: e.min(until), grades: None, degree });
            }
        }
        if let (Some((s, e)), Some(Institution::College(c))) = (path.graduate, self.graduate_school_of(x)) {
            if s < until {
                out.push(Stint { institution: Institution::College(c), from: s, to: e.min(until), grades: None, degree: (e <= until).then_some(path.level) });
            }
        }
        out
    }
}
