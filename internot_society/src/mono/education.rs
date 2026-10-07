//! Education (L5) on the monotone world: the highest level each person
//! completes and their schooling at any time, as pure functions of
//! `(seed, id, t)`. Pack `education.ron`; data and targets
//! `research/2026-10-01-education-data-and-targets.md`.
//!
//! - **A latent** `z = √own·ε_x + √parents·ε(mother's union) +
//!   √grandparents·(ε(mother's mother's union) + ε(father's mother's
//!   union)) + √union·ε(own union)`, each `ε` a standard normal keyed on a
//!   union (a woman's union key is her own, whether or not she partners, so
//!   a mother's children share it with her and her partner). Partners share
//!   their union's term (assortative mating), siblings their parents', and
//!   cousins their grandparents'. Four keys and up to four kin lookups: no
//!   recursion.
//! - **The level** is the latent's quantile in its cohort, sex and group's
//!   attainment: the pack's shares, every threshold shifted by the group's
//!   log-odds. Within some college, the latent's position decides an
//!   associate degree; within graduate degrees, master's, professional or
//!   doctorate.
//! - **The timeline:** kindergarten and grades 1–12 from a September 1
//!   cutoff; leaving without a diploma at the era's age; then college (on
//!   time or later), a degree's years, and graduate school after a gap.
//!   Death ends schooling where it stands.

use procedural_core::dmath::{exp, ln, norm_cdf};
use procedural_core::key::Key;
use procedural_core::sample::std_normal;
use procedural_core::stream::{year_of, year_start, DAY};

use super::{Mono, Pid, YEAR};
use crate::params::Sex;

const T_OWN: u64 = 80;
const T_UNION: u64 = 81;
const T_ALONE: u64 = 82;
const T_TIMING: u64 = 83;

/// Education levels, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    LessThanHighSchool,
    HighSchool,
    SomeCollege,
    Associate,
    Bachelor,
    Master,
    Professional,
    Doctorate,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::LessThanHighSchool => "less than high school",
            Level::HighSchool => "high school",
            Level::SomeCollege => "some college",
            Level::Associate => "associate degree",
            Level::Bachelor => "bachelor's degree",
            Level::Master => "master's degree",
            Level::Professional => "professional degree",
            Level::Doctorate => "doctorate",
        }
    }
}

/// What a person is doing in school at some time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schooling {
    /// Not yet in school.
    NotYet,
    Kindergarten,
    /// Grades 1–12.
    School { grade: u8 },
    /// An undergraduate program (`year` counts from 1), toward `toward`.
    College { year: u8, toward: Level },
    /// A graduate program toward `toward`.
    Graduate { year: u8, toward: Level },
    /// Not enrolled.
    Out,
}

/// A person's schooling: their final level and the dates of each stage.
#[derive(Clone, Copy, Debug)]
pub struct EducationPath {
    /// The level reached if they live long enough.
    pub level: Level,
    /// The latent (standard normal) behind it.
    pub latent: f64,
    /// Kindergarten (if any) and grade 1 start on these Septembers.
    pub kindergarten: Option<i64>,
    pub grade1: i64,
    /// Leaving school: the end of grade 12, or leaving without a diploma.
    pub school_end: i64,
    /// Grades completed by then.
    pub grades: u8,
    /// College: start and end (degree or leaving), if any.
    pub college: Option<(i64, i64)>,
    /// Graduate school: start and degree, if any.
    pub graduate: Option<(i64, i64)>,
}

/// September 1 of `year`.
fn september(year: i32) -> i64 {
    year_start(year) + 243 * DAY
}

/// June 15 of `year`.
fn june(year: i32) -> i64 {
    year_start(year) + 165 * DAY
}

impl<'a> Mono<'a> {
    /// The education latent of `x` (standard normal; see the module notes).
    pub fn education_latent(&self, x: Pid) -> f64 {
        let l = self.p.education.latent;
        let n = |k: Key| std_normal(k);
        // A woman's union key is her own; a man's is his wife's (his own if
        // he has no union).
        let union_key = |p: Pid| -> Key {
            let wife = if self.sex(p) == Sex::Female { p } else { self.spouse(p).map_or(p, |s| s.0) };
            self.pkey(wife, T_UNION)
        };
        let own = n(self.pkey(x, T_OWN));
        let parents = self.parents(x);
        let (mother, father) = match parents {
            Some((m, _, u)) => (Some(m), u.map(|u| u.husband)),
            None => (None, None),
        };
        // Missing kin take independent keys of `x`, so the variance holds.
        let parents_term = mother.map_or_else(|| n(self.pkey(x, T_ALONE).with(1)), |m| n(self.pkey(m, T_UNION)));
        let gm = |p: Option<Pid>, side: u64| -> f64 {
            match p.and_then(|p| self.mother(p)) {
                Some(g) => n(self.pkey(g, T_UNION)),
                None => n(self.pkey(x, T_ALONE).with(2 + side)),
            }
        };
        let grand = gm(mother, 0) + gm(father, 1);
        let union_term = n(union_key(x));
        l.own.sqrt() * own + l.parents.sqrt() * parents_term + l.grandparents.sqrt() * grand + l.union.sqrt() * union_term
    }

    /// The final level for latent `z`: its quantile against the cohort,
    /// sex and group's thresholds.
    fn level_of(&self, x: Pid, z: f64) -> Level {
        let e = &self.p.education;
        let female = self.sex(x) == Sex::Female;
        let s: [f64; 5] = e.attainment.get(female).at(x.y);
        let total: f64 = s.iter().sum();
        let shift = self.heritage_of(x).map_or(0.0, |h| e.shift(&self.p.heritage, h, x.y));
        // Cumulative shares, each moved by the group's log-odds (a higher
        // shift lowers every threshold: more education).
        let logit = |p: f64| ln(p / (1.0 - p));
        let logistic = |v: f64| 1.0 / (1.0 + exp(-v));
        let mut cum = [0.0f64; 4];
        let mut acc = 0.0;
        for k in 0..4 {
            acc += s[k] / total;
            cum[k] = if acc <= 0.0 { 0.0 } else if acc >= 1.0 { 1.0 } else { logistic(logit(acc) - shift) };
        }
        let u = norm_cdf(z);
        let band = |lo: f64, hi: f64| if hi > lo { ((u - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
        if u < cum[0] {
            Level::LessThanHighSchool
        } else if u < cum[1] {
            Level::HighSchool
        } else if u < cum[2] {
            // The top of the band holds the associate degrees.
            if band(cum[1], cum[2]) >= 1.0 - e.associate.at(x.y) { Level::Associate } else { Level::SomeCollege }
        } else if u < cum[3] {
            Level::Bachelor
        } else {
            let g: [f64; 3] = e.graduate.at(x.y);
            let f = band(cum[3], 1.0) * (g[0] + g[1] + g[2]);
            if f < g[0] {
                Level::Master
            } else if f < g[0] + g[1] {
                Level::Professional
            } else {
                Level::Doctorate
            }
        }
    }

    /// `x`'s final level (reached if they live long enough).
    pub fn education_level(&self, x: Pid) -> Level {
        self.level_of(x, self.education_latent(x))
    }

    /// `x`'s schooling dates.
    pub fn education_path(&self, x: Pid) -> EducationPath {
        let t = &self.p.education.timing;
        let z = self.education_latent(x);
        let level = self.level_of(x, z);
        let k = self.pkey(x, T_TIMING);
        let birth = self.birth(x);
        let y = |s: f64| (s * YEAR) as i64;
        // Grade 1: the first September by which the child is old enough.
        let ready = birth + y(t.first_grade_age.at(x.y));
        let mut e1 = year_of(ready);
        if september(e1) < ready {
            e1 += 1;
        }
        let grade1 = september(e1);
        let kindergarten = (k.with(1).unit() < t.kindergarten.at(x.y)).then(|| september(e1 - 1));
        let (school_end, grades) = if level == Level::LessThanHighSchool {
            // Leaving at the era's age, give or take its spread.
            let leave = birth + y(t.dropout_age.at(x.y) + t.dropout_spread * (2.0 * k.with(2).unit() - 1.0));
            let leave = leave.max(grade1 + y(1.0));
            let grades = ((leave - grade1) as f64 / YEAR).floor().clamp(0.0, 11.0) as u8;
            (june(year_of(grade1) + grades as i32), grades)
        } else {
            (june(e1 + 12), 12)
        };
        let after = |start: i64, span: f64| start + y(span);
        let college = if level >= Level::SomeCollege {
            let on_time = k.with(3).unit() < t.college_on_time;
            let start_year = year_of(school_end) + if on_time { 0 } else { 1 + (-ln(1.0 - k.with(4).unit()) * t.late_start_years) as i32 };
            let start = september(start_year);
            let span = match level {
                Level::SomeCollege => t.some_college_years.at(k.with(5).unit()),
                Level::Associate => t.associate_years.at(k.with(5).unit()),
                _ => t.bachelor_years.at(k.with(5).unit()),
            };
            // Degrees come in June; leaving any time.
            let end = if level == Level::SomeCollege { after(start, span) } else { june(year_of(after(start, span - 0.25))) };
            Some((start, end.max(start + 90 * DAY)))
        } else {
            None
        };
        let graduate = match (level >= Level::Master, college) {
            (true, Some((_, ba))) => {
                let gap = -ln(1.0 - k.with(6).unit()) * t.graduate_gap_years;
                let start = september(year_of(ba) + gap as i32);
                let span = match level {
                    Level::Master => t.master_years.at(k.with(7).unit()),
                    Level::Professional => t.professional_years.at(k.with(7).unit()),
                    _ => t.doctorate_years.at(k.with(7).unit()),
                };
                Some((start, june(year_of(after(start, span - 0.25))).max(start + 300 * DAY)))
            }
            _ => None,
        };
        EducationPath { level, latent: z, kindergarten, grade1, school_end, grades, college, graduate }
    }

    /// What `x` is doing in school at `t` (`Out` after death).
    pub fn schooling_at(&self, x: Pid, t: i64) -> Schooling {
        self.schooling_in(&self.education_path(x), x, t)
    }

    fn schooling_in(&self, p: &EducationPath, x: Pid, t: i64) -> Schooling {
        if t < self.birth(x) || t >= self.death(x) {
            return Schooling::Out;
        }
        let school_year = |start: i64| -> u8 { (((t - start) as f64 / YEAR).floor() as i64 + 1).clamp(1, 255) as u8 };
        if t < p.grade1 {
            return match p.kindergarten {
                Some(k) if t >= k && t < june(year_of(k) + 1) => Schooling::Kindergarten,
                _ => Schooling::NotYet,
            };
        }
        if t < p.school_end {
            // Summers count with the grade just finished's next one.
            return Schooling::School { grade: school_year(p.grade1).min(12) };
        }
        if let Some((s, e)) = p.college {
            if t >= s && t < e {
                let toward = if p.level >= Level::Bachelor { Level::Bachelor } else { p.level };
                return Schooling::College { year: school_year(s), toward };
            }
        }
        if let Some((s, e)) = p.graduate {
            if t >= s && t < e {
                return Schooling::Graduate { year: school_year(s), toward: p.level };
            }
        }
        Schooling::Out
    }

    /// The highest level `x` has completed by `t`, if any (leaving school
    /// without a diploma counts as less than high school).
    pub fn education_at(&self, x: Pid, t: i64) -> Option<Level> {
        let p = self.education_path(x);
        let t = t.min(self.death(x) - 1);
        if let Some((_, e)) = p.graduate {
            if t >= e {
                return Some(p.level);
            }
        }
        if let Some((_, e)) = p.college {
            if t >= e {
                return Some(match p.level {
                    Level::SomeCollege | Level::Associate => p.level,
                    _ => Level::Bachelor,
                });
            }
            if t >= p.school_end {
                return Some(Level::HighSchool);
            }
        }
        if t >= p.school_end {
            return Some(if p.level == Level::LessThanHighSchool { Level::LessThanHighSchool } else { Level::HighSchool });
        }
        None
    }
}
