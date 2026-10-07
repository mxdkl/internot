//! Work on the monotone world: example careers, and realism against BLS and
//! Census targets (participation by sex and era, unemployment, elapsed
//! tenure by age, jobs by age bracket, farm and factory shares by era,
//! pay), with lookup cost. `MULT` (default 0.05), `BLIND=1`.
use std::time::Instant;

use internot_society::mono::{Mono, Pid, WorkSpell};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::{from_secs, year_start};

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let p = Params::embedded("us").unwrap();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let date = |t: i64| from_secs(t).format("%Y-%m").to_string();
    let year = 31_556_952i64;

    for (c, y, j) in [(0u16, 1925, 1u64), (0, 1950, 4), (0, 1960, 7), (1, 1965, 2), (0, 1975, 9), (0, 1985, 3)] {
        if c >= m.cells() || m.cohort_n(c, y) == 0 {
            continue;
        }
        let x = Pid { cell: c, y, i: Key::from_seed(21).with2(y as u64, j).below(m.cohort_n(c, y)) };
        let t = year_start(2024).min(m.death(x) - 1);
        println!("\n{} ({:?}, born {y}, {}):", m.full_name(x, t), m.sex(x), m.education_level(x).name());
        let s = Instant::now();
        let career = m.career(x);
        let dt = s.elapsed().as_secs_f64();
        for sp in &career {
            match sp {
                WorkSpell::Employed(job) => {
                    let title = m.job_title(job);
                    let e = m.employer_info(m.employer(x, job));
                    println!(
                        "  {}–{}: {title}{} at {} ({}{}, {}, {}); ${:.0}k a year (${:.0}k then)",
                        date(job.start),
                        date(job.end),
                        if job.part_time { " (part time)" } else { "" },
                        e.name,
                        e.industry,
                        e.line.map(|l| format!(": {l}")).unwrap_or_default(),
                        e.city,
                        e.state,
                        job.pay_2023 / 1e3,
                        m.nominal(job.pay_2023, from_secs(job.start).format("%Y").to_string().parse().unwrap()) / 1e3
                    );
                }
                WorkSpell::Unemployed { from, to } => println!("  {}–{}: unemployed", date(*from), date(*to)),
                WorkSpell::OutOfLaborForce { from, to, why } => println!("  {}–{}: out of the labour force ({why:?})", date(*from), date(*to)),
                WorkSpell::Retired { from, to } => println!("  {}–{}: retired", date(*from), date(*to)),
            }
        }
        println!("  ({:.0} µs)", dt * 1e6);
    }

    // Participation, unemployment, tenure by era.
    println!("\nyear  LFP 16+ women/men   prime-age (25-54) women/men   unemployed   farm/production share   median tenure 25-34/45-54   (targets: 1900 ~20/86; 1950 33/87, prime 35/96; 2000 60/75, prime 77/91; 2023 57/68, prime 77/89; tenure 3.0/7.0)");
    for yr in [1900, 1950, 1970, 2000, 2023] {
        let t = year_start(yr) + 100 * 86_400;
        let mut people = Vec::new();
        for c in 0..m.cells() {
            for y in m.first_year()..=yr - 16 {
                m.alive_in_cohort(c, y, t, &mut |x| people.push(x));
            }
        }
        let k = people.len().min(5000);
        let (mut lf, mut n, mut plf, mut pn, mut unemp, mut emp, mut farm, mut prod) = ([0u32; 2], [0u32; 2], [0u32; 2], [0u32; 2], 0u32, 0u32, 0u32, 0u32);
        let (mut ten_y, mut ten_o) = (vec![], vec![]);
        for j in 0..k {
            let x = people[Key::from_seed(2).with2(yr as u64, j as u64).below(people.len() as u64) as usize];
            let s = (m.sex(x) == Sex::Male) as usize;
            let age = m.age_at(x, t);
            let st = m.work_at(x, t);
            let in_lf = matches!(st, Some(WorkSpell::Employed(_)) | Some(WorkSpell::Unemployed { .. }));
            n[s] += 1;
            lf[s] += in_lf as u32;
            if (25.0..55.0).contains(&age) {
                pn[s] += 1;
                plf[s] += in_lf as u32;
            }
            match st {
                Some(WorkSpell::Employed(job)) => {
                    emp += 1;
                    let (_, code) = m.occupation_info(job.occupation);
                    farm += code.starts_with("45") as u32;
                    prod += code.starts_with("51") as u32;
                    let tenure = (t - job.start) as f64 / year as f64;
                    if (25.0..35.0).contains(&age) {
                        ten_y.push(tenure);
                    } else if (45.0..55.0).contains(&age) {
                        ten_o.push(tenure);
                    }
                }
                Some(WorkSpell::Unemployed { .. }) => unemp += 1,
                _ => {}
            }
        }
        let med = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            if v.is_empty() { f64::NAN } else { v[v.len() / 2] }
        };
        let pc = |a: u32, b: u32| 100.0 * a as f64 / b.max(1) as f64;
        println!(
            "{yr}  {:5.1} / {:5.1}          {:5.1} / {:5.1}                {:5.1}%       {:5.1}% / {:5.1}%           {:4.1} / {:4.1}",
            pc(lf[0], n[0]), pc(lf[1], n[1]), pc(plf[0], pn[0]), pc(plf[1], pn[1]), pc(unemp, unemp + emp), pc(farm, emp), pc(prod, emp), med(&mut ten_y), med(&mut ten_o)
        );
    }

    // Participation by age (CPS annual averages, BLS).
    let bands = [(16.0, 20.0), (20.0, 25.0), (25.0, 35.0), (35.0, 45.0), (45.0, 55.0), (55.0, 65.0), (65.0, 200.0)];
    let targets: [(i32, [f64; 7], [f64; 7]); 4] = [
        (1950, [41.0, 46.0, 34.0, 39.1, 37.9, 27.0, 9.7], [63.2, 87.9, 96.0, 97.6, 95.8, 86.9, 45.8]),
        (1970, [44.0, 57.7, 45.0, 51.1, 54.4, 43.0, 9.7], [56.1, 85.1, 95.8, 96.9, 94.3, 83.0, 26.8]),
        (2000, [51.2, 73.1, 76.1, 77.2, 76.8, 51.9, 9.4], [52.8, 82.6, 93.4, 92.7, 88.6, 67.3, 17.7]),
        (2023, [36.9, 70.5, 78.7, 76.8, 75.5, 60.0, 15.6], [35.6, 74.1, 89.2, 90.0, 86.8, 71.4, 23.9]),
    ];
    println!("\nparticipation by age, women then men (model / CPS): 16-19, 20-24, 25-34, 35-44, 45-54, 55-64, 65+");
    for (yr, tw, tm) in targets {
        let t = year_start(yr) + 100 * 86_400;
        let mut hit = [[0u32; 7]; 2];
        let mut tot = [[0u32; 7]; 2];
        for c in 0..m.cells() {
            for y in (yr - 90).max(m.first_year())..=yr - 16 {
                let n = m.cohort_n(c, y);
                for j in 0..n.min(120) {
                    let x = Pid { cell: c, y, i: Key::from_seed(4).with3(c as u64, y as u64, j).below(n) };
                    if !m.alive_at(x, t) {
                        continue;
                    }
                    let age = m.age_at(x, t);
                    let Some(b) = bands.iter().position(|&(lo, hi)| (lo..hi).contains(&age)) else { continue };
                    let s = (m.sex(x) == Sex::Male) as usize;
                    tot[s][b] += 1;
                    hit[s][b] += m.work_at(x, t).is_some_and(|s| s.in_labor_force()) as u32;
                }
            }
        }
        for (s, tg) in [(0, tw), (1, tm)] {
            let row: Vec<String> = (0..7).map(|b| format!("{:3.0}/{:<3.0}", 100.0 * hit[s][b] as f64 / tot[s][b].max(1) as f64, tg[b])).collect();
            println!("{yr} {}  {}", if s == 0 { "W" } else { "M" }, row.join("  "));
        }
    }

    // Jobs per age bracket, cohorts 1957–64 (NLSY79: 5.6 at 18–24, 4.5 at 25–34, 2.9 at 35–44, 2.2 at 45–54).
    let mut counts = [0f64; 4];
    let mut n = 0;
    for c in 0..m.cells() {
        for y in 1957..1965 {
            for j in 0..m.cohort_n(c, y).min(300) {
                let x = Pid { cell: c, y, i: Key::from_seed(3).with3(c as u64, y as u64, j).below(m.cohort_n(c, y)) };
                if m.death(x) < m.birth(x) + 55 * year {
                    continue;
                }
                n += 1;
                for sp in m.career(x) {
                    if let WorkSpell::Employed(job) = sp {
                        let (a0, a1) = ((job.start - m.birth(x)) as f64 / year as f64, (job.end - m.birth(x)) as f64 / year as f64);
                        for (b, (lo, hi)) in [(18.0, 25.0), (25.0, 35.0), (35.0, 45.0), (45.0, 55.0)].iter().enumerate() {
                            if a0 < *hi && a1 > *lo {
                                counts[b] += 1.0;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("jobs per person, born 1957-64: 18-24 {:.1}, 25-34 {:.1}, 35-44 {:.1}, 45-54 {:.1} (NLSY79 5.6 / 4.5 / 2.9 / 2.2)", counts[0] / n as f64, counts[1] / n as f64, counts[2] / n as f64, counts[3] / n as f64);

    // Cost.
    let xs: Vec<Pid> = (0..2000u64).filter_map(|j| {
        let y = 1930 + (j % 70) as i32;
        let c = (j % m.cells() as u64) as u16;
        let n = m.cohort_n(c, y);
        (n > 0).then(|| Pid { cell: c, y, i: Key::from_seed(9).with(j).below(n) })
    }).collect();
    let s = Instant::now();
    let h: usize = xs.iter().map(|&x| m.career(x).len()).sum();
    println!("career: {:.0} µs per person ({h} spells)", s.elapsed().as_secs_f64() / xs.len() as f64 * 1e6);
    for (name, bytes) in m.memory_report().iter().filter(|r| r.0.contains("occupation")) {
        println!("{name}: {:.2} MB", *bytes as f64 / 1e6);
    }
}
