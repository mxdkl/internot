//! Residence (L4) realism on the prototype world with the `us` pack's real
//! places, against the targets in
//! `research/2026-10-01-residence-data-and-targets.md` §2.
//! Run: cargo run --release -p internot_society --example residence_realism [pack]

use std::time::Instant;

use internot_society::residence::{Places, Pos, Residence, AREA, COUNTY, TRACT, ZONE};
use internot_society::{Params, PersonId, World};
use procedural_core::geo::haversine_miles;
use procedural_core::stream::{year_start, DAY};

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

/// Census region of a state FIPS code: 0 Northeast, 1 Midwest, 2 South, 3 West.
fn census_region(state: u32) -> usize {
    match state {
        9 | 23 | 25 | 33 | 44 | 50 | 34 | 36 | 42 => 0,
        17 | 18 | 26 | 39 | 55 | 19 | 20 | 27 | 29 | 31 | 38 | 46 => 1,
        10 | 11 | 12 | 13 | 24 | 37 | 45 | 51 | 54 | 1 | 21 | 28 | 47 | 5 | 22 | 40 | 48 => 2,
        _ => 3,
    }
}

fn main() {
    let t0 = Instant::now();
    // An optional argument names a pack in `worlds/` (default: the built-in `us`).
    let params = match std::env::args().nth(1) {
        Some(pack) => {
            let root = std::env::var("WORLDS").unwrap_or_else(|_| "worlds".into());
            Params::load(root.as_ref(), &pack).expect("a pack in worlds/ (or $WORLDS)")
        }
        None => Params::prototype(),
    };
    let w = World::build(params, 42);
    let places = Places::from_params(&w.ledger().params).expect("the us pack has places");
    let r = Residence::build(&w, places);
    println!("world and residence built in {:?}", t0.elapsed());
    let pl = r.places();
    let miles = |a: &Pos, b: &Pos| haversine_miles(pl.tract_at(a[TRACT]), pl.tract_at(b[TRACT]));
    let n = w.population();
    let mut x = 0x00c0_ffee_u64;
    let mut sample = |t: i64, k: usize, ok: &dyn Fn(PersonId) -> bool| -> Vec<PersonId> {
        let mut out = Vec::with_capacity(k);
        while out.len() < k {
            let id = (xorshift(&mut x) % n) as PersonId;
            if w.present_at(id, t) && ok(id) {
                out.push(id);
            }
        }
        out
    };

    // 1. Where people live: census-region shares of the living.
    println!("\n== census-region shares of the living (NE MW S W) ==");
    let census = [
        (1850, [37.2, 23.3, 38.7, 0.8]),
        (1900, [27.6, 34.6, 32.2, 5.7]),
        (1950, [26.1, 29.4, 31.2, 13.3]),
        (2000, [19.0, 22.9, 35.6, 22.5]),
        (2020, [17.4, 20.8, 38.1, 23.7]),
    ];
    for (year, target) in census {
        let t = year_start(year) + 90 * DAY;
        let mut by = [0usize; 4];
        let people = sample(t, 20_000, &|_| true);
        for &id in &people {
            let p = r.address_of(&w, id, t).expect("present");
            let (_, fips) = pl.county(p[COUNTY]).expect("real places");
            by[census_region(fips / 1000)] += 1;
        }
        let shares: Vec<String> = by
            .iter()
            .map(|&c| format!("{:.1}", 100.0 * c as f64 / people.len() as f64))
            .collect();
        println!("{year}: model {}  census {:?}", shares.join(" "), target);
    }

    // 1b. Natives living outside their birth state (HSUS C 1–14 and ACS
    //     B05002: 20.9% in 1900, 29.7% in 1960, 32.5% in 2000, 33.7% in 2024).
    println!("\n== natives living outside their birth state ==");
    for (year, target) in [
        (1900, 20.9),
        (1930, 23.8),
        (1960, 29.7),
        (2000, 32.5),
        (2020, 33.7),
    ] {
        let t = year_start(year) + 90 * DAY;
        let people = sample(t, 10_000, &|id| {
            !w.is_immigrant(id) && w.birth(id) >= year_start(w.ledger().params.y0)
        });
        let away = people
            .iter()
            .filter(|&&id| {
                let born = r
                    .address_of(&w, id, w.birth(id) + DAY)
                    .expect("present at birth");
                let now = r.address_of(&w, id, t).expect("present");
                let state = |p: &Pos| pl.county(p[COUNTY]).expect("real places").1 / 1000;
                state(&born) != state(&now)
            })
            .count();
        println!(
            "{year}: model {:.1}%  census {target}%",
            100.0 * away as f64 / people.len() as f64
        );
    }

    // 2. Nearest parent, adults 25+ with a living parent (PSID 2013).
    // PSID counts a spouse's parents too ("parents include in-laws").
    println!("\n== nearest parent or parent-in-law, adults 25+ in 2013 (PSID: coresident 5.9%, <30 mi 59.8%, 500+ mi 9.2%) ==");
    let t = year_start(2013) + 180 * DAY;
    let parents_of = |id: PersonId| -> Vec<PersonId> {
        let mut ps: Vec<PersonId> = [w.mother(id), w.father(id)].into_iter().flatten().collect();
        if let Some(p) = w.partner_at(id, t) {
            ps.extend([w.mother(p), w.father(p)].into_iter().flatten());
        }
        ps.retain(|&p| w.present_at(p, t));
        ps
    };
    let adults = sample(t, 10_000, &|id| {
        (t - w.birth(id)) >= 25 * 365 * DAY && !parents_of(id).is_empty()
    });
    let (mut co, mut near, mut far) = (0, 0, 0);
    // Diagnostic split: never partnered against ever partnered.
    let mut split = [[0usize; 3]; 2];
    for &id in &adults {
        let h = w.household(id, t);
        let mine = r.address_of(&w, id, t).unwrap();
        let mut best = f64::MAX;
        for p in parents_of(id) {
            if w.household(p, t) == h {
                best = -1.0;
                break;
            }
            best = best.min(miles(&mine, &r.address_of(&w, p, t).unwrap()));
        }
        let ever = w.unions(id).iter().flatten().any(|u| u.start <= t) as usize;
        split[ever][0] += 1;
        if best < 0.0 {
            co += 1;
        } else if best < 30.0 {
            near += 1;
            split[ever][1] += 1;
        } else if best >= 500.0 {
            far += 1;
            split[ever][2] += 1;
        }
    }
    for (i, label) in ["never partnered", "ever partnered"].iter().enumerate() {
        let [n, near, far] = split[i];
        println!(
            "    {label}: {n} people, under 30 mi {:.1}%, 500+ mi {:.1}%",
            100.0 * near as f64 / n.max(1) as f64,
            100.0 * far as f64 / n.max(1) as f64
        );
    }
    let pc = |c: usize| 100.0 * c as f64 / adults.len() as f64;
    println!(
        "coresident {:.1}%, under 30 mi {:.1}% (with coresident {:.1}%), 500+ mi {:.1}%",
        pc(co),
        pc(near),
        pc(co + near),
        pc(far)
    );

    // 3. From 16 to 26 (cohorts born 1990–1994; Chetty et al.: same tract
    //    30%, under 10 mi 58%, under 100 mi 80%, over 500 mi 10%).
    println!("\n== where people born 1990–1994 live at 26 against at 16 ==");
    let (mut same, mut ten, mut hundred, mut five, mut k) = (0, 0, 0, 0, 0);
    let t26 = year_start(2019) + 180 * DAY;
    let young = sample(t26, 8000, &|id| {
        (1990..=1994).contains(&w.birth_year(id)) && !w.is_immigrant(id)
    });
    for &id in &young {
        let at16 = w.birth(id) + 16 * 365 * DAY + 4 * DAY;
        let at26 = w.birth(id) + 26 * 365 * DAY + 6 * DAY;
        let (Some(a), Some(b)) = (r.address_of(&w, id, at16), r.address_of(&w, id, at26)) else {
            continue;
        };
        k += 1;
        let d = miles(&a, &b);
        same += (a[TRACT] == b[TRACT]) as usize;
        ten += (d < 10.0) as usize;
        hundred += (d < 100.0) as usize;
        five += (d > 500.0) as usize;
    }
    let pk = |c: usize| 100.0 * c as f64 / k as f64;
    println!("same tract {:.1}%, under 10 mi {:.1}%, under 100 mi {:.1}%, over 500 mi {:.1}% ({k} people)", pk(same), pk(ten), pk(hundred), pk(five));

    // 4. Movers in a year, and how far (CPS A-1: 1950 ~19.9%, 1970 19.1%,
    //    1990 17.9%, 2010 12.5%, 2019 9.8%; about 60% within the county).
    println!("\n== movers in the past year (people 1+) ==");
    for year in [1950, 1970, 1990, 2010, 2019] {
        let t = year_start(year) + 90 * DAY;
        let before = t - 365 * DAY;
        let people = sample(t, 20_000, &|id| w.present_at(id, before));
        let (mut moved, mut county, mut zone, mut area) = (0, 0, 0, 0);
        for &id in &people {
            let (a, b) = (
                r.address_of(&w, id, before).unwrap(),
                r.address_of(&w, id, t).unwrap(),
            );
            if a[TRACT] != b[TRACT] {
                moved += 1;
                if a[COUNTY] == b[COUNTY] {
                    county += 1;
                } else if a[ZONE] == b[ZONE] {
                    zone += 1;
                } else if a[AREA] == b[AREA] {
                    area += 1;
                }
            }
        }
        let p = |c: usize| 100.0 * c as f64 / people.len() as f64;
        println!(
            "{year}: moved {:.1}% (same county {:.1}%, other county same zone {:.1}%, other zone same area {:.1}%, other area {:.1}%)",
            p(moved),
            p(county),
            p(zone),
            p(area),
            p(moved - county - zone - area)
        );
    }
    println!("\nreport time {:?}", t0.elapsed());
}
