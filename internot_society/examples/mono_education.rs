//! Education on the monotone world against its targets: final attainment by
//! cohort, sex and group; latent and years-of-schooling correlations of
//! partners, parents and children, and siblings; enrollment by age; age at
//! the bachelor's; and lookup cost. `MULT` (default 0.05), `BLIND=1`.
use std::time::Instant;

use internot_society::mono::{EducationLevel as L, Mono, Pid, Schooling};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;

fn years(l: L) -> f64 {
    match l {
        L::LessThanHighSchool => 9.0,
        L::HighSchool => 12.0,
        L::SomeCollege => 13.0,
        L::Associate => 14.0,
        L::Bachelor => 16.0,
        L::Master => 18.0,
        L::Professional => 19.0,
        L::Doctorate => 20.0,
    }
}

fn corr(v: &[(f64, f64)]) -> f64 {
    let n = v.len() as f64;
    let (ma, mb) = (v.iter().map(|p| p.0).sum::<f64>() / n, v.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for &(a, b) in v {
        sab += (a - ma) * (b - mb);
        saa += (a - ma) * (a - ma);
        sbb += (b - mb) * (b - mb);
    }
    sab / (saa * sbb).sqrt()
}

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let p = Params::embedded("us").unwrap();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let sample = |c: u16, y: i32, k: u64| -> Vec<Pid> {
        let n = m.cohort_n(c, y);
        (0..k.min(n)).map(|j| Pid { cell: c, y, i: Key::from_seed(1).with3(c as u64, y as u64, j).below(n) }).collect()
    };

    println!("final attainment (% <HS / HS / some coll+assoc / BA / grad), BA+ by sex, by birth cohort:");
    for y in [1880, 1910, 1935, 1945, 1965, 1985] {
        let mut by_sex = [[0u64; 5]; 2];
        for c in 0..m.cells() {
            for yy in y - 2..=y + 2 {
                for x in sample(c, yy, 600) {
                    let l = m.education_level(x);
                    let b = match l {
                        L::LessThanHighSchool => 0,
                        L::HighSchool => 1,
                        L::SomeCollege | L::Associate => 2,
                        L::Bachelor => 3,
                        _ => 4,
                    };
                    by_sex[(m.sex(x) == Sex::Male) as usize][b] += 1;
                }
            }
        }
        let all: Vec<u64> = (0..5).map(|b| by_sex[0][b] + by_sex[1][b]).collect();
        let n: u64 = all.iter().sum();
        let pct = |v: u64, n: u64| 100.0 * v as f64 / n.max(1) as f64;
        let ba = |s: usize| pct(by_sex[s][3] + by_sex[s][4], by_sex[s].iter().sum());
        println!(
            "  {y}: {:.1} / {:.1} / {:.1} / {:.1} / {:.1}   BA+ women {:.1}, men {:.1}",
            pct(all[0], n), pct(all[1], n), pct(all[2], n), pct(all[3], n), pct(all[4], n), ba(0), ba(1)
        );
    }
    if m.cells() > 1 {
        println!("BA+ by group, cohorts 1943–47 / 1973–77 / 1988–92 (ACS 2023: White 35/44/47, Black 20/29/29, Hispanic 15/21/25, Asian 41/60/70, AIAN 16/16/16):");
        for c in 0..m.cells() {
            let row: Vec<String> = [1945, 1975, 1990]
                .iter()
                .map(|&y| {
                    let xs: Vec<Pid> = (y - 2..=y + 2).flat_map(|yy| sample(c, yy, 800)).collect();
                    if xs.is_empty() {
                        return "  -  ".into();
                    }
                    let ba = xs.iter().filter(|&&x| m.education_level(x) >= L::Bachelor).count();
                    format!("{:5.1}", 100.0 * ba as f64 / xs.len() as f64)
                })
                .collect();
            println!("  {:>36}: {}", m.heritage_name(Pid { cell: c, y: 1990, i: 0 }).unwrap_or("all"), row.join(" "));
        }
    }

    // Correlations among people born 1940–1990.
    let (mut sp_z, mut sp_y, mut pc_z, mut pc_y, mut sib_z, mut sib_y) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    for c in 0..m.cells() {
        for y in (1940..1990).step_by(5) {
            for x in sample(c, y, 300) {
                let (zx, yx) = (m.education_latent(x), years(m.education_level(x)));
                if m.sex(x) == Sex::Female {
                    if let Some((h, _)) = m.spouse(x) {
                        sp_z.push((zx, m.education_latent(h)));
                        sp_y.push((yx, years(m.education_level(h))));
                    }
                }
                if let Some(mo) = m.mother(x) {
                    pc_z.push((zx, m.education_latent(mo)));
                    pc_y.push((yx, years(m.education_level(mo))));
                    if let Some(&s) = m.siblings(x).first() {
                        sib_z.push((zx, m.education_latent(s)));
                        sib_y.push((yx, years(m.education_level(s))));
                    }
                }
            }
        }
    }
    println!(
        "correlations, latent / years (targets latent, years): partners {:.2} / {:.2} (0.65–0.73, 0.56–0.64); mother–child {:.2} / {:.2} (0.55, ~0.45); siblings {:.2} / {:.2} (0.6, ~0.5)",
        corr(&sp_z), corr(&sp_y), corr(&pc_z), corr(&pc_y), corr(&sib_z), corr(&sib_y)
    );

    // Enrollment by age, 1910 and 2020.
    for year in [1910, 1960, 2020] {
        let t = year_start(year) + 300 * 86_400;
        let row: Vec<String> = [5, 6, 10, 13, 14, 15, 16, 17, 18, 20, 22]
            .iter()
            .map(|&age: &i32| {
                // Born in `year − age` (and alive): aged `age` by November.
                let xs: Vec<Pid> = (0..m.cells()).flat_map(|c| sample(c, year - age, 1500)).filter(|&x| m.alive_at(x, t) && m.age_at(x, t) >= age as f64).collect();
                let enrolled = xs.iter().filter(|&&x| !matches!(m.schooling_at(x, t), Schooling::Out | Schooling::NotYet)).count();
                format!("{age}: {:.0}%", 100.0 * enrolled as f64 / xs.len().max(1) as f64)
            })
            .collect();
        println!("enrolled in {year} (November) by age: {}", row.join(", "));
    }
    println!("  (targets 1910: 13 89%, 14 81%, 15 68%, 16 51%, 17 35%; 2020 ACS: 17 ~95%, 18 ~77%, 20 ~50%)");
    {
    }
    // Age at the bachelor's, cohorts 1985–94.
    let mut ages = Vec::new();
    for c in 0..m.cells() {
        for y in 1985..1995 {
            for x in sample(c, y, 400) {
                let path = m.education_path(x);
                if path.level >= L::Bachelor {
                    if let Some((_, end)) = path.college {
                        ages.push((end - m.birth(x)) as f64 / 31_556_952.0);
                    }
                }
            }
        }
    }
    let share = |lo: f64, hi: f64| 100.0 * ages.iter().filter(|&&a| a >= lo && a < hi).count() as f64 / ages.len().max(1) as f64;
    println!("age at the bachelor's: ≤23 {:.0}%, 24–29 {:.0}%, 30+ {:.0}% (B&B: 63 / 21 / 15)", share(0.0, 24.0), share(24.0, 30.0), share(30.0, 200.0));

    // Histories.
    let date = |t: i64| procedural_core::stream::from_secs(t).format("%Y-%m").to_string();
    for (c, y, j) in [(0u16, 1960, 7u64), (0, 1985, 3), (1, 1975, 2), (0, 1990, 11), (0, 2005, 5)] {
        if c >= m.cells() || m.cohort_n(c, y) == 0 {
            continue;
        }
        let x = Pid { cell: c, y, i: Key::from_seed(12).with2(y as u64, j).below(m.cohort_n(c, y)) };
        let t = year_start(2024).min(m.death(x) - 1);
        println!("\n{} (born {y}, {}): {}", m.full_name(x, t), m.heritage_name(x).unwrap_or("-"), m.education_level(x).name());
        let s = Instant::now();
        let hist = m.education_history(x, t);
        let dt = s.elapsed().as_secs_f64();
        for st in hist {
            let info = m.institution(st.institution);
            let what = match (st.grades, st.degree) {
                (Some((0, 0)), _) => "kindergarten".to_string(),
                (Some((a, b)), _) if a == b => format!("grade {a}"),
                (Some((a, b)), _) => format!("grades {}–{b}", if a == 0 { "K".into() } else { a.to_string() }),
                (None, Some(d)) => d.name().to_string(),
                (None, None) => "left without a degree".into(),
            };
            println!("  {}–{}: {} ({}, {}, {}) — {what}", date(st.from), date(st.to), info.name, info.kind, info.city, info.state);
        }
        println!("  ({:.0} µs)", dt * 1e6);
    }
    for (name, bytes) in m.memory_report().iter().filter(|r| r.0.contains("school")) {
        println!("{name}: {:.2} MB", *bytes as f64 / 1e6);
    }

    // Cost.
    let xs: Vec<Pid> = (0..m.cells()).flat_map(|c| (1900..2020).step_by(7).flat_map(move |y| (0..20u64).map(move |j| (c, y, j)))).filter_map(|(c, y, j)| {
        let n = m.cohort_n(c, y);
        (n > 0).then(|| Pid { cell: c, y, i: Key::from_seed(5).with3(c as u64, y as u64, j).below(n) })
    }).collect();
    let s = Instant::now();
    let h: u64 = xs.iter().map(|&x| m.education_level(x) as u64).sum();
    let level = s.elapsed().as_secs_f64() / xs.len() as f64;
    let s = Instant::now();
    let h2: u64 = xs.iter().map(|&x| matches!(m.schooling_at(x, year_start(2000)), Schooling::Out) as u64).sum();
    let at = s.elapsed().as_secs_f64() / xs.len() as f64;
    println!("cost: level {:.2} µs, schooling at a date {:.2} µs ({h} {h2})", level * 1e6, at * 1e6);
}
