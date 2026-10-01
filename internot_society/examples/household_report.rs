//! Household realism report (L3) for the prototype world, against the
//! targets in `docs/superpowers/research/2026-09-30-household-targets.md`.
//! Run: cargo run --release -p internot_society --example household_report
//!
//! People present at each date are sampled uniformly. Household-level shares
//! weight each sampled person by one over their household's size, so every
//! household counts once.

use std::time::Instant;

use internot_society::world::{year_start, DAY};
use internot_society::{Household, Params, PersonId, Sex, World};

const YEAR_S: f64 = 365.2425 * 86_400.0;
const SAMPLE: usize = 60_000;

/// Census targets by year: (mean size, one-person share, couple share,
/// 18–24 at home men/women, 25–34 at home men/women). `None` where the
/// series doesn't reach.
type Targets = (f64, Option<f64>, Option<f64>, Option<[f64; 4]>);

fn targets(year: i32) -> Targets {
    match year {
        1940 => (3.67, None, Some(76.0), None),
        1960 => (3.33, Some(13.1), Some(74.3), Some([52.4, 34.9, 10.9, 7.4])),
        1980 => (2.76, Some(22.7), Some(60.8), Some([54.3, 42.7, 10.5, 7.0])),
        2000 => (2.62, Some(25.5), Some(52.8), Some([57.1, 47.1, 12.9, 8.3])),
        // Couples: married 46.6% plus about 7.6% cohabiting.
        2025 => (2.50, Some(29.5), Some(54.2), Some([58.8, 56.4, 19.2, 13.6])),
        _ => (f64::NAN, None, None, None),
    }
}

fn main() {
    let t0 = Instant::now();
    let w = World::build(Params::prototype(), 42);
    println!("world build {:?}", t0.elapsed());
    println!(
        "\nyear  mean_size (census)  size 1/2/3/4/5/6/7+ %   couple% (census)  couple+kids%  single_mum%  single_dad%  one_person% (census)  roommates%  other_family%"
    );
    let years = [1880, 1900, 1940, 1960, 1980, 2000, 2025];
    let mut person_rows = Vec::new();
    for &year in &years {
        let t = year_start(year) + 180 * DAY;
        let s = sample(&w, t);
        let (mean, one, couple, home) = targets(year);
        let hh: f64 = s.weight.iter().sum();
        let pct = |v: f64| 100.0 * v / hh;
        let sizes: Vec<String> = s.size.iter().map(|&v| format!("{:.1}", pct(v))).collect();
        println!(
            "{year}  {:>4.2} ({})  {}   {:>5.1} ({})  {:>5.1}  {:>5.1}  {:>5.1}  {:>5.1} ({})  {:>5.1}  {:>5.1}",
            SAMPLE as f64 / hh,
            fmt(mean),
            sizes.join("/"),
            pct(s.kind[0]),
            couple.map_or("-".into(), fmt),
            pct(s.kind[1]),
            pct(s.kind[2]),
            pct(s.kind[3]),
            pct(s.kind[4]),
            one.map_or("-".into(), fmt),
            pct(s.kind[5]),
            pct(s.kind[6]),
        );
        person_rows.push((year, s, home));
    }

    println!(
        "\nyear  at home 18-24 m/f % (census)   25-34 m/f % (census)   adults: alone/partner/child_of_hh/other_rel/nonrel %   65+: alone% with_adult_child%   minors alone%"
    );
    for (year, s, home) in &person_rows {
        let r = |a: [u64; 2]| 100.0 * a[0] as f64 / a[1].max(1) as f64;
        let adults = s.adult.iter().map(|a| a[0]).sum::<u64>().max(1) as f64;
        let a: Vec<String> = s
            .adult
            .iter()
            .map(|v| format!("{:.1}", 100.0 * v[0] as f64 / adults))
            .collect();
        println!(
            "{year}  {:>4.1}/{:>4.1} ({})   {:>4.1}/{:>4.1} ({})   {}   {:>4.1} {:>4.1}   {:.3}",
            r(s.home[0]),
            r(s.home[1]),
            home.map_or("-".into(), |h| format!("{}/{}", h[0], h[1])),
            r(s.home[2]),
            r(s.home[3]),
            home.map_or("-".into(), |h| format!("{}/{}", h[2], h[3])),
            a.join("/"),
            r(s.elder_alone),
            r(s.elder_with_child),
            r(s.minors_alone),
        );
    }
    println!("\ncensus 2023 adults: alone 14.8 / spouse or partner 57.8 / child of householder 11.6 / other relatives 12.3 / nonrelatives 3.5");
    println!("Ruggles: 65+ living with an adult child about 70% in 1850, under 15% by 1990-2000; 65+ alone about 28% in 2023");
    status_by_age(&w);
    println!("\nreport time {:?}", t0.elapsed());
}

fn fmt(v: f64) -> String {
    format!("{v}")
}

#[derive(Default)]
struct Sample {
    /// Household weight (1/size) per size class 1..=7+.
    size: [f64; 7],
    /// Household weight per type: couple, couple with children under 18,
    /// single mother, single father, one person, roommates, other family.
    kind: [f64; 7],
    weight: Vec<f64>,
    /// [hits, total] at home: men 18–24, women 18–24, men 25–34, women 25–34.
    home: [[u64; 2]; 4],
    /// Adults by arrangement: alone, with partner, child of householder,
    /// with other relatives, with non-relatives ([count, _]).
    adult: [[u64; 2]; 5],
    elder_alone: [u64; 2],
    elder_with_child: [u64; 2],
    minors_alone: [u64; 2],
}

fn sample(w: &World, t: i64) -> Sample {
    let n = w.population();
    let mut s = Sample::default();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ t as u64;
    let age = |id: PersonId| (t - w.birth(id)) as f64 / YEAR_S;
    while s.weight.len() < SAMPLE {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = (x % n) as PersonId;
        let Some(h) = w.household(id, t) else {
            continue;
        };
        let members = w.members(h, t);
        let size = members.len();
        let wt = 1.0 / size as f64;
        s.weight.push(wt);
        s.size[size.min(7) - 1] += wt;
        let has_own_minor = |p: PersonId| {
            w.children(p)
                .iter()
                .any(|c| members.contains(c) && age(*c) < 18.0)
        };
        let kind = match h {
            Household::Union { a, b, .. } => {
                if has_own_minor(a) || has_own_minor(b) {
                    1
                } else {
                    0
                }
            }
            Household::Roommates { .. } => 5,
            Household::Solo { person, .. } => {
                if size == 1 {
                    4
                } else if has_own_minor(person) {
                    if w.sex(person) == Sex::Female {
                        2
                    } else {
                        3
                    }
                } else {
                    6
                }
            }
        };
        s.kind[kind] += wt;
        if kind == 1 {
            s.kind[0] += wt;
        }

        let a = age(id);
        let parent_home = w
            .dependent_of(id, t)
            .is_some_and(|p| Some(p) == w.mother(id) || Some(p) == w.father(id));
        let male = (w.sex(id) == Sex::Male) as usize;
        if (18.0..25.0).contains(&a) {
            s.home[1 - male][0] += parent_home as u64;
            s.home[1 - male][1] += 1;
        } else if (25.0..35.0).contains(&a) {
            s.home[3 - male][0] += parent_home as u64;
            s.home[3 - male][1] += 1;
        }
        if a >= 18.0 {
            let arrangement = if size == 1 {
                0
            } else if w.partner_at(id, t).is_some() {
                1
            } else if parent_home {
                2
            } else if matches!(h, Household::Roommates { .. }) {
                4
            } else {
                3
            };
            s.adult[arrangement][0] += 1;
        } else {
            s.minors_alone[0] += (size == 1) as u64;
            s.minors_alone[1] += 1;
        }
        if a >= 65.0 {
            s.elder_alone[0] += (size == 1) as u64;
            s.elder_alone[1] += 1;
            let with_child = w
                .children(id)
                .iter()
                .any(|c| members.contains(c) && age(*c) >= 18.0);
            s.elder_with_child[0] += with_child as u64;
            s.elder_with_child[1] += 1;
        }
    }
    s
}

/// Partner status by age in 2023 against Census A1 (married, spouse
/// present) plus UC3 (cohabiting): the current-status targets of R1d.
/// Divorced singles are compared with A1's divorced or separated less the
/// ~18% of them who cohabit.
fn status_by_age(w: &World) {
    let t = year_start(2023) + 180 * DAY;
    let bands = [18.0, 25.0, 35.0, 45.0, 55.0, 65.0, 75.0, 200.0];
    // Census: in a union, divorced single (approx.), widowed.
    let census: [(f64, f64, f64); 7] = [
        (15.0, 0.4, 0.0),
        (53.8, 3.4, 0.4),
        (68.9, 9.3, 0.8),
        (70.1, 13.4, 1.8),
        (66.2, 15.9, 4.7),
        (63.7, 14.4, 11.7),
        (51.5, 9.4, 33.2),
    ];
    // Per band: people, in a union, divorced single, widowed single, never
    // partnered (natives, immigrants).
    let mut c = [[0u64; 6]; 7];
    let n = w.population();
    let mut x: u64 = 0x5EED_0F57_A7A7_u64;
    let mut sampled = 0;
    while sampled < 120_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = (x % n) as PersonId;
        if !w.present_at(id, t) {
            continue;
        }
        let a = (t - w.birth(id)) as f64 / YEAR_S;
        let Some(b) = bands.windows(2).position(|r| a >= r[0] && a < r[1]) else {
            continue;
        };
        sampled += 1;
        c[b][0] += 1;
        if w.partner_at(id, t).is_some() {
            c[b][1] += 1;
            continue;
        }
        let last = w
            .unions(id)
            .into_iter()
            .flatten()
            .filter(|u| u.start <= t)
            .max_by_key(|u| u.start);
        match last {
            Some(u) if u.separation.is_some_and(|s| s == u.end) => c[b][2] += 1,
            Some(_) => c[b][3] += 1,
            None if w.is_immigrant(id) => c[b][5] += 1,
            None => c[b][4] += 1,
        }
    }
    println!("\n2023 partner status by age, % (census in brackets)");
    println!("age      in_union      divorced_single   widowed_single   never: native / immigrant");
    for (i, r) in c.iter().enumerate() {
        let p = |k: usize| 100.0 * r[k] as f64 / r[0].max(1) as f64;
        let (u, d, wd) = census[i];
        println!(
            "{:>3}-{:<3}  {:>5.1} ({u:>4.1})  {:>5.1} ({d:>4.1})  {:>5.1} ({wd:>4.1})  {:>5.1} / {:>4.1}",
            bands[i],
            bands[i + 1],
            p(1),
            p(2),
            p(3),
            p(4),
            p(5)
        );
    }
}
