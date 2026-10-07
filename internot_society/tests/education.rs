//! Education on the monotone world, over sampled people: the completed
//! level never falls over time and ends at the final level for those who
//! live through their schooling; enrollment matches the path's dates; schooling histories are
//! ordered and don't overlap. `MONO_MULT` (default 0.003), `MONO_SEEDS`
//! (default 42), `PACK` (default `us`).
use internot_society::mono::{EducationLevel, Mono, Pid, Schooling};
use internot_society::Params;
use procedural_core::key::Key;
use procedural_core::stream::year_start;
use rayon::prelude::*;

fn seeds() -> Vec<u64> {
    std::env::var("MONO_SEEDS").ok().map(|v| v.split(',').map(|s| s.trim().parse().unwrap()).collect()).unwrap_or(vec![42])
}

fn mult() -> f64 {
    std::env::var("MONO_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003)
}

#[test]
fn levels_dates_and_histories_agree() {
    let p = Params::embedded(&std::env::var("PACK").unwrap_or_else(|_| "us".into())).unwrap();
    for seed in seeds() {
        for m in [Mono::new(&p, seed, mult()), Mono::blind(&p, seed, mult())] {
            let mut xs = Vec::new();
            for c in 0..m.cells() {
                for y in (m.first_year()..=2010).step_by(3) {
                    let n = m.cohort_n(c, y);
                    for j in 0..n.min(30) {
                        xs.push(Pid { cell: c, y, i: Key::from_seed(4).with3(c as u64, y as u64, j).below(n) });
                    }
                }
            }
            let bad: Vec<String> = xs
                .par_iter()
                .flat_map_iter(|&x| {
                    let mut e = Vec::new();
                    let path = m.education_path(x);
                    if path.level != m.education_level(x) || path.latent != m.education_latent(x) {
                        e.push(format!("{x:?}: path and level disagree"));
                    }
                    // The completed level rises over the years.
                    let mut last: Option<EducationLevel> = None;
                    for age in (0..90).step_by(2) {
                        let t = m.birth(x) + age as i64 * 31_556_952;
                        let now = m.education_at(x, t);
                        if now < last {
                            e.push(format!("{x:?}: completed level fell from {last:?} to {now:?} at {age}"));
                        }
                        last = now.or(last);
                        // Enrolled only inside the path's dates.
                        let ok = match m.schooling_at(x, t) {
                            Schooling::School { grade } => t >= path.grade1 && t < path.school_end && (1..=12).contains(&grade),
                            Schooling::College { .. } => path.college.is_some_and(|(s, f)| t >= s && t < f),
                            Schooling::Graduate { .. } => path.graduate.is_some_and(|(s, f)| t >= s && t < f),
                            _ => true,
                        };
                        if !ok {
                            e.push(format!("{x:?}: enrolled outside the path's dates at {age}"));
                        }
                    }
                    // Whoever lives past the path's last date holds its level.
                    let done = path.graduate.map(|g| g.1).or(path.college.map(|c| c.1)).unwrap_or(path.school_end);
                    if m.death(x) > done && m.education_at(x, done) != Some(path.level) {
                        e.push(format!("{x:?}: when done holds {:?}, not its final {:?}", m.education_at(x, done), path.level));
                    }
                    // Histories: ordered, no overlap, within life.
                    if x.y >= 1900 {
                        let h = m.education_history(x, year_start(2024));
                        for w in h.windows(2) {
                            if w[1].from < w[0].to {
                                e.push(format!("{x:?}: stints overlap"));
                            }
                        }
                        if h.iter().any(|s| s.to < s.from || s.to > m.death(x)) {
                            e.push(format!("{x:?}: a stint outside life"));
                        }
                    }
                    e.into_iter()
                })
                .collect();
            for b in bad.iter().take(20) {
                eprintln!("{b}");
            }
            assert!(bad.is_empty(), "seed {seed}, {} cells: {} problems of {}", m.cells(), bad.len(), xs.len());
        }
    }
}
