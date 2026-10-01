//! Calibrates heritage groups' adult mortality factors (heritage.ron) to
//! life-expectancy gaps, and prints what the pack's current factors give.
//!
//! Age-adjusted death-rate ratios weight old ages, so they understate the
//! young-age excess that drives life-expectancy gaps (Black e0 was 14.6 years
//! below White in 1900 with a death-rate ratio of only 1.37). So the adult
//! factor of each group is solved, year by year, to reproduce its period e0
//! gap to White (data in research/2026-10-01-heritage-and-names.md §5c),
//! keeping the pack's infant factors and White factors.
//!
//! ```sh
//! cargo run --release -p internot_society --example calibrate_group_mortality
//! ```

use internot_society::params::{MortalityFactor, MAX_AGE};
use internot_society::{Heritage, Params, Sex};

/// Period e0 gaps to White (years), by group id and year: NCHS (nonwhite
/// before 1970) for Black; NCHS 2006–2024 for Hispanic, Asian, AIAN;
/// Hacker & Haines 2010 and IHS for AIAN before 2000.
const GAPS: &[(&str, &[(i32, f64)])] = &[
    (
        "black",
        &[
            (1850, -16.5),
            (1900, -14.6),
            (1920, -9.6),
            (1940, -11.1),
            (1950, -8.3),
            (1970, -7.6),
            (1990, -7.0),
            (2000, -5.5),
            (2010, -3.8),
            (2023, -4.4),
        ],
    ),
    (
        "aian",
        &[
            (1900, -13.0),
            (1940, -12.6),
            (1972, -8.1),
            (2000, -5.0),
            (2019, -7.0),
            (2023, -8.3),
        ],
    ),
    (
        "hispanic",
        &[(2006, 2.1), (2010, 2.9), (2019, 3.1), (2023, 2.9)],
    ),
    ("asian", &[(2019, 6.8), (2023, 6.8)]),
];

fn e0(p: &Params, year: i32, f: MortalityFactor) -> f64 {
    let one = |sex| {
        let (mut l, mut e) = (1.0, 0.0);
        for a in 0..=MAX_AGE {
            let q = p.mortality.death_prob_scaled(sex, a, year, f);
            e += l * (1.0 - q / 2.0);
            l *= 1.0 - q;
        }
        e
    };
    0.5 * (one(Sex::Female) + one(Sex::Male))
}

fn main() {
    let p = Params::prototype();
    let her = &p.heritage;
    let white = her.find("white").expect("a white group");
    println!("group     year   e0 now   gap now   gap target   adult now   adult solved");
    for (id, gaps) in GAPS {
        let Some(h) = her.find(id) else { continue };
        for &(year, target) in *gaps {
            let fw = her.mortality_factor(white, year);
            let ew = e0(&p, year, fw);
            let f = her.mortality_factor(h, year);
            let now = e0(&p, year, f);
            // e0 falls as the adult factor rises: bisect on its log.
            let (mut lo, mut hi) = (-3.0f64, 3.0f64);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                let e = e0(
                    &p,
                    year,
                    MortalityFactor {
                        infant: f.infant,
                        adult: mid.exp(),
                    },
                );
                if e - ew > target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            println!(
                "{id:<9} {year}   {now:>6.1}   {:>7.1}   {target:>10.1}   {:>9.3}   {:>12.3}",
                now - ew,
                f.adult,
                (0.5 * (lo + hi)).exp()
            );
        }
    }
    let _ = Heritage(0);
}
