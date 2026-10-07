//! Names on the monotone world, on a small world: every surname comes
//! from a parent's line or a founder's group table, wedding changes take
//! the partner's surname, names are stable, and middle names differ from
//! first names. `MONO_MULT` (default 0.01), `MONO_SEEDS` (default 42).
use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use rayon::prelude::*;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.01)
}

/// About `k` people per cell, every cohort sampled.
fn sample(m: &Mono, k: u64) -> Vec<Pid> {
    let mut v = Vec::new();
    for cell in 0..m.cells() {
        for y in m.first_year()..=m.last_year().min(2030) {
            let n = m.cohort_n(cell, y);
            for j in 0..n.min(k) {
                v.push(Pid { cell, y, i: Key::from_seed(2).with3(cell as u64, y as u64, j).below(n) });
            }
        }
    }
    v
}

#[test]
fn surnames_come_from_parents_and_weddings() {
    let p = Params::embedded("us").unwrap();
    for seed in seeds() {
        for m in [Mono::new(&p, seed, mult()), Mono::blind(&p, seed, mult())] {
            let xs = sample(&m, 40);
            let bad: Vec<String> = xs
                .par_iter()
                .flat_map_iter(|&x| {
                    let mut e = Vec::new();
                    let b = m.birth_surname(x);
                    // Stable.
                    if m.birth_surname(x) != b || m.first_name_id(x) != m.first_name_id(x) {
                        e.push(format!("{x:?}: names differ between calls"));
                    }
                    if m.first_name(x).is_empty() || m.surname_text(b).is_empty() {
                        e.push(format!("{x:?}: an empty name"));
                    }
                    if let Some(internot_society::mono::Middle::Given(g)) = m.middle_name_of(x) {
                        if g == m.first_name_id(x) {
                            e.push(format!("{x:?}: middle name repeats the first"));
                        }
                    }
                    // From a parent's line: the child's (first) surname is the
                    // father's or the mother's at the birth.
                    if let Some(mo) = m.mother(x) {
                        let birth = m.birth(x);
                        let lines: Vec<u32> = [Some(mo), m.father(x)].into_iter().flatten().flat_map(|p| [m.surname(p, birth).first, m.birth_surname(p).first]).collect();
                        if !lines.contains(&b.first) {
                            e.push(format!("{x:?}: surname {} from neither parent", m.surname_text(b)));
                        }
                    }
                    // Before a marriage, the birth surname; after, the birth
                    // surname, the partner's, or the two hyphenated.
                    if let Some(u) = m.union_of(x) {
                        let s0 = m.surname(x, u.start - 1);
                        if s0 != b {
                            e.push(format!("{x:?}: surname changed before the union"));
                        }
                        if let Some(wed) = m.marriage_of(&u) {
                            let after = m.surname(x, wed);
                            let partner = m.birth_surname(u.partner(x));
                            let ok = after == b || after == partner || (after.hyphen && after.first == b.first && after.second == Some(partner.first));
                            if !ok {
                                e.push(format!("{x:?}: after the wedding {} (born {}, partner {})", m.surname_text(after), m.surname_text(b), m.surname_text(partner)));
                            }
                            if wed < u.start || wed >= u.end {
                                e.push(format!("{x:?}: wedding outside the union"));
                            }
                            // The same from both sides.
                            if m.marriage_date(u.partner(x)) != Some(wed) {
                                e.push(format!("{x:?}: partners disagree on the wedding"));
                            }
                        }
                    }
                    e.into_iter()
                })
                .collect();
            for b in bad.iter().take(20) {
                eprintln!("{b}");
            }
            assert!(bad.is_empty(), "seed {seed}, {} cells: {} problems of {}", m.cells(), bad.len(), xs.len());
        }
    }
}

#[test]
fn most_married_women_take_their_husbands_surname_before_1970() {
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, 42, mult());
    let (mut took, mut n) = (0, 0);
    for x in sample(&m, 400).into_iter().filter(|x| (1900..1940).contains(&x.y)) {
        if m.sex(x) != Sex::Female {
            continue;
        }
        let Some(u) = m.union_of(x) else { continue };
        let Some(wed) = m.marriage_of(&u) else { continue };
        n += 1;
        took += (m.surname(x, wed) == m.birth_surname(u.husband)) as u32;
    }
    assert!(n > 200, "{n} married women");
    let share = took as f64 / n as f64;
    assert!(share > 0.9, "{share:.3} took the husband's surname");
}
