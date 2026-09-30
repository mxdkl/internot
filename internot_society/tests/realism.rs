//! Realism sanity bands and sampled duality on the prototype-scale world
//! (1840–2100, ~4.5M people ever born). The bands are the R1 prototype's
//! (plan §"Goal and pass criteria", item 5), not the spec §15 targets, which
//! need the production rate tables.

use std::sync::OnceLock;

use internot_society::{Params, PersonId, Sex, World};

const YEAR_S: f64 = 365.25 * 86_400.0;

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| World::build(Params::prototype(), 42))
}

struct Cohort {
    e0_f: f64,
    cfr: f64,
    ever_partnered: f64,
    median_union_age: f64,
    mean_gap: f64,
}

/// Statistics over a sample of the natives born in `year`, all regions.
/// Immigrants arrive single until R1b-2 step B, so their unions and
/// fertility are checked separately once that step lands.
fn cohort(year: i32) -> Cohort {
    let w = world();
    let ledger = w.ledger();
    let blocks = ledger.blocks_of_year(year);
    let (base, end) = (
        ledger.base[blocks.start as usize] as u32,
        ledger.base[blocks.end as usize] as u32,
    );
    let step = ((end - base) / 3000).max(1) as usize;
    let (mut life_f, mut nf, mut women, mut kids, mut partnered, mut gaps) =
        (0.0, 0u32, 0u32, 0u32, 0u32, 0.0);
    let mut ages = Vec::new();
    for id in (base..end)
        .step_by(step)
        .filter(|&id| w.sex(id) == Sex::Female && !w.is_immigrant(id))
    {
        let life = (w.death(id) - w.birth(id)) as f64 / YEAR_S;
        life_f += life;
        nf += 1;
        if life >= 15.0 {
            women += 1;
            kids += w.children(id).len() as u32;
            if let Some(u) = w.union(id) {
                partnered += 1;
                ages.push((u.start - w.birth(id)) as f64 / YEAR_S);
                gaps += (w.birth(id) - w.birth(u.partner)) as f64 / YEAR_S;
            }
        }
    }
    ages.sort_by(|a, b| a.total_cmp(b));
    Cohort {
        e0_f: life_f / nf as f64,
        cfr: kids as f64 / women as f64,
        ever_partnered: partnered as f64 / women as f64,
        median_union_age: ages[ages.len() / 2],
        mean_gap: gaps / partnered as f64,
    }
}

fn within(name: &str, v: f64, lo: f64, hi: f64) {
    assert!(
        (lo..=hi).contains(&v),
        "{name} = {v:.2}, expected {lo}..={hi}"
    );
}

#[test]
fn cohort_life_expectancy_rises_through_history() {
    within("e0_f 1900", cohort(1900).e0_f, 54.0, 63.0);
    within("e0_f 1950", cohort(1950).e0_f, 74.0, 81.0);
    within("e0_f 2010", cohort(2010).e0_f, 81.0, 88.0);
}

#[test]
fn fertility_follows_the_eras() {
    within("CFR 1860", cohort(1860).cfr, 2.8, 4.5);
    within("CFR 1930", cohort(1930).cfr, 2.3, 3.3);
    within("CFR 1950", cohort(1950).cfr, 1.8, 2.5);
    within("CFR 1990", cohort(1990).cfr, 1.5, 2.1);
}

#[test]
fn union_timing_and_ages_are_plausible() {
    // Mean age gap: CPS-era kernel (about +2.1 years), but 1900 runs
    // wider, as it did (Census 1900 medians at first marriage: men 25.9,
    // women 21.9), because mass immigration was male-heavy.
    for (year, lo, hi, gap_hi) in [
        (1900, 20.0, 23.5, 4.2),
        (1950, 19.0, 22.0, 3.0),
        (1990, 23.0, 26.0, 3.0),
    ] {
        let c = cohort(year);
        within(
            &format!("median union age {year}"),
            c.median_union_age,
            lo,
            hi,
        );
        within(
            &format!("ever partnered {year}"),
            c.ever_partnered,
            0.80,
            0.97,
        );
        within(&format!("mean age gap {year}"), c.mean_gap, 1.5, gap_hi);
    }
}

#[test]
fn foreign_born_share_follows_the_census() {
    // Share of the living born abroad, against the US Census (Gibson and
    // Jung 2006; ACS 2020): 13.6% (1900), 4.7% (1970), 13.7% (2020). The
    // prototype's rates are net anchors, so the bands are ±4 points.
    let w = world();
    for (year, census) in [(1900, 13.6), (1970, 4.7), (2020, 13.7)] {
        let t = internot_society::world::year_start(year) + 180 * 86_400;
        let (mut alive, mut foreign) = (0u32, 0u32);
        for id in (0..w.population() as PersonId).step_by(53) {
            if w.alive_at(id, t) {
                alive += 1;
                foreign += w.arrival(id).is_some_and(|a| a <= t) as u32;
            }
        }
        let share = 100.0 * foreign as f64 / alive as f64;
        within(
            &format!("foreign-born {year}"),
            share,
            census - 4.0,
            census + 4.0,
        );
    }
}

#[test]
fn sampled_duality_at_scale() {
    let w = world();
    let n = w.population();
    let key = procedural_core::key::Key::from_seed(99);
    for i in 0..20_000u64 {
        let x = key.with(i).below(n) as PersonId;
        if let Some(u) = w.union(x) {
            let v = w.union(u.partner).unwrap();
            assert_eq!((v.partner, v.start, v.end), (x, u.start, u.end));
        }
        if let Some(m) = w.mother(x) {
            assert!(w.children(m).contains(&x));
            let b = w.birth(x);
            assert!(w.birth(m) < b && b < w.death(m));
        }
        if let Some(f) = w.father(x) {
            assert!(w.children(f).contains(&x));
        }
        for c in w.children(x) {
            match w.sex(x) {
                Sex::Female => assert_eq!(w.mother(c), Some(x)),
                Sex::Male => assert_eq!(w.father(c), Some(x)),
            }
        }
    }
}
