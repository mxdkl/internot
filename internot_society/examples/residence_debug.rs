//! Finds units that brute force places in a area but its history misses,
//! and prints how they relate to their source unit.
use internot_society::residence::{Places, Residence, Source, Unit, AREA};
use internot_society::{Params, World};
use procedural_core::key::Key;
use procedural_core::stream::{year_start, DAY};
use std::collections::HashSet;

fn main() {
    let w = World::build(Params::tiny(), 7);
    let regions = w.ledger().params.region_count() as u16;
    let places = Places::synthetic(regions, [2, 3, 2, 2, 3], 1900, 10, Key::from_seed(107));
    let r = Residence::build(&w, places);
    let mut shown = 0;
    for year in [1912, 1931, 1950, 1968, 1977, 1989] {
        let t = year_start(year) + 123 * DAY + 4321;
        for b in 0..r.places().count(AREA) as u32 {
            let h = r.history(&w, b);
            let known: HashSet<Unit> = h.members.iter().map(|m| m.unit).collect();
            for x in 0..w.population() as u32 {
                let Some(hh) = w.household(x, t) else {
                    continue;
                };
                let (u, at) = r.anchor(&w, hh, t);
                let p = r.pos(&w, u, at).unwrap();
                if p[AREA] != b || known.contains(&u) || shown >= 6 {
                    continue;
                }
                shown += 1;
                let info = r.info(&w, u).unwrap();
                println!("missing {u:?} {info:?}");
                let mut cur = u;
                for _ in 0..6 {
                    let i = r.info(&w, cur).unwrap();
                    match i.source {
                        Source::From { unit, .. } => {
                            let people = |v: Unit| match v {
                                Unit::Union { a, b, .. } => vec![a, b],
                                Unit::Spell { x, .. } => vec![x],
                            };
                            for c in people(cur) {
                                for q in people(unit) {
                                    let rel = if w.mother(c) == Some(q) || w.father(c) == Some(q) {
                                        "child"
                                    } else if w.mother(c).is_some_and(|m| {
                                        w.mother(m) == Some(q) || w.father(m) == Some(q)
                                    }) || w.father(c).is_some_and(|f| {
                                        w.mother(f) == Some(q) || w.father(f) == Some(q)
                                    }) {
                                        "grandchild"
                                    } else if w.siblings(c).contains(&q) {
                                        "sibling"
                                    } else if c == q {
                                        "self"
                                    } else {
                                        "?"
                                    };
                                    println!(
                                        "   {c} (born {}, spell0 {:?}, death {}) is {rel} of {q}",
                                        w.birth_year(c),
                                        r.spell0_start(&w, c),
                                        w.death(c)
                                    );
                                }
                            }
                            println!("  <- {unit:?} in known: {}", known.contains(&unit));
                            if known.contains(&unit) {
                                break;
                            }
                            cur = unit;
                        }
                        Source::Seed => {
                            println!("  <- seed");
                            break;
                        }
                        Source::Fresh { area } => {
                            println!("  <- fresh in area {area}");
                            break;
                        }
                        Source::Back { unit, .. } => {
                            println!("  <- back near where {unit:?} formed");
                            break;
                        }
                    }
                }
            }
        }
    }
}
