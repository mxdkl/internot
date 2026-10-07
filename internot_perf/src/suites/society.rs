//! The society world (`internot_society::mono`, the monotone world): people
//! and kinship lookups, the closed-form "alive at a date" count and its
//! instant bounds, and the world build, on the `us` pack at its own scale
//! (about 14M people born by 2023; the world's memory and lookups don't
//! grow with scale).
//!
//! Inputs are uniform over everyone born by 2023. Lookups are timed one
//! call per sample (a batch repeats one input and would run from cache).

use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use procedural_core::stream::year_start;

use super::Suite;
use crate::harness::Harness;
use crate::inputs::InputRng;

pub const SUITE: Suite = Suite {
    name: "society",
    about: "internot_society monotone world: lookups, alive counts and build",
    run,
};

const SEED: u64 = 0x5_0C1E7;

fn run(h: &mut Harness) {
    let p: &'static Params = Box::leak(Box::new(Params::embedded("us").expect("the us pack")));
    let w = Mono::new(p, 42, 1.0);
    let sizes: Vec<(u16, i32, u64)> = (0..w.cells())
        .flat_map(|c| (w.first_year()..=2023).map(move |y| (c, y)))
        .map(|(c, y)| (c, y, w.cohort_n(c, y)))
        .collect();
    let ever: u64 = sizes.iter().map(|s| s.2).sum();
    let mut rng = InputRng::new(SEED);
    let ids: Vec<Pid> = (0..h.samples())
        .map(|_| {
            let mut r = rng.below(ever);
            for &(cell, y, n) in &sizes {
                if r < n {
                    return Pid { cell, y, i: r };
                }
                r -= n;
            }
            unreachable!()
        })
        .collect();

    h.bench("birth").batch(1).run(&ids, |&x| w.birth(x));
    h.bench("death").batch(1).run(&ids, |&x| w.death(x));
    h.bench("mother").batch(1).run(&ids, |&x| w.mother(x));
    h.bench("father").batch(1).run(&ids, |&x| w.father(x));
    h.bench("spouse").batch(1).run(&ids, |&x| w.spouse(x));
    h.bench("children").batch(1).run(&ids, |&x| w.children(x));
    h.bench("siblings").batch(1).run(&ids, |&x| w.siblings(x));
    // Names: each input's birth-year tables built first (the cold cost is
    // a few ms per year and sex, once per process).
    let t = year_start(2023);
    for &x in &ids {
        std::hint::black_box(w.full_name(x, t));
    }
    h.bench("full_name").batch(1).run(&ids, |&x| w.full_name(x, t));

    // Work: a whole career (16 to death), the spell in force at 2023, and
    // the employer of that job (name, industry, place: one home lookup).
    // Warm: the occupation tables are loaded first.
    std::hint::black_box(w.career(ids[0]));
    h.bench("career").batch(1).run(&ids, |&x| w.career(x).len());
    h.bench("work_at").batch(1).run(&ids, |&x| w.work_at(x, t).is_some());
    let jobs: Vec<(Pid, internot_society::mono::Job)> = ids
        .iter()
        .filter_map(|&x| match w.work_at(x, t) {
            Some(internot_society::mono::WorkSpell::Employed(j)) => Some((x, j)),
            _ => None,
        })
        .collect();
    for (x, j) in jobs.iter().take(64) {
        std::hint::black_box(w.employer_info(w.employer(*x, j)));
    }
    h.bench("employer").batch(1).run(&jobs, |(x, j)| w.employer_info(w.employer(*x, j)).name.len());

    // Everyone alive at a date: the exact count (closed form plus the
    // infant band checked one by one) and its instant bounds.
    let dates = [year_start(2023), year_start(1950), year_start(2023) + 287 * 86_400];
    h.bench("count_alive").samples(30).batch(1).run(&dates, |&t| w.count_alive(t));
    h.bench("alive_bounds").samples(30).batch(1).run(&dates, |&t| w.alive_bounds(t));

    h.bench("world_build")
        .samples(5)
        .batch(1)
        .run(&[42u64], |&seed| Mono::new(p, seed, 1.0).cohort_n(0, 2000));
}
