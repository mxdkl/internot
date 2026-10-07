//! Residence on the monotone world: example addresses over a life, lookup
//! cost (cold and warm), and realism against the design-A targets: movers a
//! year, people living outside their birth state, adults 25+ with a parent
//! within 30 miles, distance at 26 from where they lived at 16.
//! `MULT` (default 0.05), `BLIND=1`.
use std::time::Instant;

use internot_society::mono::{Mono, Pid, Pos, AREA, COUNTY, TRACT};
use internot_society::Params;
use procedural_core::geo::haversine_miles;
use procedural_core::key::Key;
use procedural_core::stream::year_start;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let p = Params::embedded("us").unwrap();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let s = Instant::now();
    let places = m.places();
    println!("places built in {:.0} ms", s.elapsed().as_secs_f64() * 1e3);
    let state = |pos: &Pos| places.county(pos[COUNTY]).1 / 1000;
    let show = |pos: &Pos| {
        let (county, fips) = places.county(pos[COUNTY]);
        format!("{county} ({:02}), tract {}", fips / 1000, places.tract_geoid(pos[TRACT]))
    };
    let year_t = |y: i32| year_start(y) + 120 * 86_400;

    // A life.
    let x = Pid { cell: 0, y: 1960, i: 31_337 % m.cohort_n(0, 1960) };
    println!("\n{} (born {}):", m.full_name(x, year_t(2023)), 1960);
    for y in [1960, 1970, 1978, 1985, 1995, 2005, 2015, 2023] {
        if let (Some(a), Some(h)) = (m.address_of(x, year_t(y)), m.home(x, year_t(y))) {
            let hh = m.household(x, year_t(y)).unwrap();
            let members: Vec<String> = m.members(hh, year_t(y)).iter().map(|&q| m.first_name(q).to_string()).collect();
            println!("  {y}: {} — {} [{}]", h.line(), show(&a), members.join(", "));
        }
    }
    // Household members share the address.
    let mut shared = (0, 0);
    for j in 0..2000u64 {
        let c = (j % m.cells() as u64) as u16;
        let y = 1930 + (j % 80) as i32;
        let size = m.cohort_n(c, y);
        if size == 0 {
            continue;
        }
        let x = Pid { cell: c, y, i: Key::from_seed(9).with(j).below(size) };
        let t = year_t(2010);
        let Some(h) = m.household(x, t) else { continue };
        let line = m.home(x, t).unwrap().line();
        for &q in m.members(h, t).iter() {
            shared.1 += 1;
            shared.0 += (m.home(q, t).unwrap().line() == line) as u32;
        }
    }
    println!("household members sharing the address: {} of {}", shared.0, shared.1);

    // People alive at 2023.
    let t = year_t(2023);
    let mut people = Vec::new();
    for c in 0..m.cells() {
        for y in m.first_year()..=2023 {
            m.alive_in_cohort(c, y, t, &mut |x| people.push(x));
        }
    }
    let xs: Vec<Pid> = (0..20_000).map(|j| people[Key::from_seed(2).with(j).below(people.len() as u64) as usize]).collect();
    for pass in ["cold", "warm"] {
        let mut v: Vec<u64> = xs
            .iter()
            .map(|&x| {
                let s = Instant::now();
                std::hint::black_box(m.address_of(x, t));
                s.elapsed().as_nanos() as u64
            })
            .collect();
        v.sort_unstable();
        println!("address_of at 2023, {pass}: p50 {:.1} µs, p99 {:.1} µs, mean {:.1} µs", v[v.len() / 2] as f64 / 1e3, v[v.len() * 99 / 100] as f64 / 1e3, v.iter().sum::<u64>() as f64 / v.len() as f64 / 1e3);
    }
    for (name, bytes) in m.memory_report().iter().filter(|r| r.0.starts_with("residence")) {
        println!("  {name}: {:.2} MB", *bytes as f64 / 1e6);
    }

    // Realism.
    println!("\nyear  moved in last year  outside birth state  25+ parent <30 mi   (targets: movers 20% 1948-70, 7.8% 2023; outside birth state 21-34%; parent within 30 mi 59.8%)");
    for year in [1900, 1950, 1980, 2000, 2023] {
        let t = year_t(year);
        let mut alive = Vec::new();
        for c in 0..m.cells() {
            for y in m.first_year()..=year {
                m.alive_in_cohort(c, y, t, &mut |x| alive.push(x));
            }
        }
        let k = alive.len().min(6000);
        let (mut moved, mut n_moved, mut outside, mut n_out, mut near, mut n_near) = (0, 0, 0, 0, 0, 0);
        for j in 0..k {
            let x = alive[Key::from_seed(3).with2(year as u64, j as u64).below(alive.len() as u64) as usize];
            let Some(now) = m.address_of(x, t) else { continue };
            if m.age_at(x, t) >= 1.0 {
                if let Some(before) = m.address_of(x, t - 365 * 86_400) {
                    n_moved += 1;
                    moved += (before[TRACT] != now[TRACT]) as u32;
                }
            }
            if let Some(bp) = m.birth_place(x) {
                n_out += 1;
                outside += (state(&bp) != state(&now)) as u32;
            }
            if m.age_at(x, t) >= 25.0 {
                let parents: Vec<Pid> = m.parents(x).map_or(vec![], |(mo, _, u)| std::iter::once(mo).chain(u.map(|u| u.husband)).collect());
                let alive_parents: Vec<Pos> = parents.iter().filter(|&&q| m.present_at(q, t)).filter_map(|&q| m.address_of(q, t)).collect();
                if !alive_parents.is_empty() {
                    n_near += 1;
                    let here = places.centre(TRACT, now[TRACT]);
                    near += alive_parents.iter().any(|pp| haversine_miles(here, places.centre(TRACT, pp[TRACT])) < 30.0) as u32;
                }
            }
        }
        let pc = |a: u32, n: u32| 100.0 * a as f64 / n.max(1) as f64;
        println!("{year}  {:17.1}%  {:18.1}%  {:16.1}%", pc(moved, n_moved), pc(outside, n_out), pc(near, n_near));
    }
    // 16 to 26: born 1990–94.
    let (mut n, mut within100) = (0, 0);
    for c in 0..m.cells() {
        for y in 1990..1995 {
            let size = m.cohort_n(c, y);
            for j in 0..size.min(1500) {
                let x = Pid { cell: c, y, i: Key::from_seed(5).with3(c as u64, y as u64, j).below(size) };
                let (Some(a16), Some(a26)) = (m.address_of(x, year_t(y + 16)), m.address_of(x, year_t(y + 26))) else { continue };
                n += 1;
                within100 += (haversine_miles(places.centre(TRACT, a16[TRACT]), places.centre(TRACT, a26[TRACT])) < 100.0) as u32;
            }
        }
    }
    println!("born 1990-94: within 100 mi at 26 of where they lived at 16: {:.1}% (target 80%)", 100.0 * within100 as f64 / n.max(1) as f64);
    let areas = (0..places.count(AREA) as u32).map(|a| places.area_name(a)).take(5).collect::<Vec<_>>().join(", ");
    println!("areas: {} … ({} areas)", areas, places.count(AREA));
}
