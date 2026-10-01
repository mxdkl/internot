//! A fingerprint of whole worlds: a hash of every ledger block, and a
//! checksum of kinship, union and household answers over sampled ids. Two
//! builds with equal fingerprints give the same worlds; refactors that must
//! not change the world (moving parameters into world packs, build-time
//! work) are checked with it.
//!
//! ```sh
//! cargo run --release -p internot_society --example world_fingerprint
//! ```

use std::fmt::Write as _;

use internot_society::world::{year_start, DAY};
use internot_society::{Params, PersonId, World};

/// FNV-1a over everything written to it.
struct Fnv(u64);

impl std::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for b in s.bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(())
    }
}

fn fingerprint(name: &str, w: &World, samples: usize) {
    let ledger = w.ledger();
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    // Heritage groups print as their index whatever their representation.
    let mut blocks = format!("{:?}", ledger.blocks);
    for (i, name) in ["White", "Black", "Aian", "Asian", "Hispanic"]
        .iter()
        .enumerate()
    {
        blocks = blocks.replace(&format!("heritage: {name},"), &format!("heritage: {i},"));
    }
    write!(h, "{blocks}{:?}{}", ledger.base, ledger.class_moves).unwrap();

    let n = w.population();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut sum = 0u64;
    let mut mix = |v: u64| sum = sum.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
    let dates: Vec<i64> = [1900, 1950, 1985, 2020]
        .iter()
        .map(|&y| year_start(y) + 100 * DAY)
        .collect();
    for _ in 0..samples {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = (x % n) as PersonId;
        mix(w.birth(id) as u64);
        mix(w.death(id) as u64);
        mix(w.mother(id).map_or(0, u64::from));
        mix(w.father(id).map_or(0, u64::from));
        for u in w.unions(id).into_iter().flatten() {
            mix(u.partner as u64);
            mix(u.start as u64);
            mix(u.end as u64);
        }
        mix(w.children(id).iter().map(|&c| c as u64).sum());
        mix(w.siblings(id).iter().map(|&c| c as u64).sum());
        for &t in &dates {
            if let Some(hh) = w.household(id, t) {
                write!(h, "{hh:?}").unwrap();
            }
        }
    }
    println!("{name}: ledger+households {:016x}  lookups {sum:016x}", h.0);
}

fn main() {
    fingerprint("tiny", &World::build(Params::tiny(), 7), 20_000);
    fingerprint("prototype", &World::build(Params::prototype(), 42), 20_000);
}
