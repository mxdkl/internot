//! Close-kin couples in the monotone world: every first union (wives of
//! every cohort), checked for siblings, half-siblings and parent and child.
//! `MULT=1 cargo run --release -p internot_society --example mono_kin`.
use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use rayon::prelude::*;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let seed: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(42);
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, seed, mult);
    let ys: Vec<(u16, i32)> = (0..m.cells()).flat_map(|c| (m.first_year()..=m.last_year()).map(move |y| (c, y))).collect();
    // (couples, same mother, same father only, mother–son, father–daughter)
    let v = ys
        .par_iter()
        .map(|&(cell, y)| {
            let mut v = [0u64; 5];
            for i in 0..m.cohort_n(cell, y) {
                let w = Pid { cell, y, i };
                if m.sex(w) != internot_society::params::Sex::Female {
                    continue;
                }
                let Some((h, _)) = m.spouse(w) else { continue };
                v[0] += 1;
                let (mw, mh) = (m.mother(w), m.mother(h));
                let (fw, fh) = (m.father(w), m.father(h));
                if mw.is_some() && mw == mh {
                    v[1] += 1;
                    eprintln!("siblings: {w:?} {h:?}");
                } else if fw.is_some() && fw == fh {
                    v[2] += 1;
                }
                if mh == Some(w) {
                    v[3] += 1;
                    eprintln!("mother–son: {w:?} {h:?}");
                }
                if fw == Some(h) {
                    v[4] += 1;
                    eprintln!("father–daughter: {w:?} {h:?}");
                }
            }
            v
        })
        .reduce(|| [0; 5], |a, b| std::array::from_fn(|k| a[k] + b[k]));
    println!("seed {seed}, ×{mult}: {} couples; same mother {}, same father only {}, mother–son {}, father–daughter {}", v[0], v[1], v[2], v[3], v[4]);
}
