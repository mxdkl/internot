//! `TieStrengthProfile` + `tie_strength(profile, t)`.
//!
//! Composable tie-strength function:
//!
//! ```text
//!   s(t) = clamp01(
//!     base_floor
//!     + cohabit_pulse(t, cohabit_window, cohabit_peak - base_floor)
//!     + post_cohabit_decay(t, cohabit_window.end, post_cohabit_floor, decay_tau)
//!   )
//! ```
//!
//! Where:
//! - `base_floor` is the lifetime floor (e.g. 0.4 for siblings).
//! - `cohabit_pulse` is non-zero only inside `cohabit_window`,
//!   rising to `cohabit_peak`.
//! - `post_cohabit_decay` is non-zero only after `cohabit_window.end`,
//!   following `(cohabit_peak - base_floor) * exp(-(t - end) / decay_tau)`.
//!
//! The composition is monotone-non-increasing post-cohabit-end and
//! non-negative everywhere — provable by inspection.
//!
//! Empirical τ values per tie kind (years; from spec §5.3):
//!
//! | Tie kind                  | base_floor | cohabit_peak | decay_tau_days |
//! |---------------------------|-----------:|-------------:|---------------:|
//! | Partner (cohabit→divorce) | 0.10       | 1.00         | 182  (≈ 0.5y)  |
//! | Parent ↔ child            | 0.50       | 0.90         | 1825 (≈ 5y)    |
//! | Sibling                   | 0.40       | 0.70         | 3650 (≈ 10y)   |

/// Configuration for `tie_strength`. One profile per tie kind.
/// Consumers (`internot::social`) instantiate this from their
/// kind-specific tables.
#[derive(Debug, Clone, Copy)]
pub struct TieStrengthProfile {
    /// Lifetime floor — strength outside the cohabit window and
    /// after decay completes.
    pub base_floor: f64,
    /// Strength at peak (inside cohabit window).
    pub cohabit_peak: f64,
    /// Cohabit window start in days-since-epoch (None = no window;
    /// strength always at `base_floor`).
    pub cohabit_start_day: Option<u32>,
    /// Cohabit window end. `None` means "still cohabiting" — strength
    /// stays at `cohabit_peak` from `cohabit_start_day` onward with no
    /// decay applied. Set to a concrete day to model divorce / move-out;
    /// post-end decay then engages.
    pub cohabit_end_day: Option<u32>,
    /// Time constant in days for post-cohabit exponential decay.
    pub decay_tau_days: f64,
}

impl TieStrengthProfile {
    /// Constant-strength profile — no cohabit window, no decay.
    /// Useful for "we've always been related at a base level".
    pub fn constant(strength: f64) -> Self {
        Self {
            base_floor: strength,
            cohabit_peak: strength,
            cohabit_start_day: None,
            cohabit_end_day: None,
            decay_tau_days: f64::INFINITY, // decay never applies; cohabit_*_day are None
        }
    }
}

/// Tie strength at days-since-epoch `t_days`. Always in `[0.0, 1.0]`.
pub fn tie_strength(profile: &TieStrengthProfile, t_days: u32) -> f64 {
    debug_assert!(
        profile.cohabit_peak >= profile.base_floor,
        "cohabit_peak ({}) must be >= base_floor ({})",
        profile.cohabit_peak,
        profile.base_floor,
    );
    let mut s = profile.base_floor;
    // Three regions of `t_days`: [0, start) → base_floor (implicit
    // fallthrough below); [start, end] → cohabit peak; (end, ∞) →
    // decay toward floor. When end is None the window is ongoing.
    match (profile.cohabit_start_day, profile.cohabit_end_day) {
        (Some(start), Some(end)) => {
            if (start..=end).contains(&t_days) {
                // Inside cohabit window: peak.
                s = profile.cohabit_peak.max(s);
            } else if t_days > end {
                // Post-cohabit decay back to floor.
                let elapsed_days = (t_days - end) as f64;
                let head = profile.cohabit_peak - profile.base_floor;
                let decay = head * (-elapsed_days / profile.decay_tau_days.max(0.001)).exp();
                s = profile.base_floor + decay.max(0.0);
            }
            // else t_days < start: pre-window, fall through to base_floor.
        }
        (Some(start), None) => {
            // Ongoing cohabitation — peak from `start` onward.
            if t_days >= start {
                s = profile.cohabit_peak.max(s);
            }
            // else pre-window: fall through to base_floor.
        }
        _ => {
            // No window (or end-without-start, ill-formed): base_floor.
        }
    }
    s.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent_child() -> TieStrengthProfile {
        TieStrengthProfile {
            base_floor: 0.5,
            cohabit_peak: 0.9,
            cohabit_start_day: Some(0),
            cohabit_end_day: Some(18 * 365),
            decay_tau_days: 5.0 * 365.0,
        }
    }

    #[test]
    fn strength_inside_cohabit_window_is_peak() {
        let p = parent_child();
        assert!((tie_strength(&p, 10 * 365) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn strength_at_cohabit_end_is_peak() {
        let p = parent_child();
        assert!((tie_strength(&p, 18 * 365) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn strength_decays_after_cohabit() {
        let p = parent_child();
        // 5 years (1 tau) after move-out: head = 0.4; remaining = 0.4 * e^-1 ≈ 0.147
        let s = tie_strength(&p, 18 * 365 + 5 * 365);
        assert!(s > 0.5 && s < 0.7, "got {s}");
        // 50 years after move-out: deeply into floor
        let s2 = tie_strength(&p, 18 * 365 + 50 * 365);
        assert!((s2 - 0.5).abs() < 0.01);
    }

    #[test]
    fn strength_outside_window_is_floor() {
        let p = TieStrengthProfile {
            cohabit_start_day: Some(100),
            cohabit_end_day: Some(200),
            ..parent_child()
        };
        assert!((tie_strength(&p, 50) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn strength_is_always_in_unit_interval() {
        let p = parent_child();
        for t in (0..50 * 365).step_by(30) {
            let s = tie_strength(&p, t);
            assert!((0.0..=1.0).contains(&s));
        }
    }

    #[test]
    fn constant_profile_returns_constant_strength() {
        let p = TieStrengthProfile::constant(0.6);
        for t in [0, 1_000, 100_000] {
            assert_eq!(tie_strength(&p, t), 0.6);
        }
    }

    #[test]
    fn strength_in_zero_length_window_is_peak() {
        // start == end: exactly one day is "in window".
        let p = TieStrengthProfile {
            base_floor: 0.3,
            cohabit_peak: 0.9,
            cohabit_start_day: Some(100),
            cohabit_end_day: Some(100),
            decay_tau_days: 365.0,
        };
        // Day 100 — in window: peak.
        assert!((tie_strength(&p, 100) - 0.9).abs() < 1e-9);
        // Day 99 — pre-window: floor.
        assert!((tie_strength(&p, 99) - 0.3).abs() < 1e-9);
        // Day 101 — post-window: decay 1 day after end. head = 0.6,
        // decay = 0.6 * exp(-1/365) ≈ 0.598; floor + decay ≈ 0.898.
        let s = tie_strength(&p, 101);
        assert!(s > 0.89 && s < 0.91, "expected ~0.898, got {s}");
    }

    #[test]
    fn strength_is_monotone_non_increasing_post_cohabit() {
        let p = parent_child();
        let mut prev = tie_strength(&p, 18 * 365);
        for t in (18 * 365 + 1..30 * 365).step_by(7) {
            let s = tie_strength(&p, t);
            assert!(s <= prev + 1e-12, "non-monotone at t={t}: {prev} -> {s}");
            prev = s;
        }
    }

    #[test]
    fn ongoing_cohabit_returns_peak_from_start() {
        let p = TieStrengthProfile {
            base_floor: 0.1,
            cohabit_peak: 1.0,
            cohabit_start_day: Some(100),
            cohabit_end_day: None, // currently cohabiting
            decay_tau_days: 365.0,
        };
        // Before start: base_floor.
        assert!((tie_strength(&p, 50) - 0.1).abs() < 1e-9);
        // At start: peak.
        assert!((tie_strength(&p, 100) - 1.0).abs() < 1e-9);
        // 10 years in, still cohabiting: peak.
        assert!((tie_strength(&p, 100 + 10 * 365) - 1.0).abs() < 1e-9);
    }
}
