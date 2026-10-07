//! The monotone world prototype (`thinking/claude/006`): build, closed-form
//! "alive at t" counts against brute force, enumeration speed, lookups and a
//! realism sanity check. `MULT=1 VERIFY=1 cargo run --release -p
//! internot_society --example mono_report`; `BLIND=1` for the heritage-blind
//! world (one cell).
use std::time::Instant;

use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    s.split_whitespace().nth(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) * 4096.0 / 1e6
}

fn pct(v: &mut [u64], q: f64) -> f64 {
    v.sort_unstable();
    v[((v.len() as f64 - 1.0) * q) as usize] as f64
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let verify = std::env::var("VERIFY").is_ok();
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    let r0 = rss_mb();
    let s = Instant::now();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let build = s.elapsed().as_secs_f64();
    let cys: Vec<(u16, i32)> = (0..m.cells()).flat_map(|c| (m.first_year()..=2023).map(move |y| (c, y))).collect();
    let ever: u64 = cys.iter().map(|&(c, y)| m.cohort_n(c, y)).sum();
    println!("mono ×{mult}: {ever} people born by 2023 ({:.3e}); build {build:.2} s; +{:.0} MB resident", ever as f64, rss_mb() - r0);
    if std::env::var("MEMORY").is_ok() {
        let parts = m.memory_report();
        let total: usize = parts.iter().map(|p| p.1).sum();
        for (name, bytes) in &parts {
            println!("    {name:>24}: {:7.2} MB", *bytes as f64 / 1e6);
        }
        println!("    {:>24}: {:7.2} MB", "total", total as f64 / 1e6);
    }

    for (name, t) in [("2023-01-01", year_start(2023)), ("1950-01-01", year_start(1950)), ("2023-10-15", year_start(2023) + 287 * 86_400)] {
        let s = Instant::now();
        let (lo, hi) = m.alive_bounds(t);
        println!("  alive {name}: between {lo} and {hi} (±{:.4}%), {:.2} ms, no per-person checks", 50.0 * (hi - lo) as f64 / lo as f64, s.elapsed().as_secs_f64() * 1e3);
        if std::env::var("BOUNDS_ONLY").is_ok() {
            continue;
        }
        let s = Instant::now();
        let (closed, checked, people) = m.count_alive(t);
        let dt = s.elapsed().as_secs_f64();
        let total = (closed as i64 + checked) as u64;
        println!("  alive {name}: {total} ({closed} closed form, {checked:+} from {people} people checked one by one), {:.3} ms", dt * 1e3);
        assert!(lo <= total && total <= hi, "exact count within the bounds");
        if mult <= 1.0 {
            for par in [false, true] {
                let s = Instant::now();
                let one = |&(c, y): &(u16, i32)| {
                    let mut k = 0u64;
                    let mut h = 0u64;
                    m.alive_in_cohort(c, y, t, &mut |x| {
                        k += 1;
                        h = h.wrapping_add(x.i);
                    });
                    std::hint::black_box(h);
                    k
                };
                let k: u64 = if par { cys.par_iter().map(one).sum() } else { cys.iter().map(one).sum() };
                let dt = s.elapsed().as_secs_f64();
                println!("    enumerated {k} in {:.3} s on {} ({:.1} ns per person)", dt, if par { "16 threads" } else { "1 thread" }, dt / k as f64 * 1e9);
                assert_eq!(k, total, "enumeration equals the count");
            }
        }
        if verify {
            let s = Instant::now();
            let brute: u64 = cys.par_iter().map(|&(cell, y)| (0..m.cohort_n(cell, y)).filter(|&i| m.alive_at(Pid { cell, y, i }, t)).count() as u64).sum();
            println!("    brute force: {brute} ({:.1} s) {}", s.elapsed().as_secs_f64(), if brute == total { "EQUAL" } else { "MISMATCH" });
        }
    }

    // Lookups on people drawn uniformly from everyone born by 2023.
    let sizes: Vec<(u16, i32, u64)> = cys.iter().map(|&(c, y)| (c, y, m.cohort_n(c, y))).collect();
    let draw = |k: u64| -> Pid {
        let mut r = Key::from_seed(7).with(k).below(ever);
        for &(cell, y, n) in &sizes {
            if r < n {
                return Pid { cell, y, i: r };
            }
            r -= n;
        }
        unreachable!()
    };
    let sample: Vec<Pid> = (0..200_000).map(draw).collect();
    let time = |name: &str, f: &dyn Fn(Pid) -> u64| {
        let mut v: Vec<u64> = Vec::with_capacity(sample.len());
        let mut h = 0u64;
        for &x in &sample {
            let s = Instant::now();
            h = h.wrapping_add(f(x));
            v.push(s.elapsed().as_nanos() as u64);
        }
        let mean = v.iter().sum::<u64>() as f64 / v.len() as f64;
        println!("  {name:>8}: p50 {:.3} µs, p99 {:.3} µs, mean {:.3} µs ({h:x})", pct(&mut v, 0.5) / 1e3, pct(&mut v, 0.99) / 1e3, mean / 1e3);
    };
    time("death", &|x| m.death(x) as u64);
    time("mother", &|x| m.mother(x).map_or(0, |p| p.i));
    time("father", &|x| m.father(x).map_or(0, |p| p.i));
    time("spouse", &|x| m.spouse(x).map_or(0, |p| p.0.i));
    time("children", &|x| m.children(x).len() as u64);
    let women: Vec<Pid> = sample.iter().copied().filter(|&x| m.sex(x) == Sex::Female).collect();
    let men: Vec<Pid> = sample.iter().copied().filter(|&x| m.sex(x) == Sex::Male).collect();
    let time_on = |name: &str, xs: &[Pid], f: &dyn Fn(Pid) -> u64| {
        let mut v: Vec<u64> = Vec::with_capacity(xs.len());
        let mut h = 0u64;
        for &x in xs {
            let s = Instant::now();
            h = h.wrapping_add(f(x));
            v.push(s.elapsed().as_nanos() as u64);
        }
        let mean = v.iter().sum::<u64>() as f64 / v.len() as f64;
        println!("  {name:>8}: p50 {:.3} µs, p99 {:.3} µs, mean {:.3} µs ({h:x})", pct(&mut v, 0.5) / 1e3, pct(&mut v, 0.99) / 1e3, mean / 1e3);
    };
    time_on("childrenw", &women, &|x| m.children(x).len() as u64);
    time_on("childrenm", &men, &|x| m.children(x).len() as u64);
    time("siblings", &|x| m.siblings(x).len() as u64);

    // First unions' ends: both partners agree, ends follow starts, and the
    // closed-form "separated and alive" count equals brute force per class.
    let mut bad = 0u64;
    for k in 0..50_000u64 {
        let x = draw(1_000_000 + k);
        if let Some((p2, start)) = m.spouse(x) {
            let (e1, e2) = (m.union_end(x), m.union_end(p2));
            if e1 != e2 || e1.is_none_or(|e| e.0 <= start) || m.separation(if m.sex(x) == Sex::Female { x } else { p2 }).is_some_and(|s| s <= start) {
                bad += 1;
            }
        }
    }
    println!("  union ends: {bad} inconsistent of 50000 sampled people");
    if mult <= 1.0 {
        let t = year_start(1990);
        let (mut checked, mut bad) = (0u64, 0u64);
        for cell in 0..m.cells() {
            for y in [1930, 1950, 1960] {
                for j in 0..m.wife_classes(cell, y) {
                    let mem = m.wife_class_members(cell, y, j);
                    if mem.is_empty() {
                        continue;
                    }
                    let dmax = m.sep_whole_years(y, j, t);
                    let brute = mem.clone().filter(|&i| {
                        let w = Pid { cell, y, i };
                        m.separation_class(w).is_some_and(|d| d < dmax) && m.death(w) > t
                    }).count() as u64;
                    checked += 1;
                    bad += (brute != m.separated_alive(cell, y, j, t)) as u64;
                }
            }
        }
        println!("  separated-and-alive closed form vs brute force: {bad} of {checked} classes differ");
    }

    // Realism: per cohort, women's ever-married at 50 (alive), CFR, e0, and
    // the age gap of first unions.
    println!("  cohort  e0 F/M      married@50  CFR   gap mean/sd/%wife older   first unions ending: separation/his death/her death   void seats");
    for y in [1880, 1920, 1950, 1980] {
        let cohort: Vec<(u16, u64)> = (0..m.cells()).map(|c| (c, m.cohort_n(c, y))).collect();
        let n: u64 = cohort.iter().map(|c| c.1).sum();
        let k = 4000u64;
        let (mut e0, mut married, mut alive50, mut kids, mut gaps) = ([0.0f64; 2], 0u64, 0u64, 0u64, Vec::new());
        let mut nsex = [0u64; 2];
        let mut ends = [0u64; 3];
        let (mut seats, mut voids) = (0u64, 0u64);
        for j in 0..k {
            let x = {
                let mut r = Key::from_seed(11).with2(y as u64, j).below(n);
                let mut at = Pid { cell: 0, y, i: 0 };
                for &(cell, nc) in &cohort {
                    if r < nc {
                        at = Pid { cell, y, i: r };
                        break;
                    }
                    r -= nc;
                }
                at
            };
            let s = (m.sex(x) == Sex::Male) as usize;
            let age = (m.death(x) - m.birth(x)) as f64 / 31_556_952.0;
            e0[s] += age;
            nsex[s] += 1;
            let sp = m.spouse(x);
            if s == 0 && age >= 50.0 {
                alive50 += 1;
                married += sp.is_some() as u64;
                kids += m.children(x).len() as u64;
            }
            if s == 0 && m.partner_seat(x).is_some() {
                seats += 1;
                voids += sp.is_none() as u64;
            }
            if s == 0 {
                if let Some((h, _)) = sp {
                    gaps.push((m.birth(x) - m.birth(h)) as f64 / 31_556_952.0);
                    ends[m.union_end(x).unwrap().1 as usize] += 1;
                }
            }
        }
        let (mu, sd) = {
            let mu = gaps.iter().sum::<f64>() / gaps.len() as f64;
            (mu, (gaps.iter().map(|g| (g - mu).powi(2)).sum::<f64>() / gaps.len() as f64).sqrt())
        };
        let older = gaps.iter().filter(|&&g| g < 0.0).count() as f64 / gaps.len() as f64;
        let te = ends.iter().sum::<u64>().max(1) as f64;
        println!(
            "  {y}    {:.1}/{:.1}   {:.1}%       {:.2}  {mu:.1}/{sd:.1}/{:.0}%            {:.0}/{:.0}/{:.0}%                                  {:.1}%",
            e0[0] / nsex[0] as f64,
            e0[1] / nsex[1] as f64,
            100.0 * married as f64 / alive50 as f64,
            kids as f64 / alive50 as f64,
            100.0 * older,
            100.0 * ends[0] as f64 / te,
            100.0 * ends[1] as f64 / te,
            100.0 * ends[2] as f64 / te,
            100.0 * voids as f64 / seats.max(1) as f64
        );
    }
}
