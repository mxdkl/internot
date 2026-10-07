//! Names on the monotone world: examples by group and era, lookup cost, and
//! checks against the name tables (top first names of a year against SSA's,
//! surnames by group against Census 2020's, married women's surnames by
//! era). `MULT` (default 1), `BLIND=1` for the heritage-blind world.
use std::collections::HashMap;
use std::time::Instant;

use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let p = Params::embedded("us").unwrap();
    let blind = std::env::var("BLIND").is_ok_and(|v| v == "1");
    let m = if blind { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let group = |x: Pid| m.heritage_name(x).unwrap_or("all");
    let pick = |c: u16, y: i32, k: u64| Pid { cell: c, y, i: Key::from_seed(3).with3(c as u64, y as u64, k).below(m.cohort_n(c, y).max(1)) };

    println!("examples (name at 2023, or at death):");
    for c in 0..m.cells() {
        for y in [1880, 1930, 1965, 1995, 2015] {
            if m.cohort_n(c, y) == 0 {
                continue;
            }
            let names: Vec<String> = (0..3)
                .map(|k| {
                    let x = pick(c, y, k);
                    let t = year_start(2023).min(m.death(x) - 1);
                    let born = m.surname_text(m.birth_surname(x));
                    let now = m.full_name(x, t);
                    if now.ends_with(&born) { now } else { format!("{now} (born {born})") }
                })
                .collect();
            println!("  {:>10} {y}: {}", group(pick(c, y, 0)), names.join("; "));
        }
    }

    // Cost: cold (first use of each year builds its factors) and warm.
    let ever: Vec<(u16, i32, u64)> = (0..m.cells()).flat_map(|c| (m.first_year()..=2023).map(move |y| (c, y))).map(|(c, y)| (c, y, m.cohort_n(c, y))).collect();
    let total: u64 = ever.iter().map(|e| e.2).sum();
    let draw = |k: u64| {
        let mut r = Key::from_seed(9).with(k).below(total);
        for &(cell, y, n) in &ever {
            if r < n {
                return Pid { cell, y, i: r };
            }
            r -= n;
        }
        unreachable!()
    };
    let xs: Vec<Pid> = (0..20_000).map(draw).collect();
    for pass in ["cold", "warm"] {
        let s = Instant::now();
        let h: usize = xs.iter().map(|&x| m.first_name(x).len()).sum();
        let first = s.elapsed().as_secs_f64() / xs.len() as f64;
        let s = Instant::now();
        let h2: usize = xs.iter().map(|&x| m.full_name(x, year_start(2023)).len()).sum();
        let full = s.elapsed().as_secs_f64() / xs.len() as f64;
        println!("{pass}: first name {:.2} µs, full name {:.2} µs per person ({h} {h2})", first * 1e6, full * 1e6);
    }
    let mut v: Vec<u64> = xs.iter().map(|&x| {
        let s = Instant::now();
        std::hint::black_box(m.full_name(x, year_start(2023)));
        s.elapsed().as_nanos() as u64
    }).collect();
    v.sort_unstable();
    println!("full name p50 {:.2} µs, p99 {:.2} µs, max {:.1} µs", v[v.len() / 2] as f64 / 1e3, v[v.len() * 99 / 100] as f64 / 1e3, *v.last().unwrap() as f64 / 1e3);

    // Where a full name's time goes (warm).
    let t23 = year_start(2023);
    let parts: Vec<(&str, Box<dyn Fn(Pid) -> usize>)> = vec![
        ("mother", Box::new(|x| m.mother(x).is_some() as usize)),
        ("father", Box::new(|x| m.father(x).is_some() as usize)),
        ("spouse", Box::new(|x| m.spouse(x).is_some() as usize)),
        ("marriage_date", Box::new(|x| m.marriage_date(x).is_some() as usize)),
        ("naming_group", Box::new(|x| m.naming_group(x).map_or(0, |h| h.0 as usize))),
        ("first_name", Box::new(|x| m.first_name(x).len())),
        ("middle_name", Box::new(|x| m.middle_name(x).map_or(0, |s| s.len()))),
        ("birth_surname", Box::new(|x| m.birth_surname(x).first as usize)),
        ("surname", Box::new(|x| m.surname(x, t23).first as usize)),
    ];
    for (name, f) in &parts {
        let s = Instant::now();
        let h: usize = xs.iter().map(|&x| f(x)).sum();
        println!("  {name:>14}: {:.2} µs ({h})", s.elapsed().as_secs_f64() / xs.len() as f64 * 1e6);
    }

    // Top first names of women born in 1990 against SSA's.
    let d = &p.name_data;
    for (y, sex) in [(1990, Sex::Female), (1950, Sex::Male)] {
        let mut counts: HashMap<u32, u64> = HashMap::new();
        let mut n = 0;
        for c in 0..m.cells() {
            let size = m.cohort_n(c, y);
            for k in 0..(40_000 * size / ever.iter().filter(|e| e.1 == y).map(|e| e.2).sum::<u64>().max(1)) {
                let x = Pid { cell: c, y, i: Key::from_seed(4).with3(c as u64, y as u64, k).below(size) };
                if m.sex(x) == sex {
                    *counts.entry(m.first_name_id(x)).or_default() += 1;
                    n += 1;
                }
            }
        }
        let rows = d.ssa_year(y, sex);
        let ssa_total = rows.last().unwrap().1 as f64;
        let mut ssa: Vec<(u32, f64)> = rows.iter().enumerate().map(|(i, r)| (r.0, (r.1 - if i > 0 { rows[i - 1].1 } else { 0 }) as f64 / ssa_total)).collect();
        ssa.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!("\n{y} {sex:?}, top SSA names: SSA share vs world share ({n} sampled)");
        for &(name, share) in ssa.iter().take(8) {
            println!("  {:>12} {:5.2}%  {:5.2}%", d.first[name as usize], 100.0 * share, 100.0 * *counts.get(&name).unwrap_or(&0) as f64 / n as f64);
        }
    }

    // Surnames: top surnames per group (founders and descendants alive in 2020).
    println!("\ntop birth surnames of people born 1990–1999, by group:");
    for c in 0..m.cells() {
        let mut counts: HashMap<u32, u64> = HashMap::new();
        let mut n = 0;
        for y in 1990..2000 {
            let size = m.cohort_n(c, y);
            for k in 0..(size.min(2000)) {
                let x = Pid { cell: c, y, i: Key::from_seed(6).with3(c as u64, y as u64, k).below(size) };
                *counts.entry(m.birth_surname(x).first).or_default() += 1;
                n += 1;
            }
        }
        let mut top: Vec<(u32, u64)> = counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        println!("  {:>10}: {}", group(Pid { cell: c, y: 1995, i: 0 }), top.iter().take(8).map(|(s, k)| format!("{} {:.1}%", d.surnames[*s as usize], 100.0 * *k as f64 / n as f64)).collect::<Vec<_>>().join(", "));
    }

    // Married women's surnames by wedding era.
    println!("\nwomen married at 35, by birth decade: took the husband's / hyphenated / kept");
    for y0 in [1900, 1940, 1960, 1980, 1990] {
        let (mut took, mut hy, mut kept, mut n) = (0u64, 0u64, 0u64, 0u64);
        for c in 0..m.cells() {
            for y in y0..y0 + 10 {
                let size = m.cohort_n(c, y);
                for k in 0..size.min(3000) {
                    let x = Pid { cell: c, y, i: Key::from_seed(8).with3(c as u64, y as u64, k).below(size) };
                    if m.sex(x) != Sex::Female {
                        continue;
                    }
                    let t = year_start(y + 35);
                    if !m.married_at(x, t) || m.death(x) <= t {
                        continue;
                    }
                    let (birth, now) = (m.birth_surname(x), m.surname(x, t));
                    n += 1;
                    if now == birth {
                        kept += 1;
                    } else if now.hyphen && now.first == birth.first {
                        hy += 1;
                    } else {
                        took += 1;
                    }
                }
            }
        }
        println!("  {y0}s: {:.1}% / {:.1}% / {:.1}% of {n}", 100.0 * took as f64 / n as f64, 100.0 * hy as f64 / n as f64, 100.0 * kept as f64 / n as f64);
    }
}
