//! Heritage realism report (N1) for the prototype world, against the
//! targets in `docs/superpowers/research/2026-10-01-heritage-and-names.md`.
//! Run: cargo run --release -p internot_society --example heritage_report
//!
//! People are sampled uniformly from the id space. A person counts at a date
//! if present then (born or arrived, and alive).

use std::time::Instant;

use internot_society::world::{year_start, DAY};
use internot_society::{Params, PersonId, Sex, World};

/// The US pack's group ids, in the order of the Census targets below.
const IDS: [&str; 5] = ["white", "black", "aian", "asian", "hispanic"];

/// Census composition of the living population, in [`IDS`] order,
/// present-day borders; 2000–2020 renormalized without "two or more" and
/// "some other race".
const COMPOSITION: [(i32, [f64; 5]); 12] = [
    (1850, [82.38, 15.42, 1.70, 0.00, 0.50]),
    (1870, [85.80, 12.56, 0.81, 0.16, 0.67]),
    (1900, [87.08, 11.59, 0.35, 0.32, 0.66]),
    (1930, [88.23, 9.65, 0.29, 0.45, 1.38]),
    (1950, [87.17, 9.94, 0.25, 0.45, 2.14]),
    (1970, [83.47, 10.88, 0.41, 0.76, 4.46]),
    (1980, [79.57, 11.52, 0.63, 1.57, 6.45]),
    (1990, [75.64, 11.75, 0.72, 2.80, 8.99]),
    (2000, [70.40, 12.28, 0.75, 3.79, 12.78]),
    (2010, [65.10, 12.50, 0.70, 5.00, 16.70]),
    (2020, [60.60, 12.60, 0.70, 6.40, 19.60]),
    (2040, [f64::NAN; 5]),
];

/// Census composition of the foreign-born (Gibson & Jung 2006, Table 9/10).
const FOREIGN_BORN: [(i32, [f64; 5]); 4] = [
    (1970, [73.4, 2.6, 0.2, 5.7, 18.7]),
    (1980, [49.4, 5.8, 0.3, 15.5, 29.6]),
    (1990, [31.2, 7.4, 0.2, 23.1, 39.7]),
    (2000, [22.0, 6.8, 0.4, 22.7, 45.5]),
];

const SAMPLE: usize = 200_000;

fn main() {
    let t0 = Instant::now();
    let w = World::build(Params::prototype(), 42);
    println!("world build {:?}", t0.elapsed());
    let n = w.population();
    let groups = &w.ledger().params.heritage.groups;
    let heritages = groups.len();
    let names: Vec<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    // A target for group `h`, if the pack has a group with that id.
    let target = |row: &[f64; 5], h: usize| {
        IDS.iter()
            .position(|&id| id == groups[h].id)
            .map_or(f64::NAN, |i| row[i])
    };

    println!("\nliving population by heritage, % (census in brackets)");
    println!("year   {}", names.join("            "));
    for &(year, census) in &COMPOSITION {
        let t = year_start(year) + 180 * DAY;
        let mut c = vec![0u64; heritages];
        let mut fb = vec![0u64; heritages];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ year as u64;
        let mut seen = 0;
        while seen < SAMPLE {
            x = xorshift(x);
            let id = (x % n) as PersonId;
            if !w.present_at(id, t) {
                continue;
            }
            seen += 1;
            let h = w.heritage(id).index();
            c[h] += 1;
            if w.is_immigrant(id) {
                fb[h] += 1;
            }
        }
        let row: Vec<String> = (0..heritages)
            .map(|h| {
                format!(
                    "{:>5.2} ({:>5.2})",
                    pct(c[h], seen as u64),
                    target(&census, h)
                )
            })
            .collect();
        println!("{year}   {}", row.join("  "));
        if let Some((_, fbc)) = FOREIGN_BORN.iter().find(|(y, _)| *y == year) {
            let total: u64 = fb.iter().sum();
            let row: Vec<String> = (0..heritages)
                .map(|h| format!("{:>5.2} ({:>5.2})", pct(fb[h], total), target(fbc, h)))
                .collect();
            println!("  foreign-born   {}", row.join("  "));
        }
    }

    intermarriage(&w);
    println!("\nreport time {:?}", t0.elapsed());
}

/// Cross-heritage share of unions begun in each window, by the partner's
/// heritage and sex, and of all couples together at a date.
fn intermarriage(w: &World) {
    let n = w.population();
    println!("\nnew unions (opposite-sex) with a partner of another heritage, % by heritage and sex (women / men)");
    let groups = &w.ledger().params.heritage.groups;
    let header: String = groups.iter().map(|g| format!("{:<13}", g.id)).collect();
    println!("years       all    {header}");
    for (from, to) in [
        (1880, 1900),
        (1920, 1940),
        (1950, 1960),
        (1965, 1970),
        (1978, 1982),
        (1988, 1992),
        (1998, 2002),
        (2008, 2012),
        (2013, 2017),
    ] {
        // [heritage][sex] = (cross, total)
        let mut c = vec![[(0u64, 0u64); 2]; groups.len()];
        let (lo, hi) = (year_start(from), year_start(to + 1));
        let mut x: u64 = 0x5EED_0F57_A7A7_u64 ^ from as u64;
        let mut seen = 0;
        while seen < SAMPLE * 2 {
            x = xorshift(x);
            let id = (x % n) as PersonId;
            seen += 1;
            for u in w.unions(id).into_iter().flatten() {
                if u.start < lo || u.start >= hi || w.sex(u.partner) == w.sex(id) {
                    continue;
                }
                let e = &mut c[w.heritage(id).index()][w.sex(id) as usize];
                e.0 += (w.heritage(id) != w.heritage(u.partner)) as u64;
                e.1 += 1;
            }
        }
        let all: (u64, u64) = c
            .iter()
            .flatten()
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let row: String = c
            .iter()
            .map(|s| {
                format!(
                    "{:>5.1}/{:<5.1}  ",
                    pct(s[Sex::Female as usize].0, s[Sex::Female as usize].1),
                    pct(s[Sex::Male as usize].0, s[Sex::Male as usize].1)
                )
            })
            .collect();
        // Each couple is seen from both sides, so `all` is the couple share.
        println!("{from}-{to}  {:>5.1}  {row}", pct(all.0, all.1));
    }
    println!("pew 2015 newlyweds: all 17; white 11 (m 12 / f 10); black 18 (m 24 / f 12); hispanic 27 (26/28); asian 29 (21/36); 1967: 3; 1980: 7");
}

fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

fn pct(a: u64, b: u64) -> f64 {
    100.0 * a as f64 / b.max(1) as f64
}
