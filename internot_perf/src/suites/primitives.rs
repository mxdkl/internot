//! Phase 0 primitives (spec §4): keys, samplers, keyed bijections,
//! matchings, partitions, counted spaces and event streams.
//!
//! Inputs are uniform over each structure's full domain, followed by a fixed
//! worst-case set (§16.4): domain edges, the largest domains, the most
//! expensive sampler regimes, the longest time spans.

use procedural_core::count::{CountTree, QuotaSpec, QuotaTree};
use procedural_core::key::Key;
use procedural_core::pairing::{Coupling, Pairing};
use procedural_core::partition::{Partition, SizeClasses, SplitTree};
use procedural_core::perm::{Bijection, CompactPerm, FeistelPerm, SmallPerm};
use procedural_core::sample::{binomial, lognormal, poisson, std_normal};
use procedural_core::stream::{hazard_time, poisson_buckets, PoissonTree, DAY};

use super::Suite;
use crate::harness::Harness;
use crate::inputs::InputRng;

pub const SUITE: Suite = Suite {
    name: "primitives",
    about: "procedural_core Phase 0 primitives over full domains plus worst cases",
    run,
};

const SEED: u64 = 0x9_0A5E_0000;

/// Largest id space the world uses: everyone ever born, US scale (~1.1e9),
/// rounded up to the 32-bit ceiling so the slowest Feistel shapes are covered.
const ID_SPACE: u64 = 4_294_967_295;

fn run(h: &mut Harness) {
    let mut rng = InputRng::new(SEED);
    let n = h.samples();
    let root = Key::from_seed(2026);

    // --- keys -----------------------------------------------------------
    let words: Vec<u64> = (0..n).map(|_| rng.next_u64()).collect();
    h.dist("key_with3_unit", &words, |&w| root.with3(w, 7, 9).unit());

    // --- samplers: every algorithm branch --------------------------------
    // Poisson mult (λ < 10) and PTRS; binomial inversion and BTPE. The
    // worst cases are the branch boundaries and the heaviest regimes.
    let lambdas: Vec<(u64, f64)> = (0..n)
        .map(|i| {
            let lam = match i % 4 {
                0 => rng.below(1000) as f64 / 100.0, // 0..10: mult
                1 => 10.0 + rng.below(1000) as f64,  // PTRS
                2 => 9.99,                           // mult worst case
                _ => 1e6,                            // PTRS, huge λ
            };
            (rng.next_u64(), lam)
        })
        .collect();
    h.dist("poisson", &lambdas, |&(w, lam)| poisson(root.with(w), lam));

    let binoms: Vec<(u64, u64, f64)> = (0..n)
        .map(|i| {
            let (nn, p) = match i % 4 {
                0 => (1 + rng.below(300), rng.below(100) as f64 / 1000.0), // inversion
                1 => (
                    1_000 + rng.below(1_000_000),
                    0.05 + rng.below(45) as f64 / 100.0,
                ), // BTPE
                2 => (300, 0.1),           // inversion worst: n·p = 30 boundary
                _ => (4_000_000_000, 0.5), // BTPE, huge n
            };
            (rng.next_u64(), nn, p)
        })
        .collect();
    h.dist("binomial", &binoms, |&(w, nn, p)| {
        binomial(root.with(w), nn, p)
    });

    h.dist("std_normal", &words, |&w| std_normal(root.with(w)));
    h.dist("lognormal", &words, |&w| lognormal(root.with(w), 10.8, 0.7));

    // --- keyed bijections ------------------------------------------------
    let feistel = FeistelPerm::new(ID_SPACE, root.with(1));
    let mut ids: Vec<u64> = (0..n).map(|_| rng.below(ID_SPACE)).collect();
    ids.extend([0, 1, ID_SPACE - 1, ID_SPACE / 2]);
    h.dist("feistel_fwd", &ids, |&x| feistel.fwd(x));
    h.dist("feistel_inv", &ids, |&y| feistel.inv(y));
    let compact = CompactPerm::new(ID_SPACE, root.with(1));
    h.dist("compact_fwd", &ids, |&x| compact.fwd(x));
    h.dist("compact_inv", &ids, |&y| compact.inv(y));

    let small_keys: Vec<u64> = (0..n.min(20_000)).map(|_| rng.next_u64()).collect();
    h.dist("small_perm_build_64", &small_keys, |&w| {
        SmallPerm::new(64, Key::from_bits(w))
    });

    // --- matchings ---------------------------------------------------------
    let pairing = Pairing::new(ID_SPACE, ID_SPACE - 1, root.with(2));
    h.dist("pairing_partner", &ids, |&x| pairing.partner(x));

    // A realistic union table: 30 cohorts, gap kernel ±4 cohorts.
    let blocks = 30usize;
    let sizes = vec![2_000_000u64; blocks];
    let mut plan = vec![0u64; blocks * blocks];
    for a in 0..blocks {
        for d in 0..5usize {
            if a + d < blocks {
                let m = if d == 0 { 150_000 } else { 90_000 / d as u64 };
                plan[a * blocks + a + d] = m;
                plan[(a + d) * blocks + a] = m;
            }
        }
    }
    let coupling = Coupling::new(&sizes, &plan, root.with(3)).expect("valid plan");
    let members: Vec<(usize, u64)> = (0..n)
        .map(|_| (rng.below(blocks as u64) as usize, rng.below(2_000_000)))
        .collect();
    h.dist("coupling_partner", &members, |&(b, m)| {
        coupling.partner(b, m)
    });

    // --- partitions and counted spaces -------------------------------------
    let households = SizeClasses::from_histogram(&[
        (1, 39_000_000),
        (2, 46_500_000),
        (3, 20_200_000),
        (4, 16_600_000),
        (5, 7_400_000),
        (6, 2_800_000),
        (7, 1_500_000),
    ]);
    let idx: Vec<u64> = (0..n).map(|_| rng.below(households.len())).collect();
    h.dist("size_classes_group_of", &idx, |&i| households.group_of(i));

    let split = SplitTree::new(ID_SPACE, 1_000, 5_000, root.with(4));
    h.dist("split_tree_leaf_of", &ids, |&i| split.leaf_of(i));

    let count = CountTree::new(root.with(5), 24, 3_000_000_000, |lo, hi| (hi - lo) as f64);
    let leaves: Vec<u64> = (0..n).map(|_| rng.below(1 << 24)).collect();
    h.dist("count_tree_count_before_24", &leaves, |&l| {
        count.count_before(l)
    });
    h.dist("count_tree_select_24", &leaves, |&l| count.select(l * 170));

    let quota = QuotaTree::new(&union_like_cells()).expect("consistent margins");
    let ranks: Vec<u64> = (0..n).map(|_| rng.below(quota.total())).collect();
    h.dist("quota_locate_4lvl", &ranks, |&r| quota.locate(r));

    // --- event streams -----------------------------------------------------
    // A month of a busy dyad (5 events/day) at arbitrary absolute dates.
    let starts: Vec<i64> = (0..n.min(20_000))
        .map(|_| rng.below(110_000) as i64 * DAY)
        .collect();
    let mut out = Vec::with_capacity(1_024);
    h.dist("poisson_buckets_30d", &starts, |&t| {
        out.clear();
        poisson_buckets(root.with(6), DAY, |_| 5.0, t, t + 30 * DAY, &mut out);
        out.len()
    });

    // Sparse stream over 1800-2517 (2^18 days): "how many before t".
    let tree = PoissonTree::new(root.with(7), 0, DAY, 18, |a: i64, b: i64| {
        (b - a) as f64 / (200.0 * DAY as f64)
    });
    let times: Vec<i64> = (0..n).map(|_| rng.below(1 << 18) as i64 * DAY).collect();
    h.dist("poisson_tree_count_before", &times, |&t| {
        tree.count_before(t)
    });
    h.dist("poisson_tree_last_before", &times, |&t| tree.last_before(t));

    // A life-course hazard with annual pieces over 100 years (worst case: never fires).
    let draws: Vec<f64> = (0..n.min(50_000))
        .map(|i| {
            if i % 10 == 0 {
                1e9
            } else {
                rng.below(1_000) as f64 / 100.0
            }
        })
        .collect();
    h.dist("hazard_time_100y_annual", &draws, |&e| {
        hazard_time(e, 0, 100 * 365 * DAY, |t| {
            (0.05 / (365 * DAY) as f64, (t / (365 * DAY) + 1) * 365 * DAY)
        })
    });
}

/// A quota tree shaped like one birth block's life line: sex → union-start
/// year → partner cohort → fertility plan.
fn union_like_cells() -> Vec<QuotaSpec> {
    (0..2)
        .map(|sex| {
            QuotaSpec::node(
                sex,
                (0..40)
                    .map(|year| {
                        QuotaSpec::node(
                            year,
                            (0..12)
                                .map(|cohort| {
                                    QuotaSpec::node(
                                        cohort,
                                        (0..6)
                                            .map(|plan| {
                                                QuotaSpec::leaf(
                                                    plan,
                                                    50 + ((year * 7 + cohort * 3 + plan) % 40)
                                                        as u64,
                                                )
                                            })
                                            .collect(),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}
