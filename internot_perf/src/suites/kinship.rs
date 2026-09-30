//! R1 kinship (spec §6, §16.2 "one hop" kin): people, unions, parents and
//! children on the prototype world (two lineage regions, 1840–2100, ~5M
//! people ever born).
//!
//! Inputs are uniform over every id ever born, plus a worst-case set: the
//! women with the most children and the men whose unions went through kin
//! repair checks in the busiest cohorts.

use internot_society::{Params, PersonId, Sex, World};

use super::Suite;
use crate::harness::Harness;
use crate::inputs::InputRng;

pub const SUITE: Suite = Suite {
    name: "kinship",
    about: "internot_society people and kinship lookups on the R1 prototype world",
    run,
};

const SEED: u64 = 0x4B1_5E1F;

fn run(h: &mut Harness) {
    let w = World::build(Params::prototype(), 42);
    let n = w.population();
    let mut rng = InputRng::new(SEED);
    let mut ids: Vec<PersonId> = (0..h.samples()).map(|_| rng.below(n) as PersonId).collect();

    // Worst cases: the most prolific mothers in a 1900 cohort and their men.
    let blocks = w.ledger().blocks_of_year(1900);
    let base = w.ledger().base[blocks.start as usize] as PersonId;
    let end = w.ledger().base[blocks.end as usize] as PersonId;
    let mut mothers: Vec<(usize, PersonId)> = (base..end)
        .step_by(7)
        .filter(|&x| w.sex(x) == Sex::Female)
        .map(|x| (w.children(x).len(), x))
        .collect();
    mothers.sort_unstable_by(|a, b| b.cmp(a));
    for &(_, m) in mothers.iter().take(32) {
        ids.push(m);
        if let Some(u) = w.union(m) {
            ids.push(u.partner);
        }
    }

    // Lookups over a ~60 MB world are memory-bound: each is timed one call
    // per sample, since a batch repeats one input and would run its later
    // calls from cache. `birth` (a hash, no tables) keeps batching.
    h.dist("birth", &ids, |&x| w.birth(x));
    h.bench("death").batch(1).run(&ids, |&x| w.death(x));
    h.bench("mother").batch(1).run(&ids, |&x| w.mother(x));
    h.bench("father").batch(1).run(&ids, |&x| w.father(x));
    h.bench("union").batch(1).run(&ids, |&x| w.union(x));
    h.bench("children").batch(1).run(&ids, |&x| w.children(x));
    h.bench("siblings").batch(1).run(&ids, |&x| w.siblings(x));

    // Building the world (ledger plus lookup structure), once per seed and
    // parameter set: spec §16.2 "ledger build".
    h.bench("world_build")
        .samples(5)
        .batch(1)
        .run(&[42u64], |&seed| World::build(Params::prototype(), seed));
}
