//! Residence (L4) on the tiny world with a synthetic place tree: rosters
//! are exact (they equal brute force over every person), and addresses are
//! defined for everyone present. With `TEST_PACK=<pack>` (a pack in
//! `worlds/`), on that pack's world and real places instead; in area mode
//! (`us-areas-tiny`) also: addresses follow the ledger's areas.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use internot_society::residence::{Places, Residence, AREA, COUNTY, TRACT};
use internot_society::{Household, Params, World};
use procedural_core::key::Key;
use procedural_core::stream::{year_start, DAY};

fn setup(seed: u64) -> (World, Residence) {
    if let Ok(name) = std::env::var("TEST_PACK") {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../worlds");
        let p = Params::load(&root, &name).expect("TEST_PACK names a pack in worlds/");
        let w = World::build(p, seed);
        let places = Places::from_params(&w.ledger().params).expect("the pack has places");
        let r = Residence::build(&w, places);
        return (w, r);
    }
    let w = World::build(Params::tiny(), seed);
    let regions = w.ledger().params.region_count() as u16;
    let places = Places::synthetic(
        regions,
        [2, 3, 2, 2, 3],
        1900,
        10,
        Key::from_seed(seed + 100),
    );
    let r = Residence::build(&w, places);
    (w, r)
}

/// Every present person's household, grouped by the tract it lives in.
fn brute_force(w: &World, r: &Residence, t: i64) -> BTreeMap<u32, BTreeSet<Household>> {
    let mut by_tract: BTreeMap<u32, BTreeSet<Household>> = BTreeMap::new();
    for x in 0..w.population() as u32 {
        if let Some(h) = w.household(x, t) {
            let p = r.address(w, h, t);
            by_tract.entry(p[TRACT]).or_default().insert(h);
        }
    }
    by_tract
}

#[test]
fn rosters_equal_brute_force() {
    for seed in [7u64, 11] {
        let (w, r) = setup(seed);
        let tracts = r.places().count(TRACT) as u32;
        for year in [1900, 1912, 1931, 1950, 1968, 1977, 1989] {
            let t = year_start(year) + 123 * DAY + 4321;
            let started = Instant::now();
            let mut brute = brute_force(&w, &r, t);
            let brute_time = started.elapsed();
            let started = Instant::now();
            let mut households = 0;
            for n in 0..tracts {
                let got: BTreeSet<Household> = r.roster(&w, TRACT, n, t).into_iter().collect();
                let want = brute.remove(&n).unwrap_or_default();
                households += want.len();
                assert_eq!(
                    got.symmetric_difference(&want).collect::<Vec<_>>(),
                    Vec::<&Household>::new(),
                    "seed {seed}, tract {n}, {year}"
                );
            }
            assert!(brute.is_empty(), "every address is a tract of the tree");
            println!(
                "seed {seed} {year}: {households} households, brute {:?}, rosters {:?}",
                brute_time,
                started.elapsed()
            );
        }
    }
}

#[test]
fn rosters_nest() {
    let (w, r) = setup(7);
    let t = year_start(1960) + 200 * DAY;
    let places = r.places();
    for c in 0..places.count(COUNTY) as u32 {
        let county: BTreeSet<Household> = r.roster(&w, COUNTY, c, t).into_iter().collect();
        let tracts: BTreeSet<Household> = places
            .tracts_under(COUNTY, c)
            .flat_map(|n| r.roster(&w, TRACT, n, t))
            .collect();
        assert_eq!(county, tracts, "county {c}");
    }
    for b in 0..places.count(AREA) as u32 {
        let area = r.roster(&w, AREA, b, t);
        let sum: usize = places
            .tracts_under(AREA, b)
            .map(|n| r.roster(&w, TRACT, n, t).len())
            .sum();
        assert_eq!(area.len(), sum, "area {b}");
    }
}

#[test]
fn timelines_agree_with_direct_positions() {
    let (w, r) = setup(7);
    let mut stretches = 0;
    let mut units = std::collections::HashSet::new();
    for year in [1905, 1930, 1955, 1980] {
        let t = year_start(year) + 77 * DAY;
        for x in (0..w.population() as u32).step_by(41) {
            let Some(h) = w.household(x, t) else { continue };
            let (u, _) = r.anchor(&w, h, t);
            if !units.insert(u) {
                continue;
            }
            let (info, line) = r.timeline(&w, u).expect("an anchor exists");
            assert_eq!(
                line.first().unwrap().0,
                info.start,
                "{u:?} starts at its start"
            );
            assert_eq!(line.last().unwrap().1, info.end, "{u:?} ends at its end");
            for pair in line.windows(2) {
                assert_eq!(pair[0].1, pair[1].0, "{u:?} stretches tile its span");
            }
            for &(from, to, p) in &line {
                for probe in [from, from + (to - from) / 2, to - 1] {
                    assert_eq!(r.pos(&w, u, probe).unwrap(), p, "{u:?} at {probe}");
                }
                stretches += 1;
            }
        }
    }
    println!("{} units, {stretches} stretches", units.len());
    assert!(stretches > 1000);
}

#[test]
fn addresses_follow_the_ledger_areas() {
    // Area mode (ledger spec §9): everyone independent lives in the area the
    // ledger counts them in: in their own home (a union's or a single's),
    // with roommates, or as a guest of kin.
    let (w, r) = setup(7);
    if !w.ledger().params.places.by_area {
        return;
    }
    let places = r.places();
    let (mut checked, mut roommates) = (0u64, 0u64);
    // Independent people living in another's household: roommates and kin
    // guests.
    let (mut guests, mut guests_elsewhere) = (0u64, 0u64);
    let (mut mates_elsewhere,) = (0u64,);
    for year in [1905, 1930, 1955, 1980] {
        let t = year_start(year) + 177 * DAY;
        for x in 0..w.population() as u32 {
            let Some(h) = w.household(x, t) else { continue };
            let own = match h {
                Household::Union { a, b, .. } => a == x || b == x,
                Household::Solo { person, .. } => person == x,
                Household::Roommates { .. } => {
                    roommates += 1;
                    false
                }
            };
            if !own {
                if w.dependent_of(x, t).is_none() {
                    let elsewhere = places.region(r.address(&w, h, t)[AREA]) != w.area_at(x, t);
                    if matches!(h, Household::Roommates { .. }) {
                        mates_elsewhere += elsewhere as u64;
                    } else {
                        guests += 1;
                        guests_elsewhere += elsewhere as u64;
                    }
                }
                continue;
            }
            let area = r.address(&w, h, t)[AREA];
            checked += 1;
            assert_eq!(
                places.region(area),
                w.area_at(x, t),
                "{x} at {year}: lives in area {area}, counted in region {}",
                w.area_at(x, t)
            );
        }
    }
    println!(
        "{checked} people in their own homes checked; {roommates} roommates ({mates_elsewhere} in another area), {guests} kin guests ({guests_elsewhere} in another area)"
    );
    assert!(checked > 10_000);
    assert_eq!((mates_elsewhere, guests_elsewhere), (0, 0), "roommates and kin guests elsewhere");
}
