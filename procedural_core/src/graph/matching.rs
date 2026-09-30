//! Irving's stable-roommates matching (Irving 1985).
//!
//! Gender-agnostic; symmetric in argument order by construction.
//! Given a cohort `[u32]` and a symmetric preference function
//! `pref(a, b) -> f64` (higher = more preferred), returns a list of
//! matched pairs in canonical small-id-first order.
//!
//! ## v1 simple impl (not full Irving)
//!
//! 1. Each participant ranks every other participant by `pref`.
//! 2. Round-robin proposing: each unmatched `i` proposes to its
//!    most-preferred not-yet-rejected partner. If the target prefers
//!    `i` to its current partner (or has none), swap.
//! 3. Iterate until stable or `n²` rounds elapse.
//!
//! v1 does NOT implement Irving's Phase 2 rotation-elimination, so
//! on cohorts where no stable matching exists, the output is a
//! partial matching (some members unmatched). For v1 cohort sizes
//! (~250 per `(region, year)` bucket), this is adequate.

use std::cmp::Ordering;

/// Match a cohort via stable-roommates. Returns matched pairs as
/// `(min_id, max_id)`. Unmatched cohort members are silently omitted.
pub fn stable_roommates_match<F>(cohort: &[u32], pref: F) -> Vec<(u32, u32)>
where
    F: Fn(u32, u32) -> f64,
{
    let n = cohort.len();
    if n < 2 {
        return Vec::new();
    }
    debug_assert!(
        n <= 65_535,
        "stable_roommates_match: n² overflows usize for n > 65535; got {n}",
    );
    // Preference rank lists. rank[i] = vec of other indices sorted
    // descending by pref(cohort[i], cohort[other]).
    let rank: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            let mut others: Vec<usize> = (0..n).filter(|&j| j != i).collect();
            others.sort_by(|&a, &b| {
                let pa = pref(cohort[i], cohort[a]);
                let pb = pref(cohort[i], cohort[b]);
                // NaN treated as equal; pref(a, b) must be finite in
                // practice (callers derive from hash_float ∈ [0, 1)).
                pb.partial_cmp(&pa).unwrap_or(Ordering::Equal)
            });
            others
        })
        .collect();
    // head[i] = next index into rank[i] to TRY on i's next proposal.
    // Advanced when i is rejected (target prefers current partner)
    // AND when i wins a swap (so if i is later displaced, it doesn't
    // waste a round re-proposing to the partner it already won-then-
    // lost). Stable-roommates protocol: once you've offered, you
    // don't re-offer to the same target.
    let mut head: Vec<usize> = vec![0; n];
    let mut partner: Vec<Option<usize>> = vec![None; n];
    let mut changed = true;
    let mut iter = 0_usize;
    while changed && iter < n * n {
        changed = false;
        iter += 1;
        for i in 0..n {
            if partner[i].is_some() {
                continue;
            }
            while head[i] < rank[i].len() {
                let target = rank[i][head[i]];
                let p_target_to_i = pref(cohort[target], cohort[i]);
                match partner[target] {
                    None => {
                        partner[i] = Some(target);
                        partner[target] = Some(i);
                        changed = true;
                        break;
                    }
                    Some(current) => {
                        let p_target_to_curr = pref(cohort[target], cohort[current]);
                        if p_target_to_i > p_target_to_curr {
                            partner[current] = None;
                            partner[i] = Some(target);
                            partner[target] = Some(i);
                            head[i] += 1;
                            changed = true;
                            break;
                        } else {
                            head[i] += 1;
                            continue;
                        }
                    }
                }
            }
        }
    }
    // Collect canonical (lo, hi) pairs, deduped.
    let mut out = Vec::new();
    let mut emitted = vec![false; n];
    for i in 0..n {
        if let Some(j) = partner[i] {
            if !emitted[i] && !emitted[j] {
                let a = cohort[i];
                let b = cohort[j];
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                out.push((lo, hi));
                emitted[i] = true;
                emitted[j] = true;
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::util::pair_hash_float;

    #[test]
    fn empty_cohort_returns_empty() {
        let out = stable_roommates_match(&[], |_, _| 1.0);
        assert!(out.is_empty());
    }

    #[test]
    fn singleton_cohort_returns_empty() {
        let out = stable_roommates_match(&[7], |_, _| 1.0);
        assert!(out.is_empty());
    }

    #[test]
    fn pair_cohort_matches_them() {
        let out = stable_roommates_match(&[7, 42], |_, _| 1.0);
        assert_eq!(out, vec![(7, 42)]);
    }

    #[test]
    fn output_pairs_are_canonical_order() {
        let out = stable_roommates_match(&[42, 7], |_, _| 1.0);
        for (lo, hi) in &out {
            assert!(lo < hi, "expected canonical ordering; got ({lo}, {hi})");
        }
    }

    #[test]
    fn deterministic_on_repeated_calls() {
        let cohort = vec![10, 20, 30, 40];
        let pref = |a: u32, b: u32| pair_hash_float(a, b, "pref");
        let r1 = stable_roommates_match(&cohort, pref);
        let r2 = stable_roommates_match(&cohort, pref);
        assert_eq!(r1, r2);
    }

    #[test]
    fn no_member_appears_twice() {
        let cohort: Vec<u32> = (1..=20).collect();
        let pref = |a: u32, b: u32| pair_hash_float(a, b, "pref");
        let out = stable_roommates_match(&cohort, pref);
        let mut seen: Vec<u32> = out.iter().flat_map(|p| [p.0, p.1]).collect();
        seen.sort();
        let dedup_len = {
            let mut v = seen.clone();
            v.dedup();
            v.len()
        };
        assert_eq!(seen.len(), dedup_len, "duplicate member in output");
    }

    #[test]
    fn even_cohort_of_four_matches_all_members() {
        // With a symmetric hash-derived pref, the greedy algorithm
        // converges to a full matching on this 4-cohort. `pair_hash_float`
        // is deterministic, so the count of 2 is fixed across runs.
        let cohort = vec![1, 2, 3, 4];
        let pref = |a: u32, b: u32| pair_hash_float(a, b, "pref");
        let out = stable_roommates_match(&cohort, pref);
        assert_eq!(out.len(), 2, "expected 2 pairs in cohort of 4; got {out:?}");
    }
}
