//! Comparison operator for threshold-based temporal range predicates.

/// Comparison operator between a value function `f(id, t)` and a threshold.
///
/// Note: `Eq` / `Ne` on `f64` use bit-exact comparison. For approximate
/// crossings, use `Lt` / `Le` / `Gt` / `Ge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl Op {
    /// Evaluate `value OP threshold`.
    pub fn check(self, value: f64, threshold: f64) -> bool {
        match self {
            Op::Lt => value < threshold,
            Op::Le => value <= threshold,
            Op::Gt => value > threshold,
            Op::Ge => value >= threshold,
            Op::Eq => value == threshold,
            Op::Ne => value != threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lt_checks_strict_less_than() {
        assert!(Op::Lt.check(1.0, 2.0));
        assert!(!Op::Lt.check(2.0, 2.0));
        assert!(!Op::Lt.check(3.0, 2.0));
    }

    #[test]
    fn le_checks_less_than_or_equal() {
        assert!(Op::Le.check(1.0, 2.0));
        assert!(Op::Le.check(2.0, 2.0));
        assert!(!Op::Le.check(3.0, 2.0));
    }

    #[test]
    fn gt_checks_strict_greater_than() {
        assert!(!Op::Gt.check(1.0, 2.0));
        assert!(!Op::Gt.check(2.0, 2.0));
        assert!(Op::Gt.check(3.0, 2.0));
    }

    #[test]
    fn ge_checks_greater_than_or_equal() {
        assert!(!Op::Ge.check(1.0, 2.0));
        assert!(Op::Ge.check(2.0, 2.0));
        assert!(Op::Ge.check(3.0, 2.0));
    }

    #[test]
    fn eq_checks_bit_exact_equality() {
        assert!(Op::Eq.check(2.0, 2.0));
        assert!(!Op::Eq.check(2.0, 2.0000001));
    }

    #[test]
    fn ne_checks_bit_exact_inequality() {
        assert!(!Op::Ne.check(2.0, 2.0));
        assert!(Op::Ne.check(2.0, 2.0000001));
    }
}
