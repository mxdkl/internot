//! Residence on the monotone world, over people at several dates on a small
//! world: everyone alive has a home; positions are paths of the place tree;
//! household members share the address; addresses are stable across calls
//! and within a stay. `MONO_MULT` (default 0.003), `MONO_SEEDS` (default 42).
use internot_society::mono::{Mono, Pid, AREA, CLUSTER, COUNTY, LEVELS, TRACT, ZONE};
use internot_society::Params;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003)
}

#[test]
fn homes_are_tree_paths_shared_by_households() {
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    for seed in seeds() {
        for m in [Mono::new(&p, seed, mult()), Mono::blind(&p, seed, mult())] {
            let places = m.places();
            for year in [1860, 1920, 1980, 2023] {
                let t = year_start(year) + 200 * 86_400;
                let mut people = Vec::new();
                for c in 0..m.cells() {
                    for y in m.first_year()..=year {
                        m.alive_in_cohort(c, y, t, &mut |x| people.push(x));
                    }
                }
                let bad: Vec<String> = people
                    .par_iter()
                    .step_by(3)
                    .flat_map_iter(|&x: &Pid| {
                        let mut e = Vec::new();
                        let h = m.household(x, t).unwrap();
                        if m.unit_info(m.anchor_unit(h)).is_none() {
                            let end = m.chain_end(x, t);
                            return vec![format!(
                                "{x:?}: household {h:?} at {year} has no unit; chain end {end:?} (age {:.1}), spell0 {:?}, leave {:?}, dependent at t {:?}",
                                m.age_at(end, t),
                                m.spell0_start(end),
                                m.leave_time(end),
                                m.dependent_of(end, t)
                            )]
                            .into_iter();
                        }
                        let Some(home) = m.home(x, t) else {
                            return vec![format!("{x:?}: alive with no home at {year}")].into_iter();
                        };
                        let pos = home.pos;
                        for k in ZONE..LEVELS {
                            if places.parent(k, pos[k]) != pos[k - 1] {
                                e.push(format!("{x:?}: level {k} node {} not under {}", pos[k], pos[k - 1]));
                            }
                        }
                        if pos[AREA] as usize >= places.count(AREA) || pos[TRACT] as usize >= places.count(TRACT) {
                            e.push(format!("{x:?}: position out of range {pos:?}"));
                        }
                        let again = m.home(x, t).unwrap();
                        if again.line() != home.line() || again.pos != pos {
                            e.push(format!("{x:?}: home differs between calls"));
                        }
                        for &q in m.members(h, t).iter() {
                            if m.home(q, t).map(|hq| hq.line()) != Some(home.line()) {
                                e.push(format!("{x:?}: household member {q:?} lives elsewhere at {year}"));
                            }
                        }
                        // Within a stay, the address holds.
                        if home.dwelling.since < t - 86_400 {
                            let mid = home.dwelling.since + (t - home.dwelling.since) / 2;
                            if m.household(x, mid) == Some(h) {
                                if let Some(earlier) = m.home(x, mid) {
                                    if earlier.dwelling == home.dwelling && earlier.line() != home.line() {
                                        e.push(format!("{x:?}: the same dwelling has two addresses"));
                                    }
                                }
                            }
                        }
                        let _ = (COUNTY, CLUSTER);
                        e.into_iter()
                    })
                    .collect();
                for b in bad.iter().take(20) {
                    eprintln!("{b}");
                }
                assert!(bad.is_empty(), "seed {seed}, {} cells, {year}: {} problems", m.cells(), bad.len());
            }
        }
    }
}
