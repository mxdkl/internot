//! How long "who works at establishment E at t" takes today, with no
//! roster index: scan everyone alive at t, keep those whose job then is in
//! E's industry, and resolve their employers (one home lookup each).
//! `MULT` (default 1), `THREADS` (default all).
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use internot_society::mono::{Employer, Mono, Pid, WorkSpell};
use internot_society::Params;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    if let Some(n) = std::env::var("THREADS").ok().and_then(|v| v.parse().ok()) {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global().unwrap();
    }
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, 42, mult);
    let t = year_start(2025) + 287 * 86_400; // 2025-10-15
    let cohorts: Vec<(u16, i32)> = (0..m.cells()).flat_map(|c| (m.first_year()..=2009).map(move |y| (c, y))).collect();

    // 1. Everyone alive at t, 16+.
    let s = Instant::now();
    let alive: Vec<Pid> = cohorts
        .par_iter()
        .flat_map_iter(|&(c, y)| {
            let mut v = Vec::new();
            m.alive_in_cohort(c, y, t, &mut |x| v.push(x));
            v
        })
        .collect();
    let t_enum = s.elapsed().as_secs_f64();

    // 2. The spell in force for each.
    let s = Instant::now();
    let jobs: Vec<(Pid, internot_society::mono::Job)> = alive
        .par_iter()
        .filter_map(|&x| match m.work_at(x, t) {
            Some(WorkSpell::Employed(j)) => Some((x, j)),
            _ => None,
        })
        .collect();
    let t_work = s.elapsed().as_secs_f64();

    // A target: the employer of a worker at a large establishment.
    let (tx, tj) = jobs
        .iter()
        .find(|(x, j)| matches!(m.employer(*x, j), Employer::Establishment { class, .. } if class >= 7))
        .copied()
        .expect("someone at a large establishment");
    let target = m.employer(tx, &tj);
    let info = m.employer_info(target);

    // 3. Same industry, then resolve employers.
    let s = Instant::now();
    let candidates: Vec<&(Pid, internot_society::mono::Job)> = jobs.iter().filter(|(_, j)| j.industry == tj.industry).collect();
    let resolved = AtomicU64::new(0);
    let staff: Vec<Pid> = candidates
        .par_iter()
        .filter_map(|(x, j)| {
            resolved.fetch_add(1, Ordering::Relaxed);
            (m.employer(*x, j) == target).then_some(*x)
        })
        .collect();
    let t_emp = s.elapsed().as_secs_f64();

    println!("world ×{mult}: {} alive 16+ at 2025-10-15, {} employed; the world is {:.4} of the real population", alive.len(), jobs.len(), m.sample_share(2025));
    println!("target: {} ({}, {}, {}, {:?})", info.name, info.industry, info.city, info.state, target);
    println!("  1. enumerate the alive:      {:8.3} s", t_enum);
    println!("  2. work_at for each:         {:8.3} s", t_work);
    println!("  3. employers of {} in the industry: {:8.3} s", resolved.load(Ordering::Relaxed), t_emp);
    println!("  total {:.3} s on {} threads; {} work there", t_enum + t_work + t_emp, rayon::current_num_threads(), staff.len());
}
