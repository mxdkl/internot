//! Heritage in the monotone world: each group's share of the living at
//! several dates, and first unions across groups by union decade (the open
//! market). `MULT` (default 1), `PACK` (default `us`).
use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    let m = Mono::new(&p, 42, mult);
    let names: Vec<String> = (0..m.cells()).map(|c| m.heritage(c).map_or("all".into(), |h| p.heritage.groups[h.0 as usize].name.clone())).collect();
    println!("{:>12} {}", "alive at", names.iter().map(|n| format!("{:>10}", &n[..n.len().min(10)])).collect::<String>());
    for y in [1850, 1900, 1950, 1980, 2000, 2020] {
        let t = year_start(y);
        let alive: Vec<u64> = (0..m.cells())
            .map(|c| {
                (m.first_year()..=y)
                    .into_par_iter()
                    .map(|yy| {
                        let mut k = 0u64;
                        m.alive_in_cohort(c, yy, t, &mut |_| k += 1);
                        k
                    })
                    .sum()
            })
            .collect();
        let total: u64 = alive.iter().sum();
        println!("{y:>12} {}   (total {total})", alive.iter().map(|&a| format!("{:>9.2}%", 100.0 * a as f64 / total as f64)).collect::<String>());
    }
    // First unions by the wife's union decade: share across groups, by the
    // wife's group.
    println!("\nfirst unions across groups (sampled wives), by union decade:");
    println!("{:>12} {}", "decade", names.iter().map(|n| format!("{:>10}", &n[..n.len().min(10)])).collect::<String>());
    for d in (1880..=2020).step_by(20) {
        let mut row = Vec::new();
        for c in 0..m.cells() {
            let (mut n, mut out) = (0u64, 0u64);
            for y in d - 35..d - 15 {
                let size = m.cohort_n(c, y);
                if size == 0 {
                    continue;
                }
                for k in 0..400u64 {
                    let x = Pid { cell: c, y, i: Key::from_seed(5).with3(c as u64, y as u64, k).below(size) };
                    if m.sex(x) != Sex::Female {
                        continue;
                    }
                    if let Some((h, start)) = m.spouse(x) {
                        let uy = procedural_core::stream::year_of(start);
                        if uy >= d && uy < d + 20 {
                            n += 1;
                            out += (h.cell != x.cell) as u64;
                        }
                    }
                }
            }
            row.push(if n > 0 { format!("{:>9.1}%", 100.0 * out as f64 / n as f64) } else { format!("{:>10}", "-") });
        }
        println!("{:>12} {}", format!("{d}–{}", d + 19), row.concat());
    }
}
