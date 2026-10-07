//! Work (L6) on the monotone world: careers as dated spells (jobs,
//! unemployment, time out of the labour force, retirement), each job's
//! title, occupation, industry, employer and pay, as pure functions of
//! `(seed, id, t)`. Pack `work.ron`; data `data/work.bin` (ACS 2023 PUMS and
//! the Census occupation and industry indexes,
//! `internot_society/data/distill_work.py`); research
//! `research/2026-09-29-work-and-organizations.md`.
//!
//! - **The career** runs from 16 (or leaving school before 16) to death.
//!   Windows come first: schooling (high school, college, graduate school),
//!   keeping house (some women before their first union, and after a union
//!   or birth, returning with the cohort's share when the youngest child is
//!   about school age) and disability (a hazard by age and era, with
//!   returns). Each stretch takes its highest-priority window or work; work
//!   stretches walk jobs of lognormal length by the age at their start, then
//!   the next job directly, a search or a break; school stretches alternate
//!   part-time jobs with time out at the year's student share. Retirement at
//!   an age drawn from the cohort's survival curve of working. Every draw is
//!   keyed on the person and a stretch or job ordinal.
//! - **Occupations** come from the ACS 2023 shares by sex, age band and
//!   education, each SOC major group scaled by its era factor; a job change
//!   keeps the occupation unless the person switches. The industry follows
//!   the occupation's industries; the title is drawn among the occupation's
//!   Census index titles allowed in that industry (by ACS write-in
//!   frequency) and kept across jobs where it fits.
//! - **Employers** (on demand, [`Mono::employer`]: they need the home):
//!   K–12 teachers and school staff at a real school near home, college
//!   staff at a real college, the self-employed at their own business,
//!   government workers at a government; everyone else at an establishment
//!   of the industry in the home county (or another of its commuting zone),
//!   by size class. An establishment's line of business and name are
//!   functions of the establishment alone (the pack's name patterns).
//! - **Pay:** the occupation's median wage times an experience curve, the
//!   real-wage index of the year and a person and job effect; nominal by the
//!   CPI.
//!
//! Debt: no coworker rosters until places have counted indexes
//! (`research/2026-10-04-areas-and-rosters.md`); establishments have no
//! open or close dates; lines of business are uniform within an industry;
//! part-time work only for students.

use procedural_core::dmath::exp;
use procedural_core::key::Key;
use procedural_core::sample::{exp1_by_inversion, std_normal};
use procedural_core::stream::{hazard_time, year_of};

use super::residence::{Strings, AREA, COUNTY, ZONE};
use super::schools::Institution;
use super::{Mono, Pid, YEAR};
use crate::params::Sex;

const T_CAREER: u64 = 100;
const T_EMPLOYER: u64 = 101;
const T_NAME: u64 = 102;

/// The occupation tables.
pub(super) struct Occupations {
    industry_code: Strings,
    industry_title: Strings,
    /// Each industry's lines of business (`line_at[industry]..`), with
    /// their kinds: 0 an activity, 1 a place, 2 a product.
    lines: Strings,
    line_kind: Vec<u8>,
    line_at: Vec<u32>,
    code: Strings,
    title: Strings,
    group: Vec<u8>,
    wage: Vec<f32>,
    wage_sd: Vec<f32>,
    self_share: Vec<f32>,
    gov_share: Vec<f32>,
    ind_at: Vec<u32>,
    inds: Vec<(u32, u16)>,
    /// `holders[(sex · 3 + band) · 8 + level]`: (occupation, weight).
    holders: Vec<Vec<(u32, u32)>>,
    /// National employment share of each industry (from the holders and
    /// the occupations' industries).
    industry_share: Vec<f64>,
    /// Job titles of each occupation (`title_at[o]..`), their weights and
    /// the industries each is restricted to (`res_at[title]..`; none: any).
    titles: Strings,
    title_w: Vec<u16>,
    title_at: Vec<u32>,
    res: Vec<u32>,
    res_at: Vec<u32>,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("work data is truncated")?;
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
                return Err("work data: bad varint".into());
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

impl Occupations {
    pub(super) fn from_data(b: &[u8]) -> Result<Self, String> {
        let mut r = Reader { b, i: 0 };
        if r.take(8)? != b"INTWORK1" || r.byte()? != 2 {
            return Err("not a version-2 work file".into());
        }
        let m = r.leb()? as usize;
        let (mut industry_code, mut industry_title, mut lines) = (Strings::default(), Strings::default(), Strings::default());
        let (mut line_kind, mut line_at) = (vec![], vec![0u32]);
        for _ in 0..m {
            industry_code.push(&r.text()?);
            industry_title.push(&r.text()?);
            for _ in 0..r.leb()? {
                lines.push(&r.text()?);
                line_kind.push(r.byte()?);
            }
            line_at.push(line_kind.len() as u32);
        }
        let n = r.leb()? as usize;
        let (mut code, mut title) = (Strings::default(), Strings::default());
        let (mut group, mut wage, mut wage_sd, mut self_share, mut gov_share, mut ind_at, mut inds) = (vec![], vec![], vec![], vec![], vec![], vec![0u32], vec![]);
        let (mut titles, mut title_w, mut title_at, mut res, mut res_at) = (Strings::default(), vec![], vec![0u32], vec![], vec![0u32]);
        for _ in 0..n {
            let c = r.text()?;
            group.push(c.get(..2).and_then(|g| g.parse().ok()).unwrap_or(0));
            code.push(&c);
            title.push(&r.text()?);
            wage.push(r.f32()?);
            wage_sd.push(r.f32()?);
            self_share.push(r.f32()?);
            gov_share.push(r.f32()?);
            let k = r.byte()? as usize;
            for _ in 0..k {
                let ix = r.leb()? as u32;
                let sh = r.take(2)?;
                inds.push((ix, u16::from_le_bytes([sh[0], sh[1]])));
            }
            ind_at.push(inds.len() as u32);
            for _ in 0..r.leb()? {
                titles.push(&r.text()?);
                let w = r.take(2)?;
                title_w.push(u16::from_le_bytes([w[0], w[1]]));
                for _ in 0..r.leb()? {
                    res.push(r.leb()? as u32);
                }
                res_at.push(res.len() as u32);
            }
            title_at.push(title_w.len() as u32);
        }
        let mut holders = Vec::with_capacity(48);
        for _ in 0..48 {
            let c = r.leb()? as usize;
            let mut v = Vec::with_capacity(c);
            for _ in 0..c {
                let o = r.leb()? as u32;
                let w = u32::from_le_bytes(r.take(4)?.try_into().unwrap());
                v.push((o, w));
            }
            holders.push(v);
        }
        // Industry shares: every occupation's national weight over its
        // listed industries.
        let mut occ_w = vec![0f64; n];
        for t in &holders {
            for &(o, w) in t {
                occ_w[o as usize] += w as f64;
            }
        }
        let mut industry_share = vec![0f64; m];
        for o in 0..n {
            for &(ix, sh) in &inds[ind_at[o] as usize..ind_at[o + 1] as usize] {
                industry_share[ix as usize] += occ_w[o] * sh as f64 / 65535.0;
            }
        }
        let total: f64 = industry_share.iter().sum();
        for s in &mut industry_share {
            *s /= total.max(1.0);
        }
        Ok(Occupations { industry_code, industry_title, lines, line_kind, line_at, code, title, group, wage, wage_sd, self_share, gov_share, ind_at, inds, holders, industry_share, titles, title_w, title_at, res, res_at })
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.industry_code.heap_bytes()
            + self.industry_title.heap_bytes()
            + self.lines.heap_bytes()
            + self.line_kind.len() * 5
            + self.code.heap_bytes()
            + self.title.heap_bytes()
            + self.wage.len() * 17
            + self.inds.len() * 8
            + self.holders.iter().map(|h| h.len() * 8).sum::<usize>()
            + self.industry_share.len() * 8
            + self.titles.heap_bytes()
            + self.title_w.len() * 10
            + self.res.len() * 4
    }

    /// Whether title `t` may be used in industry `ind`.
    fn title_fits(&self, t: usize, ind: u32) -> bool {
        let r = &self.res[self.res_at[t] as usize..self.res_at[t + 1] as usize];
        r.is_empty() || r.contains(&ind)
    }

    /// A job title for occupation `o` in industry `ind`, by weight among
    /// the titles allowed there.
    fn draw_title(&self, o: u32, ind: u32, u: f64) -> u32 {
        let (a, b) = (self.title_at[o as usize] as usize, self.title_at[o as usize + 1] as usize);
        let total: u64 = (a..b).filter(|&t| self.title_fits(t, ind)).map(|t| self.title_w[t] as u64).sum();
        let mut v = (u * total as f64) as u64;
        for t in a..b {
            if self.title_fits(t, ind) {
                let w = self.title_w[t] as u64;
                if v < w {
                    return t as u32;
                }
                v -= w;
            }
        }
        a as u32
    }

    /// An occupation drawn for (sex, age band, level), by the ACS shares
    /// times the era factors `factor[group]` of the year.
    fn draw(&self, female: bool, band: usize, level: usize, factor: &[f64; 64], u: f64) -> u32 {
        let t = &self.holders[((!female) as usize * 3 + band) * 8 + level];
        let t = if t.is_empty() { &self.holders[((!female) as usize * 3 + 1) * 8 + level] } else { t };
        let f = |o: u32| factor[self.group[o as usize] as usize & 63];
        let total: f64 = t.iter().map(|&(o, w)| w as f64 * f(o)).sum();
        let mut v = u * total;
        for &(o, w) in t {
            v -= w as f64 * f(o);
            if v < 0.0 {
                return o;
            }
        }
        t.last().map_or(0, |e| e.0)
    }

    /// An industry of occupation `o`, by its industries' shares.
    fn industry(&self, o: u32, u: f64) -> u32 {
        let row = &self.inds[self.ind_at[o as usize] as usize..self.ind_at[o as usize + 1] as usize];
        let total: f64 = row.iter().map(|e| e.1 as f64).sum();
        let mut v = u * total;
        for &(ix, sh) in row {
            v -= sh as f64;
            if v < 0.0 {
                return ix;
            }
        }
        row.last().map_or(0, |e| e.0)
    }
}

/// Who employs a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Employer {
    /// A procedural establishment: county node, industry, size class and
    /// index among the class's establishments there.
    Establishment { county: u32, industry: u32, class: u8, index: u32 },
    /// A school or college.
    Institution(Institution),
    /// Their own business (keyed by the owner and the job).
    SelfEmployed { owner: Pid, job: u16 },
    /// A government: 0 county, 1 state, 2 federal, in the county node.
    Government { level: u8, county: u32 },
}

/// A job.
#[derive(Clone, Copy, Debug)]
pub struct Job {
    /// Its ordinal in the career.
    pub ordinal: u16,
    pub occupation: u32,
    /// The job title (an index into the occupation's titles; see
    /// [`Mono::job_title`]).
    pub title: u32,
    pub industry: u32,
    pub start: i64,
    pub end: i64,
    /// Annual pay at the start, in 2023 dollars (at the job's hours).
    pub pay_2023: f64,
    /// A student's part-time job.
    pub part_time: bool,
}

/// Why someone is out of the labour force.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Inactive {
    /// In school and not working.
    School,
    /// Keeping house: before a first union, or with a partner or children.
    Family,
    /// Disability or ill health.
    Disability,
    /// A break between jobs.
    Other,
}

/// A stretch of a working life.
#[derive(Clone, Copy, Debug)]
pub enum WorkSpell {
    Employed(Job),
    Unemployed { from: i64, to: i64 },
    OutOfLaborForce { from: i64, to: i64, why: Inactive },
    Retired { from: i64, to: i64 },
}

impl WorkSpell {
    pub fn span(&self) -> (i64, i64) {
        match *self {
            WorkSpell::Employed(j) => (j.start, j.end),
            WorkSpell::Unemployed { from, to } | WorkSpell::OutOfLaborForce { from, to, .. } | WorkSpell::Retired { from, to } => (from, to),
        }
    }

    /// Working or looking for work.
    pub fn in_labor_force(&self) -> bool {
        matches!(self, WorkSpell::Employed(_) | WorkSpell::Unemployed { .. })
    }
}

/// A stretch of the career timeline, by priority (a later variant wins
/// where windows overlap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Seg {
    Work,
    Disability,
    Family,
    /// In school, with the education level whose young workers' jobs a
    /// student takes.
    School(u8),
}

/// Facts about an employer, as served.
#[derive(Clone, Debug)]
pub struct EmployerInfo {
    pub name: String,
    pub industry: String,
    /// What it does, from the Census industry index ("Pizza Parlor",
    /// "Coffee Tables" for a furniture maker), for businesses.
    pub line: Option<String>,
    pub city: String,
    pub state: &'static str,
}

impl<'a> Mono<'a> {
    pub(super) fn occupations(&self) -> &Occupations {
        self.occupations.get_or_init(|| Occupations::from_data(&self.p.work_data).expect("the pack's work data is valid"))
    }

    /// `x`'s career, from 16 (or leaving school before 16) to death: jobs
    /// (students' part-time ones included), unemployment, time out of the
    /// labour force and retirement, as ordered, contiguous spells.
    pub fn career(&self, x: Pid) -> Vec<WorkSpell> {
        let w = &self.p.work;
        let key = self.pkey(x, T_CAREER);
        let birth = self.birth(x);
        let death = self.death(x);
        let female = self.sex(x) == Sex::Female;
        let years = |s: f64| (s * YEAR) as i64;
        let path = self.education_path(x);
        let entry = path.school_end.max(birth + years(14.0));
        let begin = (birth + years(16.0)).min(entry);
        let retire = birth + years(w.retirement_age(female, x.y, key.with(1).unit()));
        let stop = retire.min(death);
        let mut out = Vec::new();
        if begin >= stop {
            return out;
        }
        // Windows: schooling, family and disability.
        let mut win: Vec<(i64, i64, Seg)> = Vec::with_capacity(8);
        if entry > begin {
            win.push((begin, entry, Seg::School(0)));
        }
        if let Some((a, b)) = path.college {
            win.push((a, b, Seg::School(2)));
        }
        if let Some((a, b)) = path.graduate {
            win.push((a, b, Seg::School(4)));
        }
        let single = female && key.with(12).unit() < w.homemaker_single.at(x.y);
        let partnered = female && key.with(2).unit() < w.homemaker.at(x.y);
        if single || partnered {
            let kids = self.children(x);
            let first_birth = kids.iter().map(|&c| self.birth(c)).min();
            let last_birth = kids.iter().map(|&c| self.birth(c)).max();
            let start = [first_birth, self.union_of(x).map(|u| u.start)].into_iter().flatten().min();
            if single {
                win.push((entry, start.unwrap_or(i64::MAX), Seg::Family));
            }
            if let (true, Some(s)) = (partnered, start) {
                let s = s.max(entry);
                let back = key.with(3).unit() < w.homemaker_returns.at(x.y);
                let end = if back { last_birth.map_or(s + years(3.0), |b| b + years(w.return_child_age)).max(s + years(1.0)) } else { i64::MAX };
                win.push((s, end, Seg::Family));
            }
        }
        let d = &w.disability;
        let mut from = entry;
        for n in 0..4u64 {
            let e = exp1_by_inversion(key.with2(13, n).unit());
            let rate = |t: i64| {
                let a = ((t - birth) as f64 / YEAR) as i32;
                (d.hazard.at(a) * d.era.at(year_of(t)) / YEAR, birth + years((a + 1) as f64))
            };
            let Some(at) = hazard_time(e, from, stop, rate) else { break };
            let back = key.with2(14, n).unit() < d.returns;
            let end = if back { at + years(d.years.median * exp(d.years.sigma * std_normal(key.with2(15, n)))).max(1) } else { i64::MAX };
            win.push((at, end, Seg::Disability));
            if !back {
                break;
            }
            from = end;
        }
        // The timeline: each stretch takes the highest-priority window over
        // it, or work.
        let mut cuts: Vec<i64> = Vec::with_capacity(2 + 2 * win.len());
        cuts.extend([begin, stop]);
        for &(a, b, _) in &win {
            cuts.extend([a, b].into_iter().filter(|&c| c > begin && c < stop));
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut segs: Vec<(i64, i64, Seg)> = Vec::with_capacity(cuts.len());
        for c in cuts.windows(2) {
            let kind = win.iter().filter(|v| v.0 <= c[0] && c[0] < v.1).map(|v| v.2).max().unwrap_or(Seg::Work);
            match segs.last_mut() {
                Some(l) if l.2 == kind => l.1 = c[1],
                _ => segs.push((c[0], c[1], kind)),
            }
        }

        let occ = self.occupations();
        let level = self.education_level(x) as usize;
        let factors = |y: i32| -> [f64; 64] {
            let mut f = [1.0; 64];
            for e in &w.era_factors {
                f[e.group as usize & 63] = e.factor.at(y);
            }
            f
        };
        let person = std_normal(key.with(7));
        let pay = |o: u32, experience: f64, t: i64, k: Key| {
            let e = &w.experience;
            let lift = e.start + e.per_year * experience.min(e.peak_years);
            // A person effect and a job effect (AKM: most of the variance
            // within an occupation is the person's).
            let z = 0.9 * person + 0.436 * std_normal(k);
            occ.wage[o as usize] as f64 * exp(lift + occ.wage_sd[o as usize] as f64 * 0.8 * z) * w.real_wage.at(year_of(t))
        };
        let search = |k: Key| (w.unemployment_weeks.median * exp(w.unemployment_weeks.sigma * std_normal(k)) * 7.0 * 86_400.0) as i64;
        let mut k: u16 = 0;
        let mut occupation: Option<(u32, usize)> = None;
        let mut student: Option<u32> = None;
        let mut last_title: Option<(u32, u32)> = None;
        let mut experience = 0.0f64;
        for (si, &(a, b, kind)) in segs.iter().enumerate() {
            let sk = key.with2(16, si as u64);
            match kind {
                Seg::Disability => out.push(WorkSpell::OutOfLaborForce { from: a, to: b, why: Inactive::Disability }),
                Seg::Family => out.push(WorkSpell::OutOfLaborForce { from: a, to: b, why: Inactive::Family }),
                Seg::School(lv) => {
                    // Part-time jobs alternating with time out, so the share
                    // working is the year's.
                    let s = &w.students;
                    let p = s.share.at(year_of(a)).clamp(0.01, 0.99);
                    let mut on = sk.unit() < p;
                    let mut t = a;
                    let mut j = 0u64;
                    while t < b {
                        let jk = sk.with2(1, j);
                        let median = if on { s.job_years.median } else { s.job_years.median * (1.0 - p) / p };
                        let end = (t + years(median * exp(s.job_years.sigma * std_normal(jk.with(1)))).max(7 * 86_400)).min(b);
                        if on {
                            let o = match student {
                                Some(o) if jk.with(2).unit() < 0.5 => o,
                                _ => occ.draw(female, 0, lv as usize, &factors(year_of(t)), jk.with(3).unit()),
                            };
                            let industry = occ.industry(o, jk.with(4).unit());
                            let title = match last_title {
                                Some((lo, lt)) if student == Some(o) && lo == o && occ.title_fits(lt as usize, industry) && jk.with(9).unit() < 0.8 => lt,
                                _ => occ.draw_title(o, industry, jk.with(10).unit()),
                            };
                            student = Some(o);
                            last_title = Some((o, title));
                            let pay_2023 = pay(o, experience, t, jk.with(6)) * s.hours;
                            out.push(WorkSpell::Employed(Job { ordinal: k, occupation: o, title, industry, start: t, end, pay_2023, part_time: true }));
                            experience += s.hours * (end - t) as f64 / YEAR;
                            k += 1;
                        } else {
                            out.push(WorkSpell::OutOfLaborForce { from: t, to: end, why: Inactive::School });
                        }
                        t = end;
                        on = !on;
                        j += 1;
                    }
                }
                Seg::Work => {
                    let mut t = a;
                    // A search first, unless a job is lined up.
                    if sk.with(1).unit() > w.direct {
                        let to = (t + search(sk.with(2))).min(b);
                        out.push(WorkSpell::Unemployed { from: t, to });
                        t = to;
                    }
                    while t < b {
                        let jk = key.with3(6, k as u64, 0);
                        let age = (t - birth) as f64 / YEAR;
                        let (mu, sd) = w.job_law(age);
                        let end = (t + years(exp(mu + sd * std_normal(jk.with(1)))).max(7 * 86_400)).min(b);
                        let band = if age < 25.0 { 0 } else if age < 65.0 { 1 } else { 2 };
                        // The occupation: kept, unless switching (or settling
                        // after 25).
                        let keep = match occupation {
                            Some((_, ob)) if ob == 0 && band > 0 => jk.with(2).unit() < 0.4,
                            Some(_) => jk.with(2).unit() >= w.switch_occupation,
                            None => false,
                        };
                        let o = match (keep, occupation) {
                            (true, Some((o, _))) => o,
                            _ => occ.draw(female, band, level, &factors(year_of(t)), jk.with(3).unit()),
                        };
                        occupation = Some((o, band));
                        let industry = occ.industry(o, jk.with(4).unit());
                        // The title: kept with the occupation where it fits.
                        let title = match last_title {
                            Some((lo, lt)) if lo == o && occ.title_fits(lt as usize, industry) && jk.with(9).unit() < 0.8 => lt,
                            _ => occ.draw_title(o, industry, jk.with(10).unit()),
                        };
                        last_title = Some((o, title));
                        let pay_2023 = pay(o, experience, t, jk.with(6));
                        out.push(WorkSpell::Employed(Job { ordinal: k, occupation: o, title, industry, start: t, end, pay_2023, part_time: false }));
                        experience += (end - t) as f64 / YEAR;
                        k += 1;
                        t = end;
                        if t >= b {
                            break;
                        }
                        // Next: straight to another job, a break, or a
                        // search.
                        let u = jk.with(7).unit();
                        if u < w.direct {
                            continue;
                        }
                        let (to, spell) = if u < w.direct + w.break_share {
                            let to = (t + years(w.break_years.median * exp(w.break_years.sigma * std_normal(jk.with(8))))).min(b);
                            (to, WorkSpell::OutOfLaborForce { from: t, to, why: Inactive::Other })
                        } else {
                            let to = (t + search(jk.with(8))).min(b);
                            (to, WorkSpell::Unemployed { from: t, to })
                        };
                        if to > t {
                            out.push(spell);
                        }
                        t = to;
                    }
                }
            }
        }
        if retire < death {
            // Keeping house for good stays that; anyone else retires.
            let homemaker = win.iter().any(|v| v.2 == Seg::Family && v.1 == i64::MAX && v.0 < stop);
            out.push(if homemaker && segs.last().is_some_and(|s| s.2 == Seg::Family) {
                WorkSpell::OutOfLaborForce { from: retire, to: death, why: Inactive::Family }
            } else {
                WorkSpell::Retired { from: retire, to: death }
            });
        }
        out
    }

    /// What `x` is doing at `t`: the career spell then (`None`: before
    /// entering the labour force, or not alive).
    pub fn work_at(&self, x: Pid, t: i64) -> Option<WorkSpell> {
        if !self.alive_at(x, t) {
            return None;
        }
        self.career(x).into_iter().find(|s| {
            let (a, b) = s.span();
            a <= t && t < b
        })
    }

    /// The world's people alive per person of the places' weights in
    /// `year`'s decade (the instant bounds' midpoint at mid-decade over the
    /// areas' total weight): 1 at the real population's scale.
    pub fn sample_share(&self, year: i32) -> f64 {
        const FIRST: i32 = 1800;
        const LAST: i32 = 2100;
        let shares = self.sample_share.get_or_init(|| {
            let places = self.places();
            (FIRST / 10..=LAST / 10)
                .map(|d| {
                    let y = d * 10 + 5;
                    let total: f64 = (0..places.count(AREA) as u32).map(|a| places.weight(AREA, a, y)).sum();
                    let (lo, hi) = self.alive_bounds(procedural_core::stream::year_start(y));
                    if total > 0.0 && hi > 0 { (lo + hi) as f64 / 2.0 / total } else { 0.0 }
                })
                .collect()
        });
        let d = (year.clamp(FIRST, LAST) / 10 - FIRST / 10) as usize;
        // Decades before the world's first people borrow the first that has any.
        shares[d..].iter().copied().find(|&s| s > 0.0).unwrap_or(1.0)
    }

    /// Who employs `x`'s `job`: a function of the job and of where `x`
    /// lives when it starts (so computed on demand, not in the walk).
    pub fn employer(&self, x: Pid, job: &Job) -> Employer {
        self.employer_for(x, job.ordinal, job.occupation, job.industry, job.start, self.pkey(x, T_CAREER).with2(17, job.ordinal as u64))
    }

    /// The employer of `x`'s job `k` (occupation `o`, industry `ind`)
    /// starting at `t`.
    fn employer_for(&self, x: Pid, k: u16, o: u32, ind: u32, t: i64, key: Key) -> Employer {
        let occ = self.occupations();
        let code = occ.code.get(o as usize);
        let home = self.address_of(x, t).or_else(|| self.address_of(x, self.birth(x) + 1));
        // Teachers, and everyone else working for schools and colleges, at
        // a real school or college near home.
        let industry = occ.industry_code.get(ind as usize);
        let school = code.starts_with("2520") || industry == "6111";
        let college = code.starts_with("2510") || industry == "611M1";
        if school || college {
            if let Some((lat, lon, state)) = self.home_point(x, t) {
                let inst = self.institutions();
                if college {
                    let c = &self.p.education.college;
                    if let Some(i) = inst.draw_college(lat, lon, state, None, false, c.in_state, c.miles_scale, c.decay, key.with(9)) {
                        return Employer::Institution(Institution::College(i));
                    }
                } else {
                    let grade = match code {
                        "252011" | "252012" => 0,
                        "252021" => 3,
                        "252022" => 7,
                        "252031" | "252032" => 10,
                        _ => 1 + key.with(10).below(12) as u8,
                    };
                    let private = key.with(11).unit() < self.p.education.private_share.at(year_of(t));
                    if let Some(s) = inst.nearest_school(lat, lon, grade, private as u8).or_else(|| inst.nearest_school(lat, lon, grade, 0)) {
                        return Employer::Institution(Institution::School(s));
                    }
                }
            }
        }
        let county = match home {
            Some(pos) => {
                if key.with(1).unit() < self.p.work.other_county {
                    // Another county of the zone, by weight.
                    self.places().draw_child(COUNTY, pos[ZONE], year_of(t), key.with(2))
                } else {
                    pos[COUNTY]
                }
            }
            None => 0,
        };
        if key.with(3).unit() < occ.self_share[o as usize] as f64 {
            return Employer::SelfEmployed { owner: x, job: k };
        }
        if key.with(4).unit() < occ.gov_share[o as usize] as f64 {
            let u = key.with(5).unit();
            return Employer::Government { level: if u < 0.6 { 0 } else if u < 0.85 { 1 } else { 2 }, county };
        }
        // A size class by employment share, then one of its establishments
        // here (as many as the county's workers in the industry fill).
        let w = &self.p.work;
        let mut v = key.with(6).unit();
        let mut class = w.size_classes.len() - 1;
        for (i, c) in w.size_classes.iter().enumerate() {
            v -= c.share;
            if v < 0.0 {
                class = i;
                break;
            }
        }
        let c = w.size_classes[class];
        // The places weigh the real population; the world is a sample of it.
        let workers = self.places().weight(COUNTY, county, year_of(t)) * self.sample_share(year_of(t)) * 0.45 * occ.industry_share[ind as usize];
        let n = ((workers * c.share / c.mean).round() as u64).max(1);
        Employer::Establishment { county, industry: ind, class: class as u8, index: key.with(7).below(n) as u32 }
    }

    /// Facts about an employer.
    pub fn employer_info(&self, e: Employer) -> EmployerInfo {
        let occ = self.occupations();
        let places = self.places();
        let county_place = |c: u32| -> (String, &'static str) {
            let (name, fips) = places.county(c);
            (name.trim_end_matches(" County").trim_end_matches(" Parish").trim_end_matches(" Borough").to_string(), super::state_abbr(fips / 1000))
        };
        let national_surname = |k: Key| -> String {
            self.surname_table(None).map(|t| title_case(&self.name_data().surnames[t.draw(k) as usize])).unwrap_or_else(|| "Smith".into())
        };
        let line_of = |industry: u32, k: Key| -> Option<(&str, u8)> {
            let (a, b) = (occ.line_at[industry as usize], occ.line_at[industry as usize + 1]);
            (b > a).then(|| {
                let l = a + k.below((b - a) as u64) as u32;
                (occ.lines.get(l as usize), occ.line_kind[l as usize])
            })
        };
        match e {
            Employer::Institution(i) => {
                let info = self.institution(i);
                let industry = match i {
                    Institution::School(_) => "Elementary and Secondary Schools",
                    Institution::College(_) => "Colleges, Universities, and Professional Schools",
                };
                EmployerInfo { name: info.name, industry: industry.into(), line: None, city: info.city, state: info.state }
            }
            Employer::Government { level, county } => {
                let (place, state) = county_place(county);
                let (name, industry) = match level {
                    0 => (format!("{place} County Government"), "Local Government"),
                    1 => (format!("State of {}", state_name(state)), "State Government"),
                    _ => ("United States Government".to_string(), "Federal Government"),
                };
                EmployerInfo { name, industry: industry.into(), line: None, city: place, state }
            }
            Employer::SelfEmployed { owner, job } => {
                let surname = self.surname_text(self.birth_surname(owner));
                let k = self.pkey(owner, T_NAME).with(job as u64);
                let (industry, county) = self
                    .career(owner)
                    .iter()
                    .find_map(|s| match s {
                        WorkSpell::Employed(j) if j.ordinal == job => Some((j.industry, self.address_of(owner, j.start).map_or(0, |p| p[COUNTY]))),
                        _ => None,
                    })
                    .unwrap_or((0, 0));
                let (place, state) = county_place(county);
                let line = line_of(industry, k.with(5));
                let name = business_name(&self.p.work.names, occ.industry_code.get(industry as usize), &surname, &national_surname(k.with(2)), &place, line, 0, k);
                EmployerInfo { name, industry: occ.industry_title.get(industry as usize).to_string(), line: line.map(|l| l.0.to_string()), city: place, state }
            }
            Employer::Establishment { county, industry, class, index } => {
                let (place, state) = county_place(county);
                let k = self.cell(0).key.with3(T_EMPLOYER, ((county as u64) << 20) | industry as u64, ((class as u64) << 32) | index as u64);
                let line = line_of(industry, k.with(5));
                let name = business_name(&self.p.work.names, occ.industry_code.get(industry as usize), &national_surname(k.with(1)), &national_surname(k.with(2)), &place, line, class, k);
                EmployerInfo { name, industry: occ.industry_title.get(industry as usize).to_string(), line: line.map(|l| l.0.to_string()), city: place, state }
            }
        }
    }

    /// An occupation's ACS name and SOC code.
    pub fn occupation_info(&self, o: u32) -> (&str, &str) {
        let occ = self.occupations();
        (occ.title.get(o as usize), occ.code.get(o as usize))
    }

    /// A job's title.
    pub fn job_title(&self, job: &Job) -> &str {
        self.occupations().titles.get(job.title as usize)
    }

    /// The employer's place: the county node a job is in (for distances).
    pub fn employer_county(&self, e: Employer) -> Option<u32> {
        match e {
            Employer::Establishment { county, .. } | Employer::Government { county, .. } => Some(county),
            _ => None,
        }
    }

    /// Annual pay of a job at `t` (2023 dollars), with raises of the
    /// experience curve inside the job.
    pub fn pay_at(&self, job: &Job, t: i64) -> f64 {
        let w = &self.p.work;
        let years = ((t - job.start) as f64 / YEAR).max(0.0);
        job.pay_2023 * exp(w.experience.per_year * years.min(10.0)) * w.real_wage.at(year_of(t)) / w.real_wage.at(year_of(job.start))
    }

    /// 2023 dollars at `year`'s prices.
    pub fn nominal(&self, dollars_2023: f64, year: i32) -> f64 {
        let cpi = &self.p.work.cpi;
        dollars_2023 * cpi.at(year) / cpi.at(2023)
    }
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn state_name(abbr: &str) -> &'static str {
    const S: [(&str, &str); 51] = [
        ("AL", "Alabama"), ("AK", "Alaska"), ("AZ", "Arizona"), ("AR", "Arkansas"), ("CA", "California"), ("CO", "Colorado"), ("CT", "Connecticut"), ("DE", "Delaware"),
        ("DC", "the District of Columbia"), ("FL", "Florida"), ("GA", "Georgia"), ("HI", "Hawaii"), ("ID", "Idaho"), ("IL", "Illinois"), ("IN", "Indiana"), ("IA", "Iowa"),
        ("KS", "Kansas"), ("KY", "Kentucky"), ("LA", "Louisiana"), ("ME", "Maine"), ("MD", "Maryland"), ("MA", "Massachusetts"), ("MI", "Michigan"), ("MN", "Minnesota"),
        ("MS", "Mississippi"), ("MO", "Missouri"), ("MT", "Montana"), ("NE", "Nebraska"), ("NV", "Nevada"), ("NH", "New Hampshire"), ("NJ", "New Jersey"), ("NM", "New Mexico"),
        ("NY", "New York"), ("NC", "North Carolina"), ("ND", "North Dakota"), ("OH", "Ohio"), ("OK", "Oklahoma"), ("OR", "Oregon"), ("PA", "Pennsylvania"), ("RI", "Rhode Island"),
        ("SC", "South Carolina"), ("SD", "South Dakota"), ("TN", "Tennessee"), ("TX", "Texas"), ("UT", "Utah"), ("VT", "Vermont"), ("VA", "Virginia"), ("WA", "Washington"),
        ("WV", "West Virginia"), ("WI", "Wisconsin"), ("WY", "Wyoming"),
    ];
    S.iter().find(|s| s.0 == abbr).map_or("the State", |s| s.1)
}

/// A business name by the pack's patterns: after its line of business
/// when that names a place, else by its sector (an activity line such as
/// "Installation of Linoleum" never fills `{l}`).
#[allow(clippy::too_many_arguments)]
fn business_name(n: &crate::params::EmployerNames, code: &str, s: &str, t: &str, p: &str, line: Option<(&str, u8)>, class: u8, k: Key) -> String {
    let fill = |pat: &str| -> String {
        let x = &n.suffixes[k.with(4).below(n.suffixes.len() as u64) as usize];
        let out = pat.replace("{s}", s).replace("{t}", t).replace("{p}", p).replace("{l}", line.map_or("", |l| l.0)).replace("{x}", x);
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let pick = |pats: &[&String]| -> String { fill(pats[k.with(3).below(pats.len() as u64) as usize]) };
    if let Some((l, 1)) = line {
        if k.with(1).unit() < n.place_share {
            let last = l.rsplit(' ').next().unwrap_or(l).to_lowercase();
            let pats = if n.civic_nouns.contains(&last) { &n.civic } else { &n.place };
            return pick(&pats.iter().collect::<Vec<_>>());
        }
    }
    let sector = n.sector(code);
    let pats = if class >= n.large_from && !sector.large.is_empty() { &sector.large } else { &sector.small };
    let usable: Vec<&String> = pats.iter().filter(|q| !q.contains("{l}") || line.is_some_and(|l| l.1 != 0)).collect();
    if usable.is_empty() {
        return fill("{s} & Sons");
    }
    pick(&usable)
}
