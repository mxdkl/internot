//! Households on the monotone world against Census targets: mean household
//! size, people living alone, young adults at home, elders with a child,
//! and lookup cost. `MULT` (default 0.05), `BLIND=1`.
use std::collections::HashMap;
use std::time::Instant;

use internot_society::mono::{Household, Mono, Pid};
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let p = Params::embedded("us").unwrap();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    println!("year  persons/hh  alone(of hh)  18-24 at home  25-34 at home  65+ with child  65+ alone   (targets: 1900 4.6/5%, 1950 3.4/11%, 2000 2.6/26%, 2020 2.5/28%; at home 2020: 18-24 ~55%, 25-34 ~17%)");
    for year in [1880, 1900, 1950, 1970, 2000, 2020] {
        let t = year_start(year) + 90 * 86_400;
        let mut people = Vec::new();
        for c in 0..m.cells() {
            for y in m.first_year()..=year {
                m.alive_in_cohort(c, y, t, &mut |x| people.push(x));
            }
        }
        // Sample people; a household's weight is 1 / its size, so the mean
        // over households is unbiased.
        let k = people.len().min(30_000);
        let sample: Vec<Pid> = (0..k).map(|j| people[Key::from_seed(1).with2(year as u64, j as u64).below(people.len() as u64) as usize]).collect();
        let (mut inv, mut alone_inv) = (0.0f64, 0.0f64);
        let mut buckets: HashMap<&str, (u64, u64)> = HashMap::new();
        for &x in &sample {
            let h = m.household(x, t).unwrap();
            let n = m.members(h, t).len() as f64;
            inv += 1.0 / n;
            if n == 1.0 {
                alone_inv += 1.0;
            }
            let age = m.age_at(x, t);
            let at_home = m.dependent_of(x, t).is_some() || m.kin_host(m.chain_end(x, t), t).is_some_and(|host| {
                // Living with a parent who hosts.
                m.parents(x).is_some_and(|(mo, _, u)| host == mo || u.is_some_and(|u| host == u.husband))
            });
            let key = if (18.0..25.0).contains(&age) {
                Some("18-24")
            } else if (25.0..35.0).contains(&age) {
                Some("25-34")
            } else {
                None
            };
            if let Some(kk) = key {
                let e = buckets.entry(kk).or_default();
                e.0 += at_home as u64;
                e.1 += 1;
            }
            if age >= 65.0 {
                let members = m.members(h, t);
                let with_child = m.children(x).iter().any(|c| members.contains(c));
                let e = buckets.entry("65+ child").or_default();
                e.0 += with_child as u64;
                e.1 += 1;
                let e = buckets.entry("65+ alone").or_default();
                e.0 += (members.len() == 1) as u64;
                e.1 += 1;
            }
        }
        let share = |b: &str| buckets.get(b).map_or(f64::NAN, |&(a, n)| 100.0 * a as f64 / n.max(1) as f64);
        println!(
            "{year}  {:9.2}  {:11.1}%  {:12.1}%  {:12.1}%  {:13.1}%  {:8.1}%",
            k as f64 / inv,
            100.0 * alone_inv / inv,
            share("18-24"),
            share("25-34"),
            share("65+ child"),
            share("65+ alone")
        );
    }
    // Cost.
    let t = year_start(2023);
    let mut people = Vec::new();
    for c in 0..m.cells() {
        for y in m.first_year()..=2023 {
            m.alive_in_cohort(c, y, t, &mut |x| people.push(x));
        }
    }
    let xs: Vec<Pid> = (0..20_000).map(|j| people[Key::from_seed(2).with(j).below(people.len() as u64) as usize]).collect();
    let mut v: Vec<u64> = xs
        .iter()
        .map(|&x| {
            let s = Instant::now();
            let h = m.household(x, t).unwrap();
            std::hint::black_box(m.members(h, t));
            s.elapsed().as_nanos() as u64
        })
        .collect();
    v.sort_unstable();
    println!("household + members at 2023: p50 {:.1} µs, p99 {:.1} µs", v[v.len() / 2] as f64 / 1e3, v[v.len() * 99 / 100] as f64 / 1e3);
    let kinds = xs.iter().filter(|&&x| matches!(m.household(x, t), Some(Household::Union { .. }))).count();
    println!("people in a couple's home: {:.1}%", 100.0 * kinds as f64 / xs.len() as f64);
}
