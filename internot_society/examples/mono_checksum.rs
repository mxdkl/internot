//! A checksum of the monotone world's answers, to prove that a speed change
//! leaves the world unchanged: every lookup on 200,000 people drawn
//! uniformly from everyone born by 2023 (full answers, not just counts),
//! plus the alive counts, their bounds and the separated-and-alive counts.
//! `PACK` (default `us`), `MULT` (default 1), `BLIND=1` for the
//! heritage-blind world (one cell). People hash as `(y − 1000)·2⁴² + i`
//! plus `cell·2⁵⁶`, so a one-cell world's checksum is comparable with the
//! single-cell world's before cells.
use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;

fn main() {
    let pack = std::env::var("PACK").unwrap_or_else(|_| "us".into());
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let p = Params::embedded(&pack).unwrap();
    let blind = std::env::var("BLIND").is_ok_and(|v| v == "1");
    let m = if blind { Mono::blind(&p, 42, mult) } else { Mono::new(&p, 42, mult) };
    let sizes: Vec<(u16, i32, u64)> = (0..m.cells())
        .flat_map(|c| (m.first_year()..=2023).map(move |y| (c, y)))
        .map(|(c, y)| (c, y, m.cohort_n(c, y)))
        .collect();
    let ever: u64 = sizes.iter().map(|s| s.2).sum();
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
    let xs: Vec<Pid> = (0..200_000).map(draw).collect();
    let mix = |h: u64, v: u64| (h ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    let id = |p: Pid| ((p.cell as u64) << 56) | (((p.y - 1000) as u64) << 42) | p.i;
    let pid = |x: Option<Pid>| x.map_or(u64::MAX, id);
    let parts: Vec<(&str, Box<dyn Fn(Pid) -> u64>)> = vec![
        ("birth", Box::new(|x| m.birth(x) as u64)),
        ("death", Box::new(|x| m.death(x) as u64)),
        ("mother", Box::new(|x| pid(m.mother(x)))),
        ("father", Box::new(|x| pid(m.father(x)))),
        ("seat", Box::new(|x| m.partner_seat(x).map_or(u64::MAX, |(p, s)| mix(id(p), s as u64)))),
        ("spouse", Box::new(|x| m.spouse(x).map_or(u64::MAX, |(p, s)| mix(id(p), s as u64)))),
        ("children", Box::new(|x| m.children(x).iter().fold(7, |h, c| mix(h, id(*c))))),
        ("siblings", Box::new(|x| m.siblings(x).iter().fold(7, |h, c| mix(h, id(*c))))),
        ("union_end", Box::new(|x| m.union_end(x).map_or(u64::MAX, |(t, k)| mix(t as u64, k as u64)))),
        ("sep_class", Box::new(|x| m.separation_class(x).map_or(u64::MAX, |d| d as u64))),
    ];
    let mut all = 0u64;
    for (name, f) in &parts {
        let h = xs.iter().fold(1u64, |h, &x| mix(h, f(x)));
        all = mix(all, h);
        println!("{name:>10}: {h:016x}");
    }
    let dates = [year_start(1900), year_start(1950), year_start(2023), year_start(2023) + 287 * 86_400, year_start(2060) + 1234];
    let mut h = 1u64;
    for &t in &dates {
        let (a, b, c) = m.count_alive(t);
        let (lo, hi) = m.alive_bounds(t);
        h = mix(mix(mix(mix(mix(h, a), b as u64), c), lo), hi);
    }
    all = mix(all, h);
    println!("{:>10}: {h:016x}", "alive");
    let mut h = 1u64;
    for cell in 0..m.cells() {
        for y in [1900, 1930, 1950, 1960, 1990] {
            for j in 0..m.wife_classes(cell, y) {
                h = mix(h, m.separated_alive(cell, y, j, year_start(2000)));
            }
        }
    }
    all = mix(all, h);
    println!("{:>10}: {h:016x}", "separated");
    println!("{:>10}: {all:016x}", "ALL");
}
