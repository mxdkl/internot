//! Example suite: the Layer 0 hash and sampler primitives over uniformly
//! sampled 128-bit ids. It proves the plumbing end to end and tracks the
//! cost floor every procedural attribute pays.

use procedural_core::hash::{hash_float, hash_int};
use procedural_core::sampler::lognormal;

use super::Suite;
use crate::harness::Harness;
use crate::inputs::InputRng;

pub const SUITE: Suite = Suite {
    name: "core_hash",
    about: "procedural_core hash_int, hash_float and lognormal over uniform u128 ids",
    run,
};

const SEED: u64 = 0x1D5_C0DE;

fn run(h: &mut Harness) {
    let mut rng = InputRng::new(SEED);
    let ids: Vec<u128> = (0..h.samples()).map(|_| rng.next_u128()).collect();

    h.dist("hash_int", &ids, |&id| hash_int(id, "age", 100));
    h.dist("hash_float", &ids, |&id| hash_float(id, "income"));
    h.dist("lognormal", &ids, |&id| lognormal(id, "income", 10.8, 0.7));
}
