//! Names realism report (N1) for the prototype world: sample families, top
//! names against SSA, distinctness, surnames at marriage, the married share
//! of couples, and lookup timings.
//! Run: cargo run --release -p internot_society --example names_report

use std::collections::HashMap;
use std::time::Instant;

use internot_society::world::{year_start, DAY};
use internot_society::{Params, PersonId, Sex, World};

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn main() {
    let t0 = Instant::now();
    let w = World::build(Params::prototype(), 42);
    println!("world build {:?}", t0.elapsed());
    let n = w.population();
    let groups: Vec<String> = w
        .ledger()
        .params
        .heritage
        .groups
        .iter()
        .map(|g| g.id.clone())
        .collect();
    let t = year_start(2025) + 180 * DAY;
    let age = |x: PersonId| ((t - w.birth(x)) as f64 / (365.2425 * DAY as f64)) as i32;

    // 1. Families to read.
    println!("\n== sample families in 2025 ==");
    let mut x = 0x1234_5678_9abc_def0u64;
    let mut shown = 0;
    while shown < 12 {
        let id = (xorshift(&mut x) % n) as PersonId;
        if !w.present_at(id, t) || age(id) < 30 || w.partner_at(id, t).is_none() {
            continue;
        }
        shown += 1;
        let p = w.partner_at(id, t).unwrap();
        println!(
            "{} ({}, {}, born {}{}) married to {} ({}, {})",
            w.full_name(id, t),
            age(id),
            groups[w.heritage(id).index()],
            w.birth_year(id),
            if w.is_immigrant(id) {
                ", immigrant"
            } else {
                ""
            },
            w.full_name(p, t),
            age(p),
            groups[w.heritage(p).index()],
        );
        if let Some(m) = w.mother(id) {
            let father = w.father(id).map_or("-".into(), |f| w.full_name(f, t));
            println!("    parents: {} and {}", w.full_name(m, t), father);
        }
        for c in w.children(id).iter().take(4) {
            println!(
                "    child: {} (born {}{})",
                w.full_name(*c, t),
                w.birth_year(*c),
                if w.alive_at(*c, t) { "" } else { ", died" }
            );
        }
    }

    // 2. Top first names by cohort, sex and group.
    println!("\n== top first names by birth decade, sex and group ==");
    for &decade in &[1900, 1950, 1980, 2010] {
        for sex in [Sex::Female, Sex::Male] {
            let mut by_group: Vec<HashMap<&str, u32>> = vec![HashMap::new(); groups.len()];
            let mut all: HashMap<&str, u32> = HashMap::new();
            for b in (decade..decade + 10).flat_map(|y| w.ledger().blocks_of_year(y)) {
                let (lo, hi) = (w.ledger().base[b as usize], w.ledger().base[b as usize + 1]);
                let step = ((hi - lo) / 300).max(1);
                for id in (lo..hi).step_by(step as usize) {
                    let id = id as PersonId;
                    if w.sex(id) != sex || w.is_immigrant(id) {
                        continue;
                    }
                    let name = w.first_name(id);
                    *by_group[w.heritage(id).index()].entry(name).or_default() += 1;
                    *all.entry(name).or_default() += 1;
                }
            }
            let top = |m: &HashMap<&str, u32>| {
                let mut v: Vec<(&&str, &u32)> = m.iter().collect();
                v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                v.iter()
                    .take(6)
                    .map(|(k, _)| **k)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("{decade}s {:?} all: {}", sex, top(&all));
            for (g, m) in by_group.iter().enumerate() {
                if m.values().sum::<u32>() >= 30 {
                    println!("    {:<9} {}", groups[g], top(m));
                }
            }
        }
    }

    // 3. Distinctness among the living.
    println!("\n== distinct names among 10,000 people alive in 2025 ==");
    let mut firsts = HashMap::new();
    let mut surnames = HashMap::new();
    let mut fulls = HashMap::new();
    let mut seen = 0;
    while seen < 10_000 {
        let id = (xorshift(&mut x) % n) as PersonId;
        if !w.present_at(id, t) {
            continue;
        }
        seen += 1;
        *firsts.entry(w.first_name(id).to_string()).or_insert(0u32) += 1;
        let s = w.surname_text(w.surname(id, t));
        *surnames.entry(s.clone()).or_insert(0u32) += 1;
        *fulls
            .entry(format!("{} {}", w.first_name(id), s))
            .or_insert(0u32) += 1;
    }
    let top = |m: &HashMap<String, u32>| {
        let mut v: Vec<(&String, &u32)> = m.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        v.iter()
            .take(8)
            .map(|(k, c)| format!("{k} {c}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!(
        "first names {} distinct (top: {})",
        firsts.len(),
        top(&firsts)
    );
    println!(
        "surnames {} distinct (top: {})",
        surnames.len(),
        top(&surnames)
    );
    println!(
        "first + last {} distinct; repeated: {} people",
        fulls.len(),
        fulls.values().filter(|&&c| c > 1).sum::<u32>()
    );

    // 4. Marriage and surnames.
    println!("\n== couples living together: married share (target 2023: 86.7%) ==");
    for &year in &[1950, 1970, 1990, 2010, 2023] {
        let t = year_start(year) + 180 * DAY;
        let (mut couples, mut married) = (0u64, 0u64);
        let mut took = [0u64; 2];
        while couples < 20_000 {
            let id = (xorshift(&mut x) % n) as PersonId;
            if w.sex(id) != Sex::Female || !w.present_at(id, t) || w.partner_at(id, t).is_none() {
                continue;
            }
            couples += 1;
            if w.married_at(id, t) {
                married += 1;
                let p = w.partner_at(id, t).unwrap();
                if w.sex(p) == Sex::Male {
                    took[1] += 1;
                    took[0] += (w.surname(id, t).first == w.surname(p, t).first) as u64;
                }
            }
        }
        println!(
            "{year}: married {:.1}%; married women sharing the husband's surname {:.1}%",
            100.0 * married as f64 / couples as f64,
            100.0 * took[0] as f64 / took[1].max(1) as f64
        );
    }

    // 5. Timings.
    println!("\n== lookup timings over 20,000 uniform ids (ns p50/p90/p99) ==");
    let ids: Vec<PersonId> = (0..20_000)
        .map(|_| (xorshift(&mut x) % n) as PersonId)
        .collect();
    let pct = |v: &mut Vec<u64>| {
        v.sort_unstable();
        let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
        format!("{}/{}/{}", q(0.5), q(0.9), q(0.99))
    };
    let mut sum = 0usize;
    for (label, f) in [
        (
            "first_name",
            &(|id| w.first_name(id).len()) as &dyn Fn(PersonId) -> usize,
        ),
        ("surname", &|id| w.surname(id, t).first as usize),
        ("full_name", &|id| w.full_name(id, t).len()),
    ] {
        let mut lat = Vec::with_capacity(ids.len());
        for &id in &ids {
            let s = Instant::now();
            sum += f(id);
            lat.push(s.elapsed().as_nanos() as u64);
        }
        println!("{label}: {}", pct(&mut lat));
    }
    println!("(checksum {sum})\nreport time {:?}", t0.elapsed());
}
