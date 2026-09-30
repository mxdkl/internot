//! Budget-bounded top-k similarity search: `Space::candidates(id, context)`.
//!
//! A different query API from `find()` — same Space, but cost contract
//! differs: `candidates()` is approximate, budget-bounded, and returns
//! scored top-k hits rather than a lazy iterator over matches.

use crate::bits::BitLayout;
use crate::hash::hash_with_index;
use crate::search::{BitPattern, FindIter};
use crate::space::attribute::{AttributeSlot, EvalFn};
use crate::space::context::{ContextDim, ContextSlot, Metric, Normalization};
use crate::space::Space;
use crate::word::BitWord;
use chrono::{DateTime, Utc};
use ordered_float::OrderedFloat;
use std::any::{Any, TypeId};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// A context dim resolved once at query start — holds direct references
/// to the attribute slot + a pre-selected coerce-to-f64 function pointer
/// so the hot candidate loop never re-looks-up names or walks the
/// `downcast_ref` chain.
///
/// See `docs/profiling/2026-04-23-query-hotspots.md` for the rationale:
/// `build_vector` previously did two `HashMap<String>` lookups and one
/// 10-branch `downcast_ref` chain per dim per candidate.
pub(crate) struct ResolvedDim<'a, W: BitWord> {
    pub(crate) slot: &'a AttributeSlot<W>,
    pub(crate) coerce: fn(&dyn Any) -> f64,
    pub(crate) weight: f64,
    /// For panic messages only — not read on the hot path.
    pub(crate) attr_name: &'a str,
}

/// A context resolved once at query start. Metric + per-dim resolution,
/// borrowed from the underlying `Space`.
pub(crate) struct ResolvedContext<'a, W: BitWord> {
    pub(crate) metric: Metric,
    pub(crate) dims: Vec<ResolvedDim<'a, W>>,
    /// Back-reference to the space so composite attributes can read their
    /// dependency fields' bits during evaluation.
    pub(crate) space: &'a Space<W>,
}

/// Resolve a context by name into `ResolvedContext` — all name lookups,
/// normalization validation, and coerce-fn selection happen here.
///
/// Panics on unknown context, unknown attribute in any dim, unsupported
/// normalization, unsupported attribute type, or Hyperbolic metric at
/// call sites that consume the result.
pub(crate) fn resolve_context<'a, W: BitWord>(
    space: &'a Space<W>,
    context_name: &str,
) -> ResolvedContext<'a, W> {
    let slot: &ContextSlot = space
        .context_slot(context_name)
        .unwrap_or_else(|| panic!("unknown context: {context_name:?}"));

    let dims: Vec<ResolvedDim<'a, W>> = slot
        .dimensions
        .iter()
        .map(|dim: &ContextDim| {
            let attr_slot = space.attribute_slot(&dim.attribute).unwrap_or_else(|| {
                panic!(
                    "context {:?} references unknown attribute {:?}",
                    context_name, dim.attribute
                )
            });
            // v0.1 only supports Normalization::None — check once, not per candidate.
            if !matches!(dim.normalize, Normalization::None) {
                panic!(
                    "Normalization::{:?} not implemented in v0.1 — use Normalization::None",
                    dim.normalize
                );
            }
            let coerce = coerce_fn_for(
                attr_slot.value_type_id,
                attr_slot.value_type_name,
                &dim.attribute,
            );
            ResolvedDim {
                slot: attr_slot,
                coerce,
                weight: dim.weight,
                attr_name: dim.attribute.as_str(),
            }
        })
        .collect();

    ResolvedContext {
        metric: slot.metric,
        dims,
        space,
    }
}

/// Pick the coerce-to-f64 fn pointer for a registered attribute type.
///
/// Called once per `ResolvedDim` at query setup — O(type count) TypeId
/// compares, then the returned `fn` is invoked per candidate with a single
/// `downcast_ref` for the known-correct type. Replaces the
/// 10-branch `if let Some(v) = value.downcast_ref::<T>()` chain in the hot
/// loop.
fn coerce_fn_for(type_id: TypeId, type_name: &'static str, attr_name: &str) -> fn(&dyn Any) -> f64 {
    if type_id == TypeId::of::<f64>() {
        |v| *v.downcast_ref::<f64>().unwrap()
    } else if type_id == TypeId::of::<f32>() {
        |v| *v.downcast_ref::<f32>().unwrap() as f64
    } else if type_id == TypeId::of::<u64>() {
        |v| *v.downcast_ref::<u64>().unwrap() as f64
    } else if type_id == TypeId::of::<u32>() {
        |v| *v.downcast_ref::<u32>().unwrap() as f64
    } else if type_id == TypeId::of::<u16>() {
        |v| *v.downcast_ref::<u16>().unwrap() as f64
    } else if type_id == TypeId::of::<u8>() {
        |v| *v.downcast_ref::<u8>().unwrap() as f64
    } else if type_id == TypeId::of::<i64>() {
        |v| *v.downcast_ref::<i64>().unwrap() as f64
    } else if type_id == TypeId::of::<i32>() {
        |v| *v.downcast_ref::<i32>().unwrap() as f64
    } else if type_id == TypeId::of::<i16>() {
        |v| *v.downcast_ref::<i16>().unwrap() as f64
    } else if type_id == TypeId::of::<i8>() {
        |v| *v.downcast_ref::<i8>().unwrap() as f64
    } else if type_id == TypeId::of::<bool>() {
        |v| {
            if *v.downcast_ref::<bool>().unwrap() {
                1.0
            } else {
                0.0
            }
        }
    } else {
        panic!(
            "context dim {:?} is not a numeric primitive — attribute type {} not supported (only f64/f32/u8..u64/i8..i64/bool)",
            attr_name, type_name
        );
    }
}

/// Hot-path feature-vector builder. Writes into a reusable `out` buffer
/// so candidate loops don't allocate per iteration.
pub(crate) fn build_vector_into<W: BitWord>(
    resolved: &ResolvedContext<'_, W>,
    id: W,
    t: Option<DateTime<Utc>>,
    out: &mut Vec<f64>,
) {
    out.clear();
    out.reserve(resolved.dims.len());
    for dim in &resolved.dims {
        let raw = eval_attribute(resolved.space, dim.slot, id, t, dim.attr_name);
        let v = (dim.coerce)(&*raw);
        out.push(v * dim.weight);
    }
}

/// Default budget if the caller doesn't call `.budget(n)`.
pub(crate) const DEFAULT_BUDGET: usize = 1024;

/// Builder for a similarity query.
///
/// Construct via `Space::candidates(id, context)`. Consumed by `take(k)`.
pub struct CandidateQuery<'a, W: BitWord> {
    pub(crate) space: &'a Space<W>,
    pub(crate) id: W,
    pub(crate) context: &'a str,
    pub(crate) budget: usize,
    pub(crate) envelope_field: Option<String>,
    pub(crate) time_anchor: Option<DateTime<Utc>>,
    pub(crate) min_score: f64,
}

impl<'a, W: BitWord> CandidateQuery<'a, W> {
    /// Cap on candidates drawn from the underlying stream. Default 1024.
    pub fn budget(mut self, n: usize) -> Self {
        self.budget = n;
        self
    }

    /// Narrow the candidate stream to IDs sharing the query's value at
    /// `field`. Panics at `take()` if `field` is unknown.
    pub fn envelope(mut self, field: &str) -> Self {
        self.envelope_field = Some(field.to_string());
        self
    }

    /// Time anchor for temporal context dimensions. Required if any
    /// `ContextDim`'s attribute is temporal.
    pub fn at(mut self, t: DateTime<Utc>) -> Self {
        self.time_anchor = Some(t);
        self
    }

    /// Minimum score threshold for inclusion. Default `f64::NEG_INFINITY`.
    pub fn min_score(mut self, s: f64) -> Self {
        self.min_score = s;
        self
    }

    /// Consume the query; score up to `budget` candidates and return the
    /// top `k` best-first.
    pub fn take(self, k: usize) -> CandidateResults<W> {
        // Resolve context + dims once — every lookup, normalization check,
        // and coerce-fn selection happens here, not per candidate.
        let resolved = resolve_context(self.space, self.context);
        let metric = resolved.metric;
        let n_dims = resolved.dims.len();

        // Scratch buffers — reused across all candidates. `build_vector_into`
        // clears + refills into `scratch` each iteration, avoiding the
        // per-candidate Vec allocation that dominated the prior hot loop.
        let mut query_vec: Vec<f64> = Vec::with_capacity(n_dims);
        build_vector_into(&resolved, self.id, self.time_anchor, &mut query_vec);
        let mut scratch: Vec<f64> = Vec::with_capacity(n_dims);

        // Min-heap of (score, id) — pop the smallest to evict when size > k.
        let mut heap: BinaryHeap<Reverse<(OrderedFloat<f64>, W)>> =
            BinaryHeap::with_capacity(k + 1);
        let mut evaluated: usize = 0;
        let mut drawn: usize = 0;

        // Two stream shapes — boxed Iterator<Item = W> unifies the type.
        // envelope_stream borrows space; collect into Vec so the Box can own it
        // independently of self.space's borrow, enabling us to iterate.
        let stream: Box<dyn Iterator<Item = W>> = if let Some(ref field) = self.envelope_field {
            let ids: Vec<W> = envelope_stream(self.space, self.id, field)
                .take(self.budget)
                .collect();
            Box::new(ids.into_iter())
        } else {
            Box::new(hash_stream(self.id))
        };

        for cand in stream.take(self.budget) {
            drawn += 1;
            if cand == self.id {
                continue;
            }
            evaluated += 1;
            build_vector_into(&resolved, cand, self.time_anchor, &mut scratch);
            let s = score(metric, &query_vec, &scratch);
            // Drop NaN (exotic — can leak in from a user f64 attribute) and
            // scores below min_score. NaN is not a valid similarity score and
            // would create ordering inconsistency between the heap (total
            // order via OrderedFloat) and the final sort.
            if s.is_nan() || s < self.min_score {
                continue;
            }
            heap.push(Reverse((OrderedFloat(s), cand)));
            if heap.len() > k {
                heap.pop();
            }
        }

        let mut hits: Vec<(W, f64)> = heap
            .into_iter()
            .map(|Reverse((OrderedFloat(s), id))| (id, s))
            .collect();
        // Sort best-first (highest score first); ties break by id ascending.
        // Use OrderedFloat so the final sort's total order matches the heap's —
        // NaN was filtered above, but OrderedFloat is defensive if that changes.
        hits.sort_by(|a, b| {
            OrderedFloat(b.1)
                .cmp(&OrderedFloat(a.1))
                .then(a.0.cmp(&b.0))
        });

        CandidateResults {
            hits,
            evaluated,
            budget: self.budget,
            total_considered: drawn,
        }
    }
}

/// Result of a `candidates()` query. Approximate — see `total_considered`.
pub struct CandidateResults<W: BitWord> {
    /// Top-k hits sorted best-first (highest score first).
    pub hits: Vec<(W, f64)>,
    /// Candidates actually scored (passed the query_id skip).
    pub evaluated: usize,
    /// Budget cap used for this query.
    pub budget: usize,
    /// Candidates drawn from the underlying stream. Equal to `evaluated`
    /// in v0.1; distinct field is reserved for future envelope-level
    /// pre-filtering.
    pub total_considered: usize,
}

/// Auto-cast supported numeric primitives to f64.
///
/// Supported: f64, f32, u8/u16/u32/u64, i8/i16/i32/i64, bool (false=0, true=1).
/// Anything else panics with a clear message.
///
/// Convenience wrapper — production call sites use `coerce_fn_for` once at
/// query start and invoke the returned fn pointer per candidate, avoiding
/// the linear TypeId walk here. Kept public-in-crate because tests and
/// one-off diagnostics use it directly.
#[allow(dead_code)] // used by tests; kept callable for ad-hoc diagnostics
pub(crate) fn coerce_to_f64(value: &dyn Any, attr_name: &str) -> f64 {
    // Invoke via the same TypeId dispatch as the hot path so both paths
    // stay behaviorally identical and a single bug can't hide in one.
    // Production callers use `resolve_context` which passes the correct
    // static type name; this wrapper has only the dyn Any at hand, so
    // it reports a generic label in the unsupported-type panic.
    let type_id = value.type_id();
    let coerce = coerce_fn_for(type_id, "<dyn Any>", attr_name);
    coerce(value)
}

/// Evaluate an attribute at `id` (and optionally `t`), returning its boxed value.
///
/// Panics if the attribute is temporal and `t` is `None`, or if the
/// attribute requires cross-space context (not supported by v0.1
/// candidates()).
fn eval_attribute<W: BitWord>(
    space: &Space<W>,
    slot: &AttributeSlot<W>,
    id: W,
    t: Option<DateTime<Utc>>,
    attr_name: &str,
) -> Box<dyn Any + Send + Sync> {
    match &slot.eval {
        EvalFn::Static(f) => f(id),
        EvalFn::Temporal(f) => {
            let t = t.unwrap_or_else(|| {
                panic!(
                    "context contains temporal dim {:?} — call .at(t) before .take()",
                    attr_name
                )
            });
            f(id, t)
        }
        EvalFn::Composite(f) => {
            let layout = space.layout();
            let own_bits = match slot.indexable_field.as_ref() {
                Some(fname) => {
                    let (o, w) = layout
                        .field_offset_width(fname)
                        .expect("composite own_field must exist");
                    id.extract_bits(o, w)
                }
                None => 0,
            };
            let shared: Vec<u64> = slot
                .composite_dep_fields
                .iter()
                .map(|fname| {
                    let (o, w) = layout
                        .field_offset_width(fname)
                        .expect("composite shared field must exist");
                    id.extract_bits(o, w)
                })
                .collect();
            f(own_bits, &shared)
        }
        EvalFn::CrossSpaceStatic(_) | EvalFn::CrossSpaceTemporal(_) => {
            panic!(
                "context dim {:?} is cross-space — candidates() v0.1 does not support cross-space dims",
                attr_name
            );
        }
    }
}

/// Build the f64 feature vector for `id` under the named context.
///
/// Convenience one-shot: resolves the context and writes into a fresh
/// `Vec`. Internal hot loops call `resolve_context` + `build_vector_into`
/// directly so name resolution and allocation don't happen per candidate.
#[allow(dead_code)] // used by tests; kept callable for ad-hoc diagnostics
pub(crate) fn build_vector<W: BitWord>(
    space: &Space<W>,
    id: W,
    context_name: &str,
    t: Option<DateTime<Utc>>,
) -> Vec<f64> {
    let resolved = resolve_context(space, context_name);
    let mut out = Vec::with_capacity(resolved.dims.len());
    build_vector_into(&resolved, id, t, &mut out);
    out
}

// Silence unused ContextDim import if clippy complains (only used via
// slot.dimensions iteration — which clippy can miss in some configs).
#[allow(dead_code)]
const _: Option<ContextDim> = None;

/// Cosine similarity of two equal-length vectors. Returns 0.0 if either
/// vector has zero magnitude (undefined similarity convention).
pub(crate) fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let na = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot / (na * nb)
}

/// Euclidean distance between two equal-length vectors.
pub(crate) fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

/// Score `a` against `b` under the given metric.
///
/// Higher return values = more similar. For Euclidean/Weighted this is the
/// negated distance (so identical vectors score 0.0, farther ones score
/// more negative). For Cosine this is the cosine similarity in [-1, 1].
pub(crate) fn score(metric: Metric, a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(
        a.len(),
        b.len(),
        "vectors have different dimensionality: {} vs {}",
        a.len(),
        b.len()
    );
    match metric {
        Metric::Cosine => cosine_similarity(a, b),
        Metric::Euclidean | Metric::Weighted => -euclidean_distance(a, b),
        Metric::Hyperbolic => {
            panic!("Hyperbolic not implemented in v0.1 — use Cosine/Euclidean/Weighted")
        }
    }
}

/// Deterministic hash-sampled candidate stream.
///
/// Yields `W` values by hashing `(query_id, "candidate", i)` for i = 0, 1, 2…
/// Uniform over the full `W` space. Deterministic: same `query_id` always
/// produces the same sequence. Callers must skip `query_id` itself in the
/// consumer loop.
///
/// Uses `hash_with_index` to avoid the `format!("candidate_{i}")` heap
/// allocation that dominated this path's per-candidate cost — see
/// `docs/benchmarks.md` v0.14.0 notes on `scale_candidates`. The hash
/// output is **not** bit-compatible with the previous `raw_hash(id,
/// "candidate_<i>")` output, so candidate IDs shift across this release.
fn hash_stream<W: BitWord>(query_id: W) -> impl Iterator<Item = W> {
    (0u64..).map(move |i| {
        let h = hash_with_index(query_id, "candidate", i);
        W::from_hash_u64(h)
    })
}

/// Envelope-narrowed candidate stream: yields IDs sharing the query's
/// value at `field`. Uses `FindIter` under the hood.
///
/// Panics on unknown field.
fn envelope_stream<'a, W: BitWord>(
    space: &'a Space<W>,
    query_id: W,
    field: &str,
) -> impl Iterator<Item = W> + 'a {
    let layout: &BitLayout<W> = space.layout();
    let (_offset, width) = layout
        .field_offset_width(field)
        .unwrap_or_else(|| panic!("unknown field: {field:?}"));
    let value = layout.extract(query_id, field);
    let pattern = BitPattern::exact(value, width as u8);
    let mut per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
    per_field.insert(field.to_string(), vec![pattern]);
    FindIter::new(layout, per_field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitLayout;

    fn tiny_space() -> Space<u64> {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        Space::<u64>::new("tiny", layout)
    }

    #[test]
    fn default_budget_is_1024() {
        let space = tiny_space();
        let q = space.candidates(1u64, "nonexistent");
        assert_eq!(q.budget, DEFAULT_BUDGET);
    }

    #[test]
    fn budget_setter_overrides_default() {
        let space = tiny_space();
        let q = space.candidates(1u64, "nonexistent").budget(42);
        assert_eq!(q.budget, 42);
    }

    #[test]
    fn envelope_setter_records_field() {
        let space = tiny_space();
        let q = space.candidates(1u64, "nonexistent").envelope("a");
        assert_eq!(q.envelope_field.as_deref(), Some("a"));
    }

    #[test]
    fn at_setter_records_anchor() {
        use chrono::TimeZone;
        let space = tiny_space();
        let t = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let q = space.candidates(1u64, "nonexistent").at(t);
        assert_eq!(q.time_anchor, Some(t));
    }

    #[test]
    fn min_score_setter_overrides_default() {
        let space = tiny_space();
        let q = space.candidates(1u64, "nonexistent").min_score(0.5);
        assert_eq!(q.min_score, 0.5);
    }

    #[test]
    fn default_min_score_is_neg_infinity() {
        let space = tiny_space();
        let q = space.candidates(1u64, "nonexistent");
        assert_eq!(q.min_score, f64::NEG_INFINITY);
    }

    use crate::space::context::{ContextDim, Metric, Normalization};

    #[test]
    fn coerce_f64_passes_through() {
        assert_eq!(coerce_to_f64(&1.5f64 as &dyn Any, "x"), 1.5);
    }

    #[test]
    fn coerce_f32_widens() {
        assert_eq!(coerce_to_f64(&1.5f32 as &dyn Any, "x"), 1.5);
    }

    #[test]
    fn coerce_u64_casts_to_expected_double() {
        assert_eq!(coerce_to_f64(&42u64 as &dyn Any, "x"), 42.0);
    }

    #[test]
    fn coerce_i32_negative_is_preserved() {
        assert_eq!(coerce_to_f64(&-7i32 as &dyn Any, "x"), -7.0);
    }

    #[test]
    fn coerce_bool_true_is_1_false_is_0() {
        assert_eq!(coerce_to_f64(&true as &dyn Any, "x"), 1.0);
        assert_eq!(coerce_to_f64(&false as &dyn Any, "x"), 0.0);
    }

    #[test]
    #[should_panic(expected = "not a numeric primitive")]
    fn coerce_panics_on_string() {
        let s = String::from("hello");
        let _ = coerce_to_f64(&s as &dyn Any, "x");
    }

    fn make_space_with_context() -> Space<u64> {
        let layout = crate::bits::BitLayout::<u64>::new(vec![("a", 8), ("b", 8)]).unwrap();
        let mut space = Space::<u64>::new("tiny", layout);
        space.indexable_attribute("a", "a", |bits| bits).unwrap();
        space.indexable_attribute("b", "b", |bits| bits).unwrap();
        space
            .context(
                "simple",
                vec![
                    ContextDim {
                        attribute: "a".to_string(),
                        weight: 2.0,
                        normalize: Normalization::None,
                    },
                    ContextDim {
                        attribute: "b".to_string(),
                        weight: 1.0,
                        normalize: Normalization::None,
                    },
                ],
                Metric::Euclidean,
            )
            .unwrap();
        space
    }

    #[test]
    fn build_vector_applies_weights() {
        let space = make_space_with_context();
        // id has a = 3, b = 5. Dims: ("a", weight=2), ("b", weight=1).
        // Expected vector: [3*2, 5*1] = [6, 5].
        // Encoding: a is bits 0..8, b is bits 8..16. id = 3 | (5 << 8) = 0x503.
        let id = 0x503u64;
        let v = build_vector(&space, id, "simple", None);
        assert_eq!(v, vec![6.0, 5.0]);
    }

    #[test]
    #[should_panic(expected = "unknown context")]
    fn build_vector_panics_on_unknown_context() {
        let space = make_space_with_context();
        let _ = build_vector(&space, 0u64, "nonexistent", None);
    }

    #[test]
    #[should_panic(expected = "references unknown attribute")]
    fn build_vector_panics_on_unknown_attribute_in_context() {
        use crate::space::context::{ContextSlot, Metric};
        let layout = crate::bits::BitLayout::<u64>::new(vec![("a", 8)]).unwrap();
        let mut space = Space::<u64>::new("tiny", layout);
        // Bypass normal validation to inject a context with a dangling attribute ref.
        space.insert_context_unchecked(
            "broken",
            ContextSlot {
                dimensions: vec![ContextDim {
                    attribute: "nope".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                metric: Metric::Euclidean,
            },
        );
        let _ = build_vector(&space, 0u64, "broken", None);
    }

    #[test]
    #[should_panic(expected = "call .at(t) before .take()")]
    fn build_vector_panics_on_temporal_dim_without_at() {
        let layout = crate::bits::BitLayout::<u64>::new(vec![("a", 8)]).unwrap();
        let mut space = Space::<u64>::new("tiny", layout);
        space
            .temporal_attribute("time_of_day", |_id: u64, t: DateTime<Utc>| {
                t.timestamp() as f64
            })
            .unwrap();
        space
            .context(
                "timed",
                vec![ContextDim {
                    attribute: "time_of_day".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                Metric::Euclidean,
            )
            .unwrap();
        let _ = build_vector(&space, 0u64, "timed", None);
    }

    #[test]
    fn build_vector_uses_anchor_for_temporal_dim() {
        use chrono::TimeZone;
        let layout = crate::bits::BitLayout::<u64>::new(vec![("a", 8)]).unwrap();
        let mut space = Space::<u64>::new("tiny", layout);
        space
            .temporal_attribute("t_sec", |_id: u64, t: DateTime<Utc>| t.timestamp() as f64)
            .unwrap();
        space
            .context(
                "timed",
                vec![ContextDim {
                    attribute: "t_sec".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                Metric::Euclidean,
            )
            .unwrap();
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let expected = t.timestamp() as f64;
        let v = build_vector(&space, 0u64, "timed", Some(t));
        assert_eq!(v, vec![expected]);
    }

    #[test]
    #[should_panic(expected = "Normalization::ZScore not implemented")]
    fn build_vector_panics_on_non_none_normalization_via_unchecked_path() {
        // `Space::context()` now refuses to register a non-`None`
        // normalization, so the panic in `resolve_context` is only
        // reachable via the test-only `insert_context_unchecked` bypass.
        // Keep this test as defense-in-depth for the panic path.
        use crate::space::context::ContextSlot;
        let layout = crate::bits::BitLayout::<u64>::new(vec![("a", 8)]).unwrap();
        let mut space = Space::<u64>::new("tiny", layout);
        space.indexable_attribute("a", "a", |bits| bits).unwrap();
        space.insert_context_unchecked(
            "normalized",
            ContextSlot {
                dimensions: vec![ContextDim {
                    attribute: "a".to_string(),
                    weight: 1.0,
                    normalize: Normalization::ZScore,
                }],
                metric: Metric::Euclidean,
            },
        );
        let _ = build_vector(&space, 0u64, "normalized", None);
    }

    #[test]
    fn cosine_similarity_equals_1_for_identical_vectors() {
        let a = [1.0, 2.0, 3.0];
        let s = cosine_similarity(&a, &a);
        assert!((s - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cosine_similarity_equals_minus_1_for_opposite_vectors() {
        let a = [1.0, 2.0, 3.0];
        let b = [-1.0, -2.0, -3.0];
        let s = cosine_similarity(&a, &b);
        assert!((s + 1.0).abs() < 1e-12);
    }

    #[test]
    fn cosine_similarity_equals_0_for_orthogonal_vectors() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_returns_0_for_zero_vector() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
        assert_eq!(cosine_similarity(&b, &a), 0.0);
    }

    #[test]
    fn euclidean_distance_matches_hand_computed() {
        // [0,0] to [3,4] → distance 5.
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        assert_eq!(euclidean_distance(&a, &b), 5.0);
    }

    #[test]
    fn score_euclidean_returns_negated_distance() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        assert_eq!(score(Metric::Euclidean, &a, &b), -5.0);
    }

    #[test]
    fn score_weighted_dispatches_to_euclidean() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        // Weighted should match Euclidean — weights are already applied
        // upstream in build_vector.
        assert_eq!(
            score(Metric::Weighted, &a, &b),
            score(Metric::Euclidean, &a, &b)
        );
    }

    #[test]
    fn score_cosine_of_identical_is_1() {
        let a = [1.0, 2.0, 3.0];
        assert!((score(Metric::Cosine, &a, &a) - 1.0).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "Hyperbolic not implemented in v0.1")]
    fn score_hyperbolic_panics_in_v0_1() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        let _ = score(Metric::Hyperbolic, &a, &b);
    }

    #[test]
    #[should_panic(expected = "vectors have different dimensionality")]
    fn score_panics_on_mismatched_dims() {
        let a = [1.0, 2.0];
        let b = [1.0, 2.0, 3.0];
        let _ = score(Metric::Euclidean, &a, &b);
    }

    #[test]
    fn hash_stream_is_deterministic_for_same_query_id() {
        let s1: Vec<u64> = hash_stream(42u64).take(10).collect();
        let s2: Vec<u64> = hash_stream(42u64).take(10).collect();
        assert_eq!(s1, s2);
    }

    #[test]
    fn hash_stream_varies_across_query_ids() {
        let s1: Vec<u64> = hash_stream(1u64).take(5).collect();
        let s2: Vec<u64> = hash_stream(2u64).take(5).collect();
        assert_ne!(s1, s2);
    }

    #[test]
    fn envelope_stream_yields_only_ids_with_matching_field_value() {
        let space = make_space_with_context();
        // a is 8 bits, b is 8 bits. Pin query_id = 0x0503 → a=3, b=5.
        // envelope("a") should yield all IDs with a=3, b anything.
        let query_id = 0x0503u64;
        let ids: Vec<u64> = envelope_stream(&space, query_id, "a").take(100).collect();
        // 256 possible values of b, but we cap at 100.
        assert!(ids.len() <= 100);
        for id in &ids {
            assert_eq!(space.layout().extract(*id, "a"), 3);
        }
    }

    #[test]
    #[should_panic(expected = "unknown field")]
    fn envelope_stream_panics_on_unknown_field() {
        let space = make_space_with_context();
        let _: Vec<u64> = envelope_stream(&space, 0u64, "nope").take(1).collect();
    }

    #[test]
    fn take_returns_at_most_k_hits() {
        let space = make_space_with_context();
        let results = space.candidates(0u64, "simple").budget(50).take(5);
        assert!(results.hits.len() <= 5);
    }

    #[test]
    fn take_hits_are_sorted_best_first() {
        let space = make_space_with_context();
        let results = space.candidates(0u64, "simple").budget(50).take(10);
        for w in results.hits.windows(2) {
            assert!(w[0].1 >= w[1].1, "not sorted: {} before {}", w[0].1, w[1].1);
        }
    }

    #[test]
    fn take_is_deterministic_for_same_query_and_budget() {
        let space = make_space_with_context();
        let r1 = space.candidates(0u64, "simple").budget(30).take(5);
        let r2 = space.candidates(0u64, "simple").budget(30).take(5);
        assert_eq!(r1.hits, r2.hits);
        assert_eq!(r1.evaluated, r2.evaluated);
    }

    #[test]
    fn take_respects_min_score() {
        let space = make_space_with_context();
        // Euclidean scores are ≤ 0. Set min_score = -0.5 → keep only
        // hits within 0.5 Euclidean distance → vanishingly few (likely 0).
        let results = space
            .candidates(0u64, "simple")
            .budget(50)
            .min_score(-0.5)
            .take(10);
        for (_, s) in &results.hits {
            assert!(*s >= -0.5);
        }
    }

    #[test]
    fn take_skips_query_id() {
        let space = make_space_with_context();
        let query_id = 0x0503u64;
        let results = space.candidates(query_id, "simple").budget(500).take(20);
        for (id, _) in &results.hits {
            assert_ne!(*id, query_id, "query_id leaked into hits");
        }
    }

    #[test]
    fn take_envelope_narrows_to_matching_field() {
        let space = make_space_with_context();
        let query_id = 0x0503u64; // a=3, b=5
        let results = space
            .candidates(query_id, "simple")
            .envelope("a")
            .budget(300)
            .take(20);
        for (id, _) in &results.hits {
            assert_eq!(space.layout().extract(*id, "a"), 3);
        }
    }

    #[test]
    fn take_evaluated_matches_candidate_count_minus_skipped_query_id() {
        let space = make_space_with_context();
        let query_id = 0u64;
        let results = space.candidates(query_id, "simple").budget(10).take(5);
        // hash_stream produces 10 candidates; query_id skipped 0 or 1 times
        // depending on whether it appears in the first 10 hash samples.
        // evaluated = drawn - (skipped query_id count).
        assert_eq!(results.budget, 10);
        assert!(results.evaluated <= 10);
        assert_eq!(results.total_considered, 10);
    }
}
