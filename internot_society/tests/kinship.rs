//! R1 pass criteria on a tiny world, checked exhaustively over every person:
//! duality of every relation, life bounds, kin rules, ledger closure and
//! cache independence. See `docs/superpowers/plans/2026-09-30-r1-kinship-prototype.md`.

use std::collections::HashMap;
use std::sync::OnceLock;

use internot_society::world::{year_of, year_start, DAY};
use internot_society::{CellKind, Params, PersonId, Sex, World};

const SECS_PER_YEAR: f64 = 365.25 * 86_400.0;

/// The tiny test pack, or the pack named by `TEST_PACK` (in `worlds/`), so
/// the suite also checks other configurations (the area mode:
/// `TEST_PACK=us-areas-tiny`).
fn params() -> Params {
    match std::env::var("TEST_PACK") {
        Ok(name) => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../worlds");
            Params::load(&root, &name).expect("TEST_PACK names a pack in worlds/")
        }
        Err(_) => Params::tiny(),
    }
}

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    // The ledger's cells are kept: the suite checks the world against them.
    W.get_or_init(|| World::build_keeping_ledger(params(), 7))
}

fn everyone(w: &World) -> impl Iterator<Item = PersonId> {
    0..w.population() as PersonId
}

fn age_at(w: &World, id: PersonId, t: i64) -> f64 {
    (t - w.birth(id)) as f64 / SECS_PER_YEAR
}

#[test]
fn unions_are_mutual_and_agree_on_every_date() {
    let w = world();
    let (mut partnered, mut second, mut third) = (0, 0, 0);
    for x in everyone(w) {
        let us = w.unions(x);
        let cells = w.union_cells(x);
        for (k, (u, cell)) in us.iter().zip(cells).enumerate() {
            let Some(u) = u else { continue };
            let (_, kind) = cell.expect("a union has a cell");
            match k {
                0 => partnered += 1,
                1 => second += 1,
                _ => third += 1,
            }
            // The partner holds the same union, in whichever seat it is for
            // them (a re-partnering partner in a later one, R1c and R1d). A
            // divorced couple can remarry each other, so match the start
            // too.
            let vs = w.unions(u.partner);
            let j = vs
                .iter()
                .position(|v| v.is_some_and(|v| v.partner == x && v.start == u.start))
                .unwrap_or_else(|| panic!("partner of partner of {x}"));
            let v = vs[j].unwrap();
            assert_eq!(
                (u.start, u.separation, u.end, u.key),
                (v.start, v.separation, v.end, v.key),
                "union facts differ for {x}"
            );
            assert_eq!(w.union_cells(u.partner)[j].unwrap().1, kind.partner());
            assert_eq!(
                w.sex(x) == w.sex(u.partner),
                kind.same_sex(),
                "{x}: sexes vs union kind"
            );
            assert_eq!(
                k >= 1,
                kind.second(),
                "{x}: seat {k} holds a {kind:?} union"
            );
        }
        // Availability (R1c, R1d): each later union follows the previous
        // one's separation, in a later year, and starts while alive; the
        // seats fill in order.
        for k in 1..us.len() {
            match (us[k - 1], us[k]) {
                (Some(a), Some(b)) => {
                    let sep = a.separation.expect("a later union follows a separation");
                    assert!(
                        year_of(sep) < year_of(b.start),
                        "{x}: union {k} before union {} separates",
                        k - 1
                    );
                    assert!(b.start < w.death(x), "{x} dies before union {k}");
                }
                (None, Some(_)) => panic!("{x}: union {k} without union {}", k - 1),
                _ => {}
            }
        }
    }
    assert!(partnered > 1000, "only {partnered} partnered people");
    assert!(second > 50, "only {second} second unions");
    assert!(third > 0, "no third unions");
    eprintln!("unions: {partnered} first, {second} second, {third} third or later");
}

#[test]
fn mothers_and_children_are_dual() {
    let w = world();
    let mut with_mother = 0;
    for x in everyone(w) {
        if let Some(m) = w.mother(x) {
            with_mother += 1;
            assert_eq!(w.sex(m), Sex::Female);
            assert!(
                w.children(m).contains(&x),
                "{x} missing from mother {m}'s children"
            );
        }
    }
    assert!(with_mother > 1000);
    for m in everyone(w).filter(|&p| w.sex(p) == Sex::Female) {
        for c in w.children(m) {
            assert_eq!(w.mother(c), Some(m), "child {c} of {m}");
        }
    }
}

#[test]
fn fathers_and_children_are_dual() {
    let w = world();
    let mut with_father = 0;
    for x in everyone(w) {
        if let Some(f) = w.father(x) {
            with_father += 1;
            assert_eq!(w.sex(f), Sex::Male);
            assert!(
                w.children(f).contains(&x),
                "{x} missing from father {f}'s children"
            );
        }
    }
    assert!(with_father > 1000);
    for f in everyone(w).filter(|&p| w.sex(p) == Sex::Male) {
        for c in w.children(f) {
            assert_eq!(w.father(c), Some(f), "child {c} of {f}");
        }
    }
}

#[test]
fn siblings_are_the_union_of_both_parents_children() {
    // `siblings` reads only the mother's children; this holds while every
    // man's children are his partner's (one union each, R1a/b).
    let w = world();
    for x in everyone(w) {
        let mut expected: Vec<PersonId> = Vec::new();
        if let Some(m) = w.mother(x) {
            expected.extend(w.children(m));
        }
        if let Some(f) = w.father(x) {
            expected.extend(w.children(f));
        }
        expected.sort_unstable();
        expected.dedup();
        expected.retain(|&s| s != x);
        let mut got = w.siblings(x).to_vec();
        got.sort_unstable();
        assert_eq!(got, expected, "siblings of {x}");
    }
}

#[test]
fn life_bounds_hold() {
    let w = world();
    let y0_start = year_start(w.ledger().params.y0);
    for x in everyone(w) {
        let (b, d) = (w.birth(x), w.death(x));
        assert!(b < d, "{x} dies before birth");
        if w.is_founder(x) {
            assert!(d > y0_start, "founder {x} dies before the world starts");
        }
        if let Some(m) = w.mother(x) {
            assert!(
                w.birth(m) < b && b < w.death(m),
                "mother of {x} not alive at birth"
            );
            let age = age_at(w, m, b);
            assert!((12.0..=51.0).contains(&age), "mother of {x} aged {age:.1}");
        }
        if let Some(f) = w.father(x) {
            let conception = b - w.ledger().params.fertility.gestation_days * DAY;
            assert!(
                w.birth(f) < conception && conception < w.death(f),
                "father of {x} not alive at conception"
            );
        }
        if let Some(u) = w.union(x) {
            assert!(u.start < w.death(x), "{x} dies before the union starts");
            assert!(
                age_at(w, x, u.start) >= 15.0,
                "{x} partners at {:.1}",
                age_at(w, x, u.start)
            );
            assert!(u.end <= w.death(x).min(w.death(u.partner)));
        }
    }
}

/// A union formed abroad by a couple who arrived together: it began no
/// later than the woman's arrival year (in-world unions of arrivals start
/// the year after at the earliest).
fn arrived_together(w: &World, woman: PersonId, u: &internot_society::Union) -> bool {
    w.arrival(woman)
        .is_some_and(|a| year_of(u.start) <= year_of(a))
}

#[test]
fn immigrants_arrive_alive_with_their_families() {
    let w = world();
    let (mut adults, mut children, mut couples) = (0, 0, 0);
    for x in everyone(w) {
        let Some(arrival) = w.arrival(x) else {
            assert!(!w.is_immigrant(x));
            continue;
        };
        assert!(w.is_immigrant(x) && !w.is_founder(x));
        assert!(
            w.birth(x) < arrival && arrival < w.death(x),
            "{x} not alive at arrival"
        );
        let age = age_at(w, x, arrival);
        match w.mother(x) {
            // A child who arrived with the parents: born abroad, a minor,
            // same arrival as the mother, father her partner.
            Some(m) => {
                children += 1;
                assert!(age < 18.0, "{x} arrives with its mother aged {age:.1}");
                assert_eq!(
                    w.arrival(m),
                    Some(arrival),
                    "{x} arrives apart from its mother"
                );
                let f = w.father(x).expect("an arriving child has its father");
                assert_eq!(w.union(m).map(|u| u.partner), Some(f));
                assert_eq!(
                    w.arrival(f),
                    Some(arrival),
                    "{x} arrives apart from its father"
                );
            }
            None => {
                adults += 1;
                assert!(w.father(x).is_none());
                assert!(age >= 17.0, "{x} arrives alone aged {age:.1}");
            }
        }
        if let Some(u) = w.union(x) {
            let woman = if w.sex(x) == Sex::Female {
                x
            } else {
                u.partner
            };
            if arrived_together(w, woman, &u) {
                couples += 1;
                assert_eq!(
                    w.arrival(u.partner),
                    Some(arrival),
                    "{x} arrives apart from partner"
                );
                assert!(
                    u.start < w.death(x) && u.end > arrival,
                    "{x}'s union ends abroad"
                );
            } else {
                assert!(u.start > arrival, "{x} partners before arriving");
            }
        }
        for c in w.children(x) {
            let born_abroad = w.birth(c) < arrival;
            assert_eq!(born_abroad, w.is_immigrant(c), "{x}'s child {c}");
        }
    }
    assert!(adults > 1000, "only {adults} adult immigrants");
    assert!(
        children > 100,
        "only {children} children arrived with parents"
    );
    assert!(couples > 200, "only {couples} people arrived in couples");
}

#[test]
fn same_sex_couples_exist_for_both_sexes_and_have_no_joint_children() {
    let w = world();
    let (mut couples, mut same) = (0u32, [0u32; 2]);
    for x in everyone(w) {
        let Some(u) = w.union(x) else { continue };
        couples += 1;
        if w.sex(x) != w.sex(u.partner) {
            continue;
        }
        same[w.sex(x) as usize] += 1;
        // A same-sex union plans no births: a man's children are none, and
        // a woman's are her own, never fathered by a woman.
        match w.sex(x) {
            Sex::Male => assert!(w.children(x).is_empty(), "{x}"),
            Sex::Female => {
                for c in w.children(x) {
                    assert_eq!(w.mother(c), Some(x));
                    assert!(w.father(c).is_none_or(|f| w.sex(f) == Sex::Male));
                }
            }
        }
    }
    // The tiny world boosts the same-sex share tenfold (about 5%) so these
    // exhaustive checks cover same-sex couples; the realistic share is a
    // realism check at prototype scale.
    let share = (same[0] + same[1]) as f64 / couples as f64;
    assert!(
        same.iter().all(|&n| n > 50),
        "same-sex partnered people {same:?}"
    );
    assert!((0.01..0.10).contains(&share), "same-sex share {share:.4}");
}

#[test]
fn nobody_partners_close_kin() {
    let w = world();
    let mut related = 0;
    // Every couple, of any sexes, from both sides.
    for x in everyone(w) {
        let Some(u) = w.union(x) else { continue };
        let m = u.partner;
        let same_mother = w.mother(x).is_some() && w.mother(x) == w.mother(m);
        let same_father = w.father(x).is_some() && w.father(x) == w.father(m);
        let parent_child = w.mother(m) == Some(x)
            || w.father(m) == Some(x)
            || w.mother(x) == Some(m)
            || w.father(x) == Some(m);
        if same_mother || same_father || parent_child {
            related += 1;
        }
    }
    assert_eq!(related, 0, "{related} partnered pairs are close kin");
}

#[test]
fn people_per_block_match_the_ledger() {
    let w = world();
    let ledger = w.ledger();
    let block = |x: PersonId| {
        ledger
            .block_of(w.birth_year(x), w.lineage_group(x))
            .expect("every person is in a block")
    };
    let mut per_block: HashMap<u32, (u64, u64)> = HashMap::new();
    // (cell year, kind, dissolution class, left block, right block, left
    // sex), counted from the left-role member of each couple. An
    // opposite-sex couple's class is its separation year after the cell
    // year (0: none); same-sex cells record class 0, their dissolution being
    // a keyed draw.
    let mut unions: HashMap<(i32, CellKind, u8, u32, u32, Sex), u64> = HashMap::new();
    for x in everyone(w) {
        let e = per_block.entry(block(x)).or_default();
        e.0 += 1;
        if w.sex(x) == Sex::Female {
            e.1 += 1;
        }
        // Both unions (R1c): each is counted from its left-role member.
        for (cell, u) in w.union_cells(x).into_iter().zip(w.unions(x)) {
            let (Some((year, kind)), Some(u)) = (cell, u) else {
                continue;
            };
            let left = match kind {
                CellKind::SameLeft => true,
                CellKind::SameRight => false,
                _ => w.sex(x) == Sex::Female,
            };
            if left {
                let class = match (kind.same_sex(), u.separation) {
                    (false, Some(s)) => (year_of(s) - year) as u8,
                    _ => 0,
                };
                *unions
                    .entry((year, kind, class, block(x), block(u.partner), w.sex(x)))
                    .or_default() += 1;
            }
        }
    }
    // The ledger's unions by the same key; in area mode a block has a cell
    // per area for the same year, kind and class, so cells add up.
    let mut recorded: HashMap<(i32, CellKind, u8, u32, u32, Sex), u64> = HashMap::new();
    for (bi, b) in ledger.blocks.iter().enumerate() {
        assert_eq!(
            per_block.get(&(bi as u32)).copied().unwrap_or((0, 0)),
            (b.size, b.females),
            "block {} region {}",
            b.year,
            b.region
        );
        let left_cells = b
            .union_f
            .iter()
            .filter(|c| c.kind != CellKind::SameRight)
            .map(|c| (c, Sex::Female))
            .chain(
                b.union_m
                    .iter()
                    .filter(|c| c.kind == CellKind::SameLeft)
                    .map(|c| (c, Sex::Male)),
            );
        for (c, sex) in left_cells {
            for &(pb, n) in &c.partners {
                *recorded
                    .entry((c.year, c.kind, c.class, bi as u32, pb, sex))
                    .or_default() += n;
            }
        }
    }
    for (&(year, kind, class, b, pb, sex), &n) in &recorded {
        assert_eq!(
            unions
                .get(&(year, kind, class, b, pb, sex))
                .copied()
                .unwrap_or(0),
            n,
            "unions {year} ({kind:?}, class {class}): block {b} × block {pb}"
        );
    }
    assert_eq!(
        recorded.values().sum::<u64>(),
        unions.values().sum::<u64>(),
        "unrecorded unions"
    );
}

#[test]
fn children_inherit_the_mothers_group_and_unions_cross_groups() {
    let w = world();
    let (mut cross, mut unions) = (0u64, 0u64);
    // In area mode a block's region is the area its members grow up in,
    // which can differ from the mother's (she moved); heritage is always
    // the mother's.
    let areas = w.ledger().params.places.by_area;
    let mut directions = vec![0u64; w.ledger().params.region_count()];
    // Cross-heritage unions, by the woman's heritage.
    let mut mixed = vec![0u64; w.ledger().params.heritage_count()];
    for x in everyone(w) {
        if let Some(m) = w.mother(x) {
            if areas {
                assert_eq!(w.heritage(x), w.heritage(m), "{x}'s heritage");
            } else {
                assert_eq!(
                    (w.lineage_group(x), w.region(x), w.heritage(x)),
                    (w.lineage_group(m), w.region(m), w.heritage(m)),
                    "{x} is not in its mother's group"
                );
            }
        }
        if w.sex(x) != Sex::Female {
            continue;
        }
        if let Some(u) = w.union(x) {
            unions += 1;
            if w.region(x) != w.region(u.partner) {
                cross += 1;
                directions[w.region(x) as usize] += 1;
            }
            if w.heritage(x) != w.heritage(u.partner) {
                mixed[w.heritage(x).index()] += 1;
            }
        }
    }
    // The tiny world floors every heritage at 8% and boosts the open
    // market, so every group's women have cross-heritage unions.
    assert!(
        mixed.iter().all(|&m| m > 20),
        "cross-heritage unions: {mixed:?}"
    );
    let share = cross as f64 / unions as f64;
    if areas {
        // Partners grew up in different areas: the national share, those
        // who moved before partnering, and small areas' unions spilling
        // over. Crossings run both ways in most areas.
        assert!(
            (0.05..=0.70).contains(&share),
            "cross-area share {share:.3}"
        );
        let both = directions.iter().filter(|&&d| d > 0).count();
        assert!(
            both * 2 > directions.len(),
            "crossings from {both} of {} areas",
            directions.len()
        );
    } else {
        // ρ runs 0.25–0.35 over the tiny world's years and half of
        // national unions cross regions (12–18%); the open market (boosted
        // four-fold in the tiny world) mixes regions as well, so 15–35% in
        // all.
        assert!(
            (0.06..=0.40).contains(&share),
            "cross-region share {share:.3}"
        );
        assert!(
            directions.iter().all(|&d| d > 50),
            "one-way crossings: {directions:?}"
        );
    }
}

#[test]
fn independently_built_worlds_agree() {
    // Same parameters and seed, built twice: every answer identical (G1).
    let fresh = World::build(params(), 7);
    let w = world();
    for x in (0..w.population() as PersonId).step_by(37) {
        assert_eq!(fresh.sex(x), w.sex(x));
        assert_eq!((fresh.birth(x), fresh.death(x)), (w.birth(x), w.death(x)));
        assert_eq!(
            (fresh.mother(x), fresh.father(x)),
            (w.mother(x), w.father(x))
        );
        assert_eq!(fresh.union(x), w.union(x));
        assert_eq!(fresh.children(x), w.children(x));
    }
    // A different seed gives a different world of the same shape: among
    // people partnered in both worlds, partners almost never coincide.
    let other = World::build(params(), 8);
    let n = w.population().min(other.population()) as PersonId;
    let (mut both, mut same) = (0, 0);
    for x in (0..n).step_by(11) {
        if let (Some(a), Some(b)) = (w.union(x), other.union(x)) {
            both += 1;
            same += (a.partner == b.partner) as u32;
        }
    }
    assert!(
        both > 500,
        "too few people partnered in both worlds ({both})"
    );
    assert!(
        same * 20 < both,
        "seeds 7 and 8 share {same} of {both} partners"
    );
}

#[test]
fn couples_move_between_areas_and_their_children_grow_up_there() {
    // Area mode (stage 3): a union's move comes from the woman's plan leaf;
    // a child is in the block of their area of upbringing (where the family
    // lives at 18), so a child under 18 at the move is in the destination's
    // block. Both partners see the same move.
    let w = world();
    if !w.ledger().params.places.by_area {
        assert!(everyone(w).all(|x| w.couple_moves(x).iter().all(Option::is_none)));
        return;
    }
    let (mut unions, mut moved, mut later) = (0u64, 0u64, 0u64);
    for x in everyone(w) {
        if w.sex(x) != Sex::Female {
            continue;
        }
        for (u, mv) in w.unions(x).iter().zip(w.couple_moves(x)) {
            let Some(u) = u else { continue };
            unions += 1;
            let Some((dest, t)) = mv else { continue };
            moved += 1;
            assert!(u.start < t, "{x} moves before the union");
            assert!(u.separation.is_none_or(|s| t < s), "{x} moves after separating");
            if w.death(u.partner) > t {
                let theirs = w.unions(u.partner);
                let k = theirs
                    .iter()
                    .position(|v| v.is_some_and(|v| v.partner == x && v.start == u.start))
                    .expect("the partner holds the union");
                assert_eq!(w.couple_moves(u.partner)[k], Some((dest, t)), "{x}'s partner");
            }
            for c in w.children(x) {
                // This union's plan births (not a later union's with the
                // same partner, nor a non-union birth, which follows the
                // mother's cohort: debt) under 18 in the move year.
                let ours =
                    w.union_birth(c) && u.start <= w.birth(c) && w.birth(c) < u.end + 300 * DAY;
                // ... and before any separation by their 18th year.
                let intact = u.separation.is_none_or(|s| year_of(s) > year_of(w.birth(c)) + 18);
                if w.father(c) == Some(u.partner)
                    && ours
                    && intact
                    && year_of(w.birth(c)) + 18 >= year_of(t)
                {
                    later += 1;
                    assert_eq!(w.region(c), dest, "{x}'s child {c}, under 18 at the move");
                }
            }
        }
    }
    let share = moved as f64 / unions as f64;
    println!("{moved} of {unions} unions move ({share:.3}); {later} children under 18 at the move");
    assert!(
        (0.02..0.6).contains(&share) && later > 20,
        "{moved} of {unions} unions move ({share:.3}); {later} children under 18 at the move"
    );
}

#[test]
fn women_partner_in_the_area_the_ledger_counts_them_in() {
    // Area mode (stage 4): a union's cell area is the woman's market area,
    // so just before it she is counted there, except a migrant partnering
    // in her move year before her move date (the ledger counts the whole
    // year in the destination; residence forms such unions afresh there).
    let w = world();
    if !w.ledger().params.places.by_area {
        return;
    }
    let (mut checked, mut window) = (0u64, 0u64);
    for x in everyone(w) {
        if w.sex(x) != Sex::Female {
            continue;
        }
        let areas = w.union_areas(x);
        for (k, (u, cell)) in w.unions(x).iter().zip(w.union_cells(x)).enumerate() {
            let (Some(u), Some((_, kind))) = (u, cell) else { continue };
            if kind == CellKind::Arrival || u.start <= w.birth(x) || w.is_founder(x) {
                continue;
            }
            if kind.same_sex() && w.sex(u.partner) == Sex::Female && kind == CellKind::SameRight {
                continue;
            }
            let before = w.area_at(x, u.start - 1);
            let area = areas[k].expect("a union has an area");
            checked += 1;
            if before != area {
                let in_window = k == 0
                    && w.migration_class(x) == Some((area, year_of(u.start)))
                    && w.migration(x).is_none();
                window += 1;
                assert!(in_window, "{x}'s union {k} in {area}, counted in {before} before");
            }
        }
    }
    println!("{checked} unions checked, {window} in a migrant's move year before the move");
    assert!(window * 50 < checked, "{window} of {checked} unions form outside the woman's area");
}
