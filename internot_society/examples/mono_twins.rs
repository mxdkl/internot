//! Same-year siblings in the monotone world: births whose mother has
//! another child that year, by era and by the mother's kind.
use internot_society::mono::{Mono, Pid};
use internot_society::params::Sex;
use internot_society::Params;
use rayon::prelude::*;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.1);
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, 42, mult);
    for (lo, hi) in [(1840, 1900), (1900, 1950), (1950, 2000), (2000, 2050)] {
        let ys: Vec<(u16, i32)> = (0..m.cells()).flat_map(|c| (lo - 45..hi - 15).map(move |y| (c, y))).collect();
        // (births in [lo, hi), births sharing their year with a sibling, max in a year)
        let v = ys
            .par_iter()
            .map(|&(cell, y)| {
                let mut v = [0u64; 3];
                for i in 0..m.cohort_n(cell, y) {
                    let w = Pid { cell, y, i };
                    if m.sex(w) != Sex::Female {
                        continue;
                    }
                    let kids: Vec<Pid> = m.children(w).into_iter().filter(|c| c.y >= lo && c.y < hi).collect();
                    for (k, c) in kids.iter().enumerate() {
                        let same = kids.iter().enumerate().filter(|&(j, d)| j != k && d.y == c.y).count() as u64;
                        v[0] += 1;
                        v[1] += (same > 0) as u64;
                        v[2] = v[2].max(same + 1);
                    }
                }
                v
            })
            .reduce(|| [0; 3], |a, b| [a[0] + b[0], a[1] + b[1], a[2].max(b[2])]);
        println!("births {lo}–{hi}: {} ; with a same-year sibling {:.2}% ; most in one year {}", v[0], 100.0 * v[1] as f64 / v[0] as f64, v[2]);
    }
}
