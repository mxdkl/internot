//! Counts close-kin couples (siblings by either parent, parent and child)
//! over every union of every person, in parallel, and lists the first few.
//! Kin repair makes them rare, not impossible: a repair group whose every
//! pairing is related cannot be fixed (AGENTS.md, "close kin").
//! Run: cargo run --release -p internot_society --example close_kin [pack]
//! (a pack in `worlds/`; default `us-tiny`).
use internot_society::{Params, PersonId, World};
use rayon::prelude::*;

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "us-tiny".into());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../worlds");
    let w = World::build(Params::load(&root, &name).expect("a pack in worlds/"), 7);
    let n = w.population() as PersonId;
    let t = std::time::Instant::now();
    let found: Vec<(PersonId, PersonId, bool, bool, bool)> = (0..n)
        .into_par_iter()
        .flat_map_iter(|x| {
            let w = &w;
            w.unions(x).into_iter().flatten().filter_map(move |u| {
                let m = u.partner;
                let mother = w.mother(x).is_some() && w.mother(x) == w.mother(m);
                let father = w.father(x).is_some() && w.father(x) == w.father(m);
                let parent = w.mother(m) == Some(x)
                    || w.father(m) == Some(x)
                    || w.mother(x) == Some(m)
                    || w.father(x) == Some(m);
                (mother || father || parent).then_some((x, m, mother, father, parent))
            })
        })
        .collect();
    let seats: usize = (0..n)
        .into_par_iter()
        .map(|x| w.unions(x).iter().flatten().count())
        .sum();
    println!(
        "{name}: {n} people, {seats} union seats, {} close-kin seats (each couple counted from both sides; {:.1} s)",
        found.len(),
        t.elapsed().as_secs_f64()
    );
    for &(x, m, mother, father, parent) in found.iter().take(6) {
        println!(
            "  {x} × {m}: same mother {mother}, same father {father}, parent and child {parent}; groups {} and {}, born {} and {}",
            w.lineage_group(x),
            w.lineage_group(m),
            w.birth_year(x),
            w.birth_year(m)
        );
    }
}
