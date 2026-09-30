//! Context (vector-projection) declaration types. Similarity queries implemented in Plan 4.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Cosine similarity, range [-1, 1]. Zero-vector pairs score 0.
    Cosine,
    /// Negated Euclidean distance, range (-∞, 0]. Identical vectors score 0.
    Euclidean,
    /// Reserved for hyperbolic-plane similarity. Panics at
    /// `candidates().take()` in v0.1 — use Cosine/Euclidean/Weighted.
    Hyperbolic,
    /// Alias for `Euclidean` in v0.1 — per-dimension weights are applied
    /// upstream in `build_vector` via `ContextDim::weight`, so Weighted
    /// and Euclidean share the same scoring code path.
    Weighted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalization {
    None,
    ZScore,
    MinMax,
    Decade,
}

#[derive(Debug, Clone)]
pub struct ContextDim {
    pub attribute: String,
    pub weight: f64,
    pub normalize: Normalization,
}

pub(crate) struct ContextSlot {
    pub(crate) dimensions: Vec<ContextDim>,
    pub(crate) metric: Metric,
}
