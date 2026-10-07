//! Households on the monotone world, exhaustively over people at several
//! dates on a small world: `household` and `members` agree both ways, every
//! dependent chain ends at an independent adult, and minors don't head
//! homes while a parent, grandparent or adult sibling lives.
//! `MONO_MULT` (default 0.003), `MONO_SEEDS` (default 42).
use internot_society::mono::{Household, Mono, Pid};
use internot_society::Params;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003)
}

fn alive(m: &Mono, t: i64) -> Vec<Pid> {
    let mut v = Vec::new();
    for cell in 0..m.cells() {
        for y in m.first_year()..=m.last_year() {
            m.alive_in_cohort(cell, y, t, &mut |x| v.push(x));
        }
    }
    v
}

#[test]
fn households_and_members_agree() {
    let p = Params::embedded("us").unwrap();
    for seed in seeds() {
        for m in [Mono::new(&p, seed, mult()), Mono::blind(&p, seed, mult())] {
            for year in [1900, 1950, 1990, 2023] {
                let t = year_start(year) + 123 * 86_400;
                let people = alive(&m, t);
                let bad: Vec<String> = people
                    .par_iter()
                    .flat_map_iter(|&x| {
                        let mut e = Vec::new();
                        let Some(h) = m.household(x, t) else {
                            return vec![format!("{x:?}: alive with no household at {year}")].into_iter();
                        };
                        let members = m.members(h, t);
                        if !members.contains(&x) {
                            e.push(format!("{x:?}: not among the members of its household {h:?} at {year}"));
                        }
                        for &y in members.iter() {
                            if m.household(y, t) != Some(h) {
                                e.push(format!("{x:?}: member {y:?} of {h:?} resolves to {:?} at {year}", m.household(y, t)));
                            }
                        }
                        // A home is headed by an adult or a couple; a minor
                        // heads one only without any living guardian.
                        if let Household::Solo { person, .. } = h {
                            if m.age_at(person, t) < 18.0 && person == x && m.dependent_of(x, t).is_some() {
                                e.push(format!("{x:?}: a minor alone with a guardian at {year}"));
                            }
                        }
                        let end = m.chain_end(x, t);
                        if m.dependent_of(end, t).is_some() {
                            e.push(format!("{x:?}: chain end {end:?} is a dependent"));
                        }
                        e.into_iter()
                    })
                    .collect();
                for b in bad.iter().take(20) {
                    eprintln!("{b}");
                }
                assert!(bad.is_empty(), "seed {seed}, {} cells, {year}: {} problems among {} people", m.cells(), bad.len(), people.len());
            }
        }
    }
}
