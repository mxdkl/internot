//! The monotone world, exhaustively on a small world: every person's
//! kinship and union facts agree from every side, and closed-form counts
//! equal brute force. `MONO_MULT` sets the scale (default 0.01, about 200k
//! people); `MONO_SEEDS` the seeds (default 42). Each test runs the
//! heritage world (one cell per group, with the open market) and the
//! heritage-blind one (one cell).
use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::stream::year_start;
use rayon::prelude::*;

const GESTATION: i64 = 266 * 86_400;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.01)
}

/// The worlds a test runs on: heritage cells, then heritage-blind.
fn worlds(p: &Params, seed: u64) -> [Mono<'_>; 2] {
    [Mono::new(p, seed, mult()), Mono::blind(p, seed, mult())]
}

fn everyone(m: &Mono) -> Vec<Pid> {
    (0..m.cells())
        .flat_map(|cell| (m.first_year()..=m.last_year()).map(move |y| (cell, y)))
        .flat_map(|(cell, y)| (0..m.cohort_n(cell, y)).map(move |i| Pid { cell, y, i }))
        .collect()
}

#[test]
fn kinship_and_unions_agree_from_every_side() {
    let p = Params::embedded("us").unwrap();
    for (seed, m) in seeds().into_iter().flat_map(|s| worlds(&p, s).map(|m| (s, m))) {
        let all = everyone(&m);
        let bad: Vec<String> = all
            .par_iter()
            .flat_map_iter(|&x| {
                let mut e = Vec::new();
                let (b, d) = (m.birth(x), m.death(x));
                if d <= b {
                    e.push(format!("{x:?}: dies {d} at or before birth {b}"));
                }
                if m.pid(m.id(x)) != x {
                    e.push(format!("{x:?}: id {} reads back as {:?}", m.id(x), m.pid(m.id(x))));
                }
                if let Some(mo) = m.mother(x) {
                    if mo.cell != x.cell {
                        e.push(format!("{x:?}: mother {mo:?} in another cell"));
                    }
                    if !m.children(mo).contains(&x) {
                        e.push(format!("{x:?}: not among mother {mo:?}'s children"));
                    }
                    if m.death(mo) <= b {
                        e.push(format!("{x:?}: mother {mo:?} dead at the birth"));
                    }
                    let age = (b - m.birth(mo)) as f64 / 31_556_952.0;
                    if !(14.0..=47.0).contains(&age) {
                        e.push(format!("{x:?}: mother aged {age:.1}"));
                    }
                }
                if let Some(f) = m.father(x) {
                    if !m.children(f).contains(&x) {
                        e.push(format!("{x:?}: not among father {f:?}'s children"));
                    }
                    if m.death(f) <= b - GESTATION {
                        e.push(format!("{x:?}: father {f:?} dead at the conception"));
                    }
                    if m.spouse(m.mother(x).unwrap()).map(|s| s.0) != Some(f) {
                        e.push(format!("{x:?}: father {f:?} is not the mother's partner"));
                    }
                }
                let kids = m.children(x);
                for &c in &kids {
                    let back = if m.sex(x) == Sex::Female { m.mother(c) } else { m.father(c) };
                    if back != Some(x) {
                        e.push(format!("{x:?}: child {c:?} points back to {back:?}"));
                    }
                }
                // A mother's births are at least 330 days apart.
                if m.sex(x) == Sex::Female {
                    let mut births: Vec<i64> = kids.iter().map(|&c| m.birth(c)).collect();
                    births.sort_unstable();
                    for w in births.windows(2) {
                        if w[1] - w[0] < 330 * 86_400 {
                            e.push(format!("{x:?}: births {} days apart", (w[1] - w[0]) / 86_400));
                        }
                    }
                }
                if let Some((s, start)) = m.spouse(x) {
                    if m.spouse(s) != Some((x, start)) {
                        e.push(format!("{x:?}: spouse {s:?} disagrees"));
                    }
                    if m.union_end(x) != m.union_end(s) {
                        e.push(format!("{x:?}: union end disagrees with {s:?}"));
                    }
                    if m.death(x) <= start || m.death(s) <= start {
                        e.push(format!("{x:?}: union starts after a death"));
                    }
                }
                e.into_iter()
            })
            .collect();
        for b in bad.iter().take(20) {
            eprintln!("{b}");
        }
        assert!(bad.is_empty(), "seed {seed}, {} cells: {} disagreements", m.cells(), bad.len());
        // Every native child is counted once, by its mother.
        let with_mother = all.par_iter().filter(|&&x| m.mother(x).is_some()).count();
        let counted: usize = all.par_iter().filter(|&&x| m.sex(x) == Sex::Female).map(|&w| m.children(w).len()).sum();
        assert_eq!(with_mother, counted, "seed {seed}: children counted once");
    }
}

#[test]
fn alive_counts_equal_brute_force() {
    let p = Params::embedded("us").unwrap();
    for (seed, m) in seeds().into_iter().flat_map(|s| worlds(&p, s).map(|m| (s, m))) {
        let all = everyone(&m);
        for t in [year_start(1900), year_start(1950) + 100 * 86_400, year_start(2023) + 287 * 86_400, year_start(2060)] {
            let brute = all.par_iter().filter(|&&x| m.alive_at(x, t)).count() as u64;
            let (closed, checked, _) = m.count_alive(t);
            let exact = (closed as i64 + checked) as u64;
            assert_eq!(exact, brute, "seed {seed}, t {t}");
            let (lo, hi) = m.alive_bounds(t);
            assert!(lo <= exact && exact <= hi, "seed {seed}, t {t}: bounds");
        }
    }
}
