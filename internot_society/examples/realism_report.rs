//! Realism report for the R1 prototype world: e0 by cohort, TFR by cohort,
//! union timing, never-partnered shares, parity, age gaps, divorce.
//! Run: cargo run --release -p internot_society --example realism_report

use std::time::Instant;

use internot_society::world::{year_start, DAY};
use internot_society::{Params, Sex, World};

const YEAR_S: f64 = 365.25 * 86_400.0;

fn main() {
    let t0 = Instant::now();
    let ledger_only = internot_society::ledger::Ledger::build(Params::prototype(), 42);
    let t_ledger = t0.elapsed();
    drop(ledger_only);
    let t1 = Instant::now();
    let w = World::build_keeping_ledger(Params::prototype(), 42);
    println!(
        "ledger build: {t_ledger:?}; world build (ledger + layouts, events, tables): {:?}; population ever born: {}",
        t1.elapsed(),
        w.population()
    );
    let ledger = w.ledger();
    // Couples moved to a neighbouring dissolution class because they were
    // alone on both sides of theirs (R1c de-isolation).
    let opposite: u64 = ledger
        .blocks
        .iter()
        .flat_map(|b| &b.union_f)
        .filter(|c| !c.kind.same_sex())
        .map(|c| c.total)
        .sum();
    println!(
        "dissolution classes: {} of {opposite} opposite-sex couples moved by de-isolation ({:.3}%)",
        ledger.class_moves,
        100.0 * ledger.class_moves as f64 / opposite.max(1) as f64
    );
    let names: Vec<&str> = ledger
        .params
        .regions
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    for &year in &[1850, 1900, 1950, 1975, 2000, 2025, 2050, 2090] {
        let mut by_region = vec![0u64; names.len()];
        for b in ledger.blocks_of_year(year) {
            let b = &ledger.blocks[b as usize];
            by_region[b.region as usize] += b.size;
        }
        let sizes: Vec<String> = names
            .iter()
            .zip(&by_region)
            .map(|(n, s)| format!("{n} {s}"))
            .collect();
        println!("births {year}: {}", sizes.join(", "));
    }
    // Per-cohort statistics over a sample of each cohort's natives
    // (immigrants arrive single until R1b-2 step B, so their fertility and
    // unions are not yet realistic).
    println!("\ncohort  e0_f  e0_m   CFR  childless  ever_partnered  median_union_age_f  never_at_50  mean_gap  divorced_by_20y  cross_region");
    for &cohort in &[1860, 1900, 1930, 1950, 1970, 1990, 2010] {
        let blocks = ledger.blocks_of_year(cohort);
        let base = ledger.base[blocks.start as usize] as u32;
        let size = ledger.base[blocks.end as usize] as u32 - base;
        let step = (size / 4000).max(1);
        let mut cross = 0u64;
        let (mut lf, mut nf, mut lm, mut nm) = (0.0, 0, 0.0, 0);
        let (
            mut kids,
            mut women,
            mut childless,
            mut partnered,
            mut union_ages,
            mut gaps,
            mut divorced,
            mut unions,
        ) = (0u64, 0u64, 0u64, 0u64, Vec::new(), 0.0, 0u64, 0u64);
        for id in (base..base + size)
            .step_by(step as usize)
            .filter(|&id| !w.is_immigrant(id))
        {
            let life = (w.death(id) - w.birth(id)) as f64 / YEAR_S;
            match w.sex(id) {
                Sex::Female => {
                    lf += life;
                    nf += 1;
                    // Fertility among women alive at 15.
                    if life >= 15.0 {
                        women += 1;
                        let c = w.children(id).len() as u64;
                        kids += c;
                        if c == 0 {
                            childless += 1;
                        }
                        if let Some(u) = w.union(id) {
                            partnered += 1;
                            union_ages.push((u.start - w.birth(id)) as f64 / YEAR_S);
                            gaps += (w.birth(id) - w.birth(u.partner)) as f64 / YEAR_S;
                            unions += 1;
                            cross += (w.region(id) != w.region(u.partner)) as u64;
                            if u.separation.is_some_and(|s| {
                                s - u.start <= (20.0 * YEAR_S) as i64 && s < u.end + 1
                            }) {
                                divorced += 1;
                            }
                        }
                    }
                }
                Sex::Male => {
                    lm += life;
                    nm += 1;
                }
            }
        }
        union_ages.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = union_ages
            .get(union_ages.len() / 2)
            .copied()
            .unwrap_or(f64::NAN);
        println!(
            "{cohort}  {:>5.1} {:>5.1}  {:>4.2}  {:>8.1}%  {:>13.1}%  {:>18.1}  {:>10.1}%  {:>7.2}  {:>13.1}%  {:>11.1}%",
            lf / nf as f64,
            lm / nm as f64,
            kids as f64 / women.max(1) as f64,
            100.0 * childless as f64 / women.max(1) as f64,
            100.0 * partnered as f64 / women.max(1) as f64,
            med,
            100.0 * (women - partnered) as f64 / women.max(1) as f64,
            gaps / unions.max(1) as f64,
            100.0 * divorced as f64 / unions.max(1) as f64,
            100.0 * cross as f64 / unions.max(1) as f64,
        );
    }
    // Sibships, by cohort: share of natives who are only children, and the
    // mean number of siblings (spec §6.2: about 9% only children today).
    println!("\ncohort  only_children  mean_siblings");
    for &cohort in &[1880, 1920, 1950, 1970, 1990, 2010] {
        let blocks = ledger.blocks_of_year(cohort);
        let base = ledger.base[blocks.start as usize] as u32;
        let end = ledger.base[blocks.end as usize] as u32;
        let step = ((end - base) / 4000).max(1) as usize;
        let (mut n, mut only, mut sibs) = (0u64, 0u64, 0u64);
        for id in (base..end).step_by(step).filter(|&id| !w.is_immigrant(id)) {
            let k = w.siblings(id).len() as u64;
            n += 1;
            only += (k == 0) as u64;
            sibs += k;
        }
        println!(
            "{cohort}  {:>12.1}%  {:>13.2}",
            100.0 * only as f64 / n.max(1) as f64,
            sibs as f64 / n.max(1) as f64
        );
    }

    // Fathers, by cohort, among natives with an in-world mother: a father
    // need only be alive at conception, so some children are posthumous
    // (he died during the pregnancy), and a widow's children conceived
    // after his death have no in-world father until widowed re-partnering.
    println!("\ncohort  with_father  posthumous  conceived_after_partner_died");
    for &cohort in &[1860, 1880, 1900, 1920, 1950, 1970, 1990, 2010] {
        let blocks = ledger.blocks_of_year(cohort);
        let base = ledger.base[blocks.start as usize] as u32;
        let end = ledger.base[blocks.end as usize] as u32;
        let step = ((end - base) / 4000).max(1) as usize;
        let (mut n, mut fathered, mut posthumous, mut widowed) = (0u64, 0u64, 0u64, 0u64);
        for id in (base..end).step_by(step) {
            let Some(m) = w.mother(id) else { continue };
            let b = w.birth(id);
            let conception = b - w.ledger().params.fertility.gestation_days * DAY;
            n += 1;
            match w.father(id) {
                Some(f) => {
                    fathered += 1;
                    posthumous += (w.death(f) < b) as u64;
                }
                None => {
                    widowed += w.union(m).is_some_and(|u| {
                        w.sex(u.partner) == Sex::Male
                            && u.start < b
                            && u.separation.map_or(true, |s| s > conception)
                            && w.death(u.partner) <= conception
                    }) as u64;
                }
            }
        }
        let pct = |k: u64| 100.0 * k as f64 / n.max(1) as f64;
        println!(
            "{cohort}  {:>10.1}%  {:>9.2}%  {:>27.2}%",
            pct(fathered),
            pct(posthumous),
            pct(widowed)
        );
    }

    // Re-partnering (R1c; targets in research/2026-09-30-remarriage-targets.md).
    {
        let year_of = |t: i64| internot_society::world::year_of(t);
        // Women divorced from a first union in 1970-1995 before 45: share
        // remarried within 1/3/5/10 years (NSFG 1995: .15/.39/.54/.75).
        let mut within = [0u64; 4];
        let mut divorced = 0u64;
        let mut waits: Vec<i32> = Vec::new();
        let mut ages2: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
        let (mut second_n, mut second_broken) = (0u64, 0u64);
        let (mut gap_first, mut gap_first_10, mut gap_re, mut gap_re_10) = (0u64, 0u64, 0u64, 0u64);
        // Unions per person ever partnered, born 1930-1970 (R1d: no cap).
        let mut counts = [0u64; internot_society::world::MAX_UNIONS + 1];
        for id in (0..w.population() as u32).step_by(7) {
            let us = w.unions(id);
            let (first, second) = (us[0], us[1]);
            if (1930..1970).contains(&w.birth_year(id)) {
                counts[us.iter().flatten().count()] += 1;
            }
            let sex = w.sex(id) as usize;
            let birth_year = w.birth_year(id);
            if let Some(f) = first {
                if let Some(sep) = f.separation {
                    let dy = year_of(sep);
                    if w.sex(id) == Sex::Female
                        && (1970..=1995).contains(&dy)
                        && dy - birth_year < 45
                        && w.death(id) > sep
                    {
                        divorced += 1;
                        if let Some(s2) = second {
                            let wait = year_of(s2.start) - dy;
                            waits.push(wait);
                            for (k, lim) in [1, 3, 5, 10].iter().enumerate() {
                                within[k] += (wait <= *lim) as u64;
                            }
                        }
                    }
                }
                if w.sex(id) == Sex::Male && (1980..2010).contains(&year_of(f.start)) {
                    gap_first += 1;
                    gap_first_10 += (w.birth_year(f.partner) - birth_year >= 10) as u64;
                }
            }
            if let Some(s2) = second {
                let y = year_of(s2.start);
                if (1980..2010).contains(&y) {
                    ages2[sex].push(y - birth_year);
                    if w.sex(id) == Sex::Male {
                        gap_re += 1;
                        gap_re_10 += (w.birth_year(s2.partner) - birth_year >= 10) as u64;
                    }
                }
                if (1960..1990).contains(&y) && w.sex(id) == Sex::Female {
                    second_n += 1;
                    second_broken += s2.separation.is_some_and(|t| year_of(t) - y <= 10) as u64;
                }
            }
        }
        let median = |v: &mut Vec<i32>| {
            v.sort_unstable();
            v.get(v.len() / 2).copied().unwrap_or(0)
        };
        let pct = |a: u64, b: u64| 100.0 * a as f64 / b.max(1) as f64;
        println!(
            "\nre-partnering: women divorced 1970-95 before 45, remarried within 1/3/5/10 y: {:.0}/{:.0}/{:.0}/{:.0}% (NSFG 15/39/54/75); median wait {} y (SIPP ~4)",
            pct(within[0], divorced),
            pct(within[1], divorced),
            pct(within[2], divorced),
            pct(within[3], divorced),
            median(&mut waits)
        );
        println!(
            "  median age at second union 1980-2009: women {}, men {} (SIPP 33/36); husband 10+ older: first unions {:.1}%, remarriages {:.1}% (Pew 4/16)",
            median(&mut ages2[0]),
            median(&mut ages2[1]),
            pct(gap_first_10, gap_first),
            pct(gap_re_10, gap_re)
        );
        println!(
            "  second unions begun 1960-89 broken within 10 y: {:.0}% (NSFG ~39)",
            pct(second_broken, second_n)
        );
        let ever: u64 = counts[1..].iter().sum();
        let shares: Vec<String> = counts[1..]
            .iter()
            .map(|&c| format!("{:.1}", pct(c, ever)))
            .collect();
        let most = counts.iter().rposition(|&c| c > 0).unwrap_or(0);
        println!(
            "  unions per ever-partnered person born 1930-69, 1/2/3/...: {}% (most: {most}; SIPP 2009: married twice 12%, three+ times 3% of adults)",
            shares.join("/")
        );
        // New opposite-sex unions 2005-2014 by partners' previous marriage
        // (Pew 2013: ~20% both previously married, ~20% one).
        let mut by_kind = [0u64; 7];
        for b in &ledger.blocks {
            for c in b.union_f.iter().filter(|c| (2005..2015).contains(&c.year)) {
                by_kind[c.kind as usize] += c.total;
            }
        }
        let opposite = by_kind[0] + by_kind[4] + by_kind[5] + by_kind[6];
        println!(
            "  new unions 2005-14: both previously partnered {:.0}%, one {:.0}% (Pew ~20/20)",
            pct(by_kind[6], opposite),
            pct(by_kind[4] + by_kind[5], opposite)
        );
    }

    // Same-sex share of couples alive in 2019 (ACS: about 1.5% of coupled
    // households).
    {
        let t = year_start(2019) + 180 * 86_400;
        let (mut couples, mut same) = (0u64, 0u64);
        for id in (0..w.population() as u32).step_by(31) {
            if let Some(u) = w.union(id) {
                if u.start <= t && t < u.end {
                    couples += 1;
                    same += (w.sex(id) == w.sex(u.partner)) as u64;
                }
            }
        }
        println!(
            "\ncouples together in 2019: same-sex {:.2}% (ACS 2019 about 1.5%)",
            100.0 * same as f64 / couples.max(1) as f64
        );
    }

    // Population alive and foreign-born share at census dates (US
    // Census: 9.7% 1850, 14.4% 1870, 13.6% 1900, 14.7% 1910, 11.6% 1930,
    // 6.9% 1950, 4.7% 1970, 7.9% 1990, 12.9% 2010, 13.7% 2020).
    println!();
    for &(year, census) in &[
        (1850, 9.7),
        (1870, 14.4),
        (1900, 13.6),
        (1910, 14.7),
        (1930, 11.6),
        (1950, 6.9),
        (1970, 4.7),
        (1990, 7.9),
        (2010, 12.9),
        (2020, 13.7),
    ] {
        let t = year_start(year) + 180 * 86_400;
        let step = 97u32;
        let (mut alive, mut foreign) = (0u64, 0u64);
        for id in (0..w.population() as u32).step_by(step as usize) {
            if w.alive_at(id, t) {
                alive += 1;
                if w.arrival(id).is_some_and(|a| a <= t) {
                    foreign += 1;
                }
            }
        }
        // Arrivals in the decade before: share who came as children with
        // their parents.
        let (mut arrived, mut kids) = (0u64, 0u64);
        for id in (0..w.population() as u32).step_by(step as usize) {
            if w.arrival(id)
                .is_some_and(|a| a <= t && a > t - 10 * 365 * 86_400)
            {
                arrived += 1;
                kids += w.mother(id).is_some() as u64;
            }
        }
        println!(
            "{year}: alive ~{:>9}, foreign-born {:>5.1}% (census {census}%), arrivals in the decade {:>5.1}% children",
            alive * step as u64,
            100.0 * foreign as f64 / alive.max(1) as f64,
            100.0 * kids as f64 / arrived.max(1) as f64
        );
    }
    println!("total report time: {:?}", t0.elapsed());
}
