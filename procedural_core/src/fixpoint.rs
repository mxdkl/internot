//! Least fixed points of monotone expansions: closures.
//!
//! Many inverse queries have the shape "everything that entered through a
//! static door, plus everything derived from a member". Examples are the
//! households ever in a residential basin, the descendants who inherited a
//! place, and the workers ever in a firm. The answer is the least set
//! containing the entries and closed under a successor relation.
//!
//! A worklist computes it in O(|closure| · successor cost), with each item
//! expanded once. The successor function is where a consumer verifies
//! membership (it pushes only confirmed successors). So the closure is exact
//! whenever every member is reachable from an entry through confirmed
//! successors, which the consumer proves by induction on its own order
//! (time, generation).

use std::collections::HashSet;
use std::hash::Hash;

/// Everything reachable from `entries` through `successors`, each once, in
/// breadth-first discovery order. `successors(item, out)` appends the
/// item's successors to `out` (duplicates are fine).
///
/// The output order depends only on the entries' order and on what
/// `successors` appends, never on hashing, so it is deterministic.
pub fn closure<T, I, F>(entries: I, mut successors: F) -> Vec<T>
where
    T: Eq + Hash + Clone,
    I: IntoIterator<Item = T>,
    F: FnMut(&T, &mut Vec<T>),
{
    let mut seen: HashSet<T> = HashSet::new();
    let mut out: Vec<T> = Vec::new();
    for e in entries {
        if seen.insert(e.clone()) {
            out.push(e);
        }
    }
    let mut buf: Vec<T> = Vec::new();
    let mut i = 0;
    while i < out.len() {
        buf.clear();
        let item = out[i].clone();
        successors(&item, &mut buf);
        for s in buf.drain(..) {
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
        i += 1;
    }
    out
}

/// [`closure`] by breadth-first layers: `expand(layer)` returns the
/// successors of a whole layer at once (in any order, duplicates allowed),
/// so a consumer can expand a layer in parallel. Each layer is
/// deduplicated against everything seen, in the order `expand` returned it,
/// so the result is deterministic whenever `expand` is.
pub fn closure_layers<T, I, F>(entries: I, mut expand: F) -> Vec<T>
where
    T: Eq + Hash + Clone,
    I: IntoIterator<Item = T>,
    F: FnMut(&[T]) -> Vec<T>,
{
    let mut seen: HashSet<T> = HashSet::new();
    let mut out: Vec<T> = Vec::new();
    for e in entries {
        if seen.insert(e.clone()) {
            out.push(e);
        }
    }
    let mut layer_start = 0;
    while layer_start < out.len() {
        let layer_end = out.len();
        let next = expand(&out[layer_start..layer_end]);
        for s in next {
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
        layer_start = layer_end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A random directed graph on `n` nodes with out-degree up to `d`.
    fn graph(n: u32, d: u32, mut x: u64) -> Vec<Vec<u32>> {
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        (0..n)
            .map(|_| {
                let k = (next() % (d as u64 + 1)) as u32;
                (0..k).map(|_| (next() % n as u64) as u32).collect()
            })
            .collect()
    }

    /// Reference: iterate "add every successor of every member" to a fixed point.
    fn naive(g: &[Vec<u32>], entries: &[u32]) -> Vec<bool> {
        let mut inside = vec![false; g.len()];
        for &e in entries {
            inside[e as usize] = true;
        }
        loop {
            let mut changed = false;
            for v in 0..g.len() {
                if inside[v] {
                    for &w in &g[v] {
                        if !inside[w as usize] {
                            inside[w as usize] = true;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return inside;
            }
        }
    }

    #[test]
    fn equals_the_naive_fixed_point() {
        for seed in 1..60u64 {
            let g = graph(200, 3, seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let entries: Vec<u32> = (0..5)
                .map(|i| ((seed * 31 + i * 17) % 200) as u32)
                .collect();
            let got = closure(entries.iter().copied(), |&v, out| {
                out.extend(&g[v as usize])
            });
            let want = naive(&g, &entries);
            let mut inside = vec![false; g.len()];
            for &v in &got {
                assert!(!inside[v as usize], "each item once");
                inside[v as usize] = true;
            }
            assert_eq!(inside, want, "seed {seed}");
        }
    }

    #[test]
    fn layers_give_the_same_set() {
        for seed in 1..40u64 {
            let g = graph(150, 3, seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
            let entries: Vec<u32> = (0..4)
                .map(|i| ((seed * 13 + i * 29) % 150) as u32)
                .collect();
            let a = closure(entries.iter().copied(), |&v, out| {
                out.extend(&g[v as usize])
            });
            let b = closure_layers(entries.iter().copied(), |layer| {
                layer
                    .iter()
                    .flat_map(|&v| g[v as usize].iter().copied())
                    .collect()
            });
            let (mut a2, mut b2) = (a.clone(), b.clone());
            a2.sort_unstable();
            b2.sort_unstable();
            assert_eq!(a2, b2, "seed {seed}");
            assert_eq!(a, b, "breadth-first order is the same too");
        }
    }

    #[test]
    fn order_is_breadth_first_and_deterministic() {
        // 0 → 1, 2; 1 → 3; 2 → 3, 4; 4 → 0 (a cycle back to an entry).
        let g: Vec<Vec<u32>> = vec![vec![1, 2], vec![3], vec![3, 4], vec![], vec![0]];
        let got = closure([0u32, 0], |&v, out| out.extend(&g[v as usize]));
        assert_eq!(got, vec![0, 1, 2, 3, 4]);
        let again = closure([0u32], |&v, out| out.extend(&g[v as usize]));
        assert_eq!(got, again);
    }

    #[test]
    fn verification_in_successors_prunes() {
        // Only even successors are confirmed, so odd nodes never enter.
        let g: Vec<Vec<u32>> = vec![vec![1, 2], vec![4], vec![3, 4], vec![], vec![]];
        let got = closure([0u32], |&v, out| {
            out.extend(g[v as usize].iter().filter(|&&w| w % 2 == 0))
        });
        assert_eq!(got, vec![0, 2, 4]);
        assert!(closure(std::iter::empty::<u32>(), |_, _| {}).is_empty());
    }
}
