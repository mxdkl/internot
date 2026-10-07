//! Work on the monotone world, over sampled people: a career is a run of
//! contiguous spells from 16 (or leaving school before 16) to death, ordered
//! and non-empty; jobs are numbered in order; students' part-time jobs fall
//! inside schooling and full-time jobs outside it; retirement (or keeping
//! house for good) comes last; `work_at` is the spell in force; K–12
//! teachers work at schools; the walk is deterministic. `MONO_MULT`
//! (default 0.003), `MONO_SEEDS` (default 42), `PACK` (default `us`).
use internot_society::mono::{Employer, Inactive, Institution, Mono, Pid, WorkSpell};
use internot_society::Params;
use procedural_core::key::Key;
use rayon::prelude::*;

const YEAR: i64 = 31_556_952;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003)
}

fn sample(m: &Mono, per: u64) -> Vec<Pid> {
    let mut xs = Vec::new();
    for c in 0..m.cells() {
        for y in (m.first_year()..=2010).step_by(3) {
            let n = m.cohort_n(c, y);
            for j in 0..n.min(per) {
                xs.push(Pid { cell: c, y, i: Key::from_seed(5).with3(c as u64, y as u64, j).below(n) });
            }
        }
    }
    xs
}

#[test]
fn careers_are_ordered_contiguous_and_consistent() {
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    for seed in seeds() {
        let m = Mono::new(&p, seed, mult());
        let xs = sample(&m, 30);
        let bad: Vec<String> = xs
            .par_iter()
            .flat_map_iter(|&x| {
                let mut e = Vec::new();
                let career = m.career(x);
                let (birth, death) = (m.birth(x), m.death(x));
                if format!("{career:?}") != format!("{:?}", m.career(x)) {
                    e.push(format!("{x:?}: the career is not deterministic"));
                }
                let path = m.education_path(x);
                let entry = path.school_end.max(birth + 14 * YEAR);
                let in_school = |t: i64| t < entry || path.college.is_some_and(|(a, b)| t >= a && t < b) || path.graduate.is_some_and(|(a, b)| t >= a && t < b);
                let mut last_end: Option<i64> = None;
                let mut ordinal = 0u16;
                for (i, s) in career.iter().enumerate() {
                    let (a, b) = s.span();
                    if a >= b {
                        e.push(format!("{x:?}: empty spell {s:?}"));
                    }
                    if i == 0 && (a < birth + 14 * YEAR - YEAR / 12 || a > birth + 16 * YEAR + 1) {
                        e.push(format!("{x:?}: the career starts at {:.2} years", (a - birth) as f64 / YEAR as f64));
                    }
                    if last_end.is_some_and(|l| l != a) {
                        e.push(format!("{x:?}: spell {i} does not start where the last ended"));
                    }
                    last_end = Some(b);
                    if b > death {
                        e.push(format!("{x:?}: spell {i} ends after death"));
                    }
                    match s {
                        WorkSpell::Employed(job) => {
                            if job.ordinal != ordinal {
                                e.push(format!("{x:?}: job {} out of order (expected {ordinal})", job.ordinal));
                            }
                            ordinal += 1;
                            if job.part_time != in_school(job.start) {
                                e.push(format!("{x:?}: job {} part time {} but in school {}", job.ordinal, job.part_time, in_school(job.start)));
                            }
                            if !(job.pay_2023.is_finite() && job.pay_2023 > 0.0) {
                                e.push(format!("{x:?}: job {} pays {}", job.ordinal, job.pay_2023));
                            }
                            if m.job_title(job).is_empty() {
                                e.push(format!("{x:?}: job {} has no title", job.ordinal));
                            }
                        }
                        WorkSpell::OutOfLaborForce { why: Inactive::School, .. } if !in_school(a) => e.push(format!("{x:?}: out for school outside schooling")),
                        WorkSpell::Retired { .. } if i + 1 != career.len() => e.push(format!("{x:?}: retired before the end")),
                        _ => {}
                    }
                }
                // Once the career starts it runs to death.
                if let Some(l) = last_end {
                    if l != death {
                        e.push(format!("{x:?}: the career ends {:.2} years before death", (death - l) as f64 / YEAR as f64));
                    }
                }
                // work_at is the spell in force.
                for age in (10..100).step_by(7) {
                    let t = birth + age * YEAR;
                    let want = career.iter().find(|s| s.span().0 <= t && t < s.span().1).map(|s| format!("{s:?}"));
                    let got = m.work_at(x, t).map(|s| format!("{s:?}"));
                    if t < death && want != got {
                        e.push(format!("{x:?}: work_at at {age} is {got:?}, the career says {want:?}"));
                    }
                }
                e
            })
            .collect();
        assert!(bad.is_empty(), "seed {seed}: {} problems, e.g. {:#?}", bad.len(), &bad[..bad.len().min(10)]);
    }
}

#[test]
fn employers_resolve_and_teachers_teach_at_schools() {
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    let m = Mono::new(&p, 42, mult());
    let xs = sample(&m, 4);
    let (mut teachers, mut jobs) = (0, 0);
    for &x in &xs {
        for s in m.career(x) {
            let WorkSpell::Employed(job) = s else { continue };
            jobs += 1;
            let e = m.employer(x, &job);
            let info = m.employer_info(e);
            assert!(!info.name.is_empty() && !info.city.is_empty(), "{x:?}: employer {e:?} has no name or place: {info:?}");
            assert!(!info.name.contains('{'), "{x:?}: unfilled pattern in {:?}", info.name);
            if m.occupation_info(job.occupation).1.starts_with("2520") && m.address_of(x, job.start).is_some() {
                teachers += 1;
                assert!(matches!(e, Employer::Institution(Institution::School(_))), "{x:?}: a teacher works at {e:?}");
            }
        }
    }
    assert!(jobs > 300, "only {jobs} jobs sampled");
    assert!(teachers > 0, "no K-12 teachers sampled");
}
