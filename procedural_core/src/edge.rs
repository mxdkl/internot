//! Pairwise edge functions — similarity and connection probability.
//!
//! Each function takes two vectors (or other domain-specific inputs) and returns
//! a scalar in `[0, 1]` representing similarity or connection strength.

/// Geometric edge function: similarity from Euclidean distance.
///
/// If `soft == false`, returns 1.0 when distance <= radius, 0.0 otherwise (hard threshold).
/// If `soft == true`, returns a smooth sigmoid centered at `radius` — value 0.5 at exactly
/// the boundary, tending to 1.0 below and 0.0 far above.
///
/// # Panics
///
/// Panics if `a` and `b` have different lengths.
pub fn geometric(a: &[f64], b: &[f64], radius: f64, soft: bool) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors must have equal length");
    let dist_sq: f64 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
    let dist = dist_sq.sqrt();
    if soft {
        // Sigmoid: 1 / (1 + exp(k * (d - r))), with k chosen so the transition is sharp but smooth.
        let k = 6.0 / radius.max(f64::MIN_POSITIVE);
        1.0 / (1.0 + (k * (dist - radius)).exp())
    } else if dist <= radius {
        1.0
    } else {
        0.0
    }
}

/// Cosine similarity, clamped to `[0, 1]`.
///
/// Returns 0 for zero-vectors and for negative cosine (angle > 90°).
/// Values below `threshold` are clamped to 0, for coarse pre-filtering.
///
/// # Panics
///
/// Panics if `a` and `b` have different lengths.
pub fn cosine(a: &[f64], b: &[f64], threshold: f64) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors must have equal length");
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    let s = (dot / (norm_a * norm_b)).max(0.0);
    if s < threshold {
        0.0
    } else {
        s
    }
}

/// Hyperbolic edge function (Papadopoulos popularity × similarity model).
///
/// Each point is `(r, theta)` in a Poincaré-disk parameterization where `r ∈ [0, 1)`.
/// The radial coordinate is first converted to a true hyperbolic radius via
/// `r_hyp = 2 * arctanh(r)`, then the hyperbolic distance between the two points is
/// computed via the hyperbolic law of cosines. Connection probability is a sigmoid of
/// that distance centered at `r_disk` with sharpness controlled by `temperature`
/// (small temperature → sharp threshold).
pub fn hyperbolic(a: (f64, f64), b: (f64, f64), r_disk: f64, temperature: f64) -> f64 {
    let (r1, theta1) = a;
    let (r2, theta2) = b;
    // Convert Poincaré-disk radial coordinates to hyperbolic radii.
    let h1 = 2.0 * r1.clamp(0.0, 1.0 - f64::EPSILON).atanh();
    let h2 = 2.0 * r2.clamp(0.0, 1.0 - f64::EPSILON).atanh();
    // Hyperbolic law of cosines. Guard against numerical issues when the two points coincide.
    let d_theta = (theta1 - theta2).cos();
    let cosh_d = h1.cosh() * h2.cosh() - h1.sinh() * h2.sinh() * d_theta;
    let cosh_d = cosh_d.max(1.0); // ensure input to acosh is valid
    let d = cosh_d.acosh();
    // Sigmoid connection probability, sharper for smaller temperature.
    let t = temperature.max(1e-6);
    1.0 / (1.0 + ((d - r_disk) / (2.0 * t)).exp())
}

/// Stochastic block model edge probability.
///
/// Returns `p_in` if the two group IDs match, `p_out` otherwise. A trivial but
/// foundational model for community structure: entities within the same community
/// are more likely to connect than those across communities.
pub fn block(group_a: u64, group_b: u64, p_in: f64, p_out: f64) -> f64 {
    if group_a == group_b {
        p_in
    } else {
        p_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometric_identical_vectors_is_max() {
        let v = [1.0, 2.0, 3.0];
        assert!((geometric(&v, &v, 0.5, false) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn geometric_far_vectors_is_zero_hard() {
        let a = [0.0, 0.0];
        let b = [10.0, 10.0];
        assert_eq!(geometric(&a, &b, 1.0, false), 0.0);
    }

    #[test]
    fn geometric_boundary_soft_is_half() {
        // Soft threshold: at exactly radius, similarity is 0.5
        let a = [0.0, 0.0];
        let b = [0.6, 0.8]; // distance = 1.0
        assert!((geometric(&a, &b, 1.0, true) - 0.5).abs() < 1e-2);
    }

    #[test]
    #[should_panic(expected = "vectors must have equal length")]
    fn geometric_panics_on_length_mismatch() {
        geometric(&[1.0, 2.0], &[1.0], 0.5, false);
    }

    #[test]
    fn cosine_identical_vectors_is_max() {
        let v = [1.0, 2.0, 3.0];
        assert!((cosine(&v, &v, 0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_orthogonal_vectors_is_zero() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        let s = cosine(&a, &b, 0.0);
        assert!(s.abs() < 1e-9, "orthogonal cosine = {}", s);
    }

    #[test]
    fn cosine_opposite_vectors_is_zero() {
        // We clamp negative similarity to 0.0.
        let a = [1.0, 0.0];
        let b = [-1.0, 0.0];
        assert_eq!(cosine(&a, &b, 0.0), 0.0);
    }

    #[test]
    fn cosine_zero_vector_is_zero() {
        let a = [0.0, 0.0];
        let b = [1.0, 1.0];
        assert_eq!(cosine(&a, &b, 0.0), 0.0);
    }

    #[test]
    fn cosine_threshold_filters() {
        // Partial similarity below threshold returns 0
        let a = [1.0, 0.0];
        let b = [1.0, 1.0]; // cos theta = 1/sqrt(2) ≈ 0.707
        assert_eq!(cosine(&a, &b, 0.8), 0.0);
        assert!(cosine(&a, &b, 0.5) > 0.0);
    }

    #[test]
    fn hyperbolic_close_points_high_similarity() {
        // Two points at (r=0.1, theta=0.0) and (r=0.1, theta=0.01) — very close.
        let s = hyperbolic((0.1, 0.0), (0.1, 0.01), 5.0, 0.1);
        assert!(s > 0.9, "expected high similarity, got {}", s);
    }

    #[test]
    fn hyperbolic_far_points_low_similarity() {
        // Two points at the edge of the disk, on opposite sides.
        let s = hyperbolic((0.99, 0.0), (0.99, std::f64::consts::PI), 5.0, 0.1);
        assert!(s < 0.1, "expected low similarity, got {}", s);
    }

    #[test]
    fn hyperbolic_is_symmetric() {
        let a = (0.5, 1.0);
        let b = (0.7, 2.0);
        let s_ab = hyperbolic(a, b, 5.0, 0.3);
        let s_ba = hyperbolic(b, a, 5.0, 0.3);
        assert!((s_ab - s_ba).abs() < 1e-9);
    }

    #[test]
    fn hyperbolic_output_in_unit_interval() {
        for r1 in [0.0, 0.3, 0.7, 0.99] {
            for theta1 in [0.0, 1.0, 3.0] {
                for r2 in [0.0, 0.3, 0.7, 0.99] {
                    for theta2 in [0.0, 1.0, 3.0] {
                        let s = hyperbolic((r1, theta1), (r2, theta2), 5.0, 0.3);
                        assert!(
                            (0.0..=1.0).contains(&s),
                            "out of range: s({}, {}) vs ({}, {}) = {}",
                            r1,
                            theta1,
                            r2,
                            theta2,
                            s
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn block_same_group_uses_p_in() {
        let p = block(5, 5, 0.8, 0.1);
        assert_eq!(p, 0.8);
    }

    #[test]
    fn block_different_groups_uses_p_out() {
        let p = block(5, 7, 0.8, 0.1);
        assert_eq!(p, 0.1);
    }

    #[test]
    fn block_output_in_unit_interval() {
        let p = block(5, 7, 0.3, 0.05);
        assert!((0.0..=1.0).contains(&p));
    }
}
