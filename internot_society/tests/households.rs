//! L3 pass criteria on a tiny world, checked exhaustively over every person
//! at many times: households partition everyone present, `members` agrees
//! with `household` both ways, and the structure makes sense. See
//! `docs/superpowers/plans/2026-09-30-l3-households.md` §9.

use std::collections::HashMap;
use std::sync::OnceLock;

use internot_society::world::{year_start, DAY};
use internot_society::{Household, Members, Params, PersonId, World};

const SECS_PER_YEAR: f64 = 365.2425 * 86_400.0;

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| World::build(Params::tiny(), 7))
}

fn age_at(w: &World, id: PersonId, t: i64) -> f64 {
    (t - w.birth(id)) as f64 / SECS_PER_YEAR
}

/// Times to check: every sixth year of the tiny world, at a different day
/// of the year each time, so epoch and birthday boundaries vary.
fn times() -> impl Iterator<Item = i64> {
    (1902..=1990)
        .step_by(6)
        .enumerate()
        .map(|(k, y)| year_start(y) + (37 + 53 * k as i64) % 365 * DAY + 12_345)
}

/// Every household at `t` with its members, after checking that each
/// person present is listed in their own household's members exactly once
/// and that nobody else is listed.
fn partition(w: &World, t: i64) -> HashMap<Household, Members> {
    let mut found: HashMap<Household, (Members, usize)> = HashMap::new();
    for x in 0..w.population() as PersonId {
        match w.household(x, t) {
            None => assert!(!w.present_at(x, t), "{x} present but homeless at {t}"),
            Some(h) => {
                assert!(w.present_at(x, t), "{x} housed but absent");
                let (members, count) = found.entry(h).or_insert_with(|| (w.members(h, t), 0));
                assert!(
                    members.contains(&x),
                    "{x} not among the members of its household {h:?}: {members:?}"
                );
                *count += 1;
            }
        }
    }
    // Each member lists back (members hold no duplicates, and every person
    // whose household is `h` is among them), so equal counts mean equal
    // sets.
    found
        .into_iter()
        .map(|(h, (members, count))| {
            assert_eq!(
                members.len(),
                count,
                "{h:?} lists {members:?} but {count} people live there"
            );
            (h, members)
        })
        .collect()
}

/// True if minor `x` has no living parent, grandparent or adult sibling at
/// `t`: nobody in the world to take them in.
fn kinless(w: &World, x: PersonId, t: i64) -> bool {
    let parents: Vec<PersonId> = [w.mother(x), w.father(x)].into_iter().flatten().collect();
    let grandparents = parents
        .iter()
        .flat_map(|&p| [w.mother(p), w.father(p)])
        .flatten();
    parents
        .iter()
        .copied()
        .chain(grandparents)
        .all(|p| !w.present_at(p, t))
        && w.siblings(x)
            .iter()
            .all(|&s| !w.present_at(s, t) || age_at(w, s, t) < 18.0)
}

#[test]
fn households_partition_everyone_present_and_members_agree() {
    let w = world();
    let mut kinds = [0usize; 3];
    for t in times() {
        for (h, members) in partition(w, t) {
            kinds[match h {
                Household::Union { .. } => 0,
                Household::Solo { .. } => 1,
                Household::Roommates { .. } => 2,
            }] += 1;
            assert!(!members.is_empty());
        }
    }
    assert!(kinds.iter().all(|&k| k > 0), "every kind occurs: {kinds:?}");
}

#[test]
fn households_make_sense() {
    let w = world();
    let (mut founder_minors, mut kinless_orphans) = (0, 0);
    let (mut orphans_with_kin, mut elders_with_child) = (0, 0);
    for t in times() {
        for (h, members) in partition(w, t) {
            match h {
                Household::Union { a, b, start } => {
                    // Both partners live there, in an active union.
                    assert!(members.contains(&a) && members.contains(&b));
                    let u = w
                        .unions(a)
                        .into_iter()
                        .flatten()
                        .find(|u| u.partner == b && u.start == start)
                        .expect("the household's union exists");
                    assert!(u.start <= t && t < u.end, "{h:?} is not active at {t}");
                }
                Household::Solo { person, .. } => {
                    assert!(members.contains(&person));
                    let alone_minor = members.len() == 1 && age_at(w, person, t) < 18.0;
                    if alone_minor {
                        // Only founders (no in-world parents) and orphans
                        // with no living kin to take them in can be minors
                        // living alone (the plan's measured residual).
                        if w.is_founder(person) {
                            founder_minors += 1;
                        } else {
                            assert!(kinless(w, person, t), "minor {person} lives alone at {t}");
                            kinless_orphans += 1;
                        }
                    }
                }
                Household::Roommates { .. } => {
                    // Roommates are single adults with no child under 18, at
                    // least two of them (plus any kin they took in).
                    let adults: Vec<_> = members
                        .iter()
                        .filter(|&&m| w.partner_at(m, t).is_none() && age_at(w, m, t) >= 18.0)
                        .collect();
                    assert!(adults.len() >= 2, "{h:?}: {members:?}");
                }
            }
            for &m in members.iter() {
                // A minor lives with a parent, or with kin if orphaned.
                if age_at(w, m, t) < 18.0 && !w.is_founder(m) {
                    let parents = [w.mother(m), w.father(m)];
                    let with_parent = parents.iter().flatten().any(|p| members.contains(p));
                    if !with_parent {
                        assert!(
                            parents
                                .iter()
                                .flatten()
                                .all(|&p| !w.present_at(p, t) || w.household(p, t) != Some(h)),
                            "{m}"
                        );
                        orphans_with_kin += 1;
                    }
                }
                if age_at(w, m, t) >= 65.0
                    && w.children(m)
                        .iter()
                        .any(|c| members.contains(c) && age_at(w, *c, t) >= 18.0)
                {
                    elders_with_child += 1;
                }
            }
        }
    }
    eprintln!(
        "minors alone: founders {founder_minors}, kinless orphans {kinless_orphans}; \
         minors with kin but no parent {orphans_with_kin}; elders with an adult child {elders_with_child}"
    );
    assert!(elders_with_child > 0);
}

#[test]
fn households_change_only_now_and_then() {
    // Over a lifetime, a household changes at events (leaving home, unions,
    // deaths, moving in with kin, roommate epochs): a few dozen at most, and
    // never back and forth from one day to the next. (Two different events
    // can fall on consecutive days, e.g. a roommate group thinning out just
    // before a union starts.)
    let w = world();
    let mut longest = 0;
    for x in (0..w.population() as PersonId).step_by(97) {
        let (from, to) = (
            w.birth(x).max(year_start(1900)),
            w.death(x).min(year_start(1990)),
        );
        let mut last = None;
        let mut changes = 0;
        let mut t = from;
        while t < to {
            let h = w.household(x, t);
            if h.is_some() && last.is_some() && h != last {
                changes += 1;
                // A change doesn't revert the next day.
                let next = w.household(x, t + DAY);
                assert!(
                    next.is_none() || next != last,
                    "{x} flickers back to {last:?} at {t}"
                );
            }
            if h.is_some() {
                last = h;
            }
            t += 30 * DAY;
        }
        longest = longest.max(changes);
    }
    eprintln!("most household changes in one life: {longest}");
    assert!(longest < 60, "too many household changes: {longest}");
}
