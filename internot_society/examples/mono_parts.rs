//! Mean cost of each internal step of the monotone world's lookups over a
//! million people drawn uniformly from everyone born by 2023. `BLIND=1` for
//! the heritage-blind world.
use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use procedural_core::key::Key;

fn main() {
    let p = Params::embedded("us").unwrap();
    let m = if std::env::var("BLIND").is_ok_and(|v| v == "1") { Mono::blind(&p, 42, 1.0) } else { Mono::new(&p, 42, 1.0) };
    let sizes: Vec<(u16, i32, u64)> =
        (0..m.cells()).flat_map(|c| (m.first_year()..=2023).map(move |y| (c, y))).map(|(c, y)| (c, y, m.cohort_n(c, y))).collect();
    let ever: u64 = sizes.iter().map(|s| s.2).sum();
    let xs: Vec<Pid> = (0..1_000_000u64)
        .map(|k| {
            let mut r = Key::from_seed(7).with(k).below(ever);
            for &(cell, y, n) in &sizes {
                if r < n {
                    return Pid { cell, y, i: r };
                }
                r -= n;
            }
            unreachable!()
        })
        .collect();
    // HOT=1: a thousand people repeated (in cache), so compute alone.
    let xs: Vec<Pid> = if std::env::var("HOT").is_ok() { (0..1_000_000).map(|k| xs[k % 1000]).collect() } else { xs };
    for _ in 0..2 {
        for (name, ns) in m.profile_parts(&xs) {
            println!("{name:>34}: {ns:6.1} ns");
        }
        println!();
    }
}
