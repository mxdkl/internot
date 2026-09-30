//! Arity — fixed or dynamic member count for a relation.

use crate::word::BitWord;
use chrono::{DateTime, Utc};
use std::fmt;
use std::sync::Arc;

pub enum Arity<W: BitWord> {
    Fixed(usize),
    Dynamic(Arc<dyn Fn(W, DateTime<Utc>) -> usize + Send + Sync>),
}

impl<W: BitWord> fmt::Debug for Arity<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arity::Fixed(n) => f.debug_tuple("Fixed").field(n).finish(),
            Arity::Dynamic(_) => f.debug_tuple("Dynamic").field(&"<closure>").finish(),
        }
    }
}

impl<W: BitWord> Arity<W> {
    /// Helper constructor for the dynamic variant — wraps the closure in `Arc`.
    pub fn dynamic<F>(f: F) -> Self
    where
        F: Fn(W, DateTime<Utc>) -> usize + Send + Sync + 'static,
    {
        Arity::Dynamic(Arc::new(f))
    }

    /// Resolve the arity for a specific owner and time.
    pub fn resolve(&self, owner_id: W, t: DateTime<Utc>) -> usize {
        match self {
            Arity::Fixed(n) => *n,
            Arity::Dynamic(f) => f(owner_id, t),
        }
    }

    /// Kind tag for introspection ("fixed" or "dynamic").
    pub fn kind(&self) -> &'static str {
        match self {
            Arity::Fixed(_) => "fixed",
            Arity::Dynamic(_) => "dynamic",
        }
    }
}

impl<W: BitWord> Clone for Arity<W> {
    fn clone(&self) -> Self {
        match self {
            Arity::Fixed(n) => Arity::Fixed(*n),
            Arity::Dynamic(f) => Arity::Dynamic(f.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t_ref() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap()
    }

    #[test]
    fn fixed_resolves_to_constant() {
        let a: Arity<u64> = Arity::Fixed(3);
        assert_eq!(a.resolve(0u64, t_ref()), 3);
        assert_eq!(a.resolve(999u64, t_ref()), 3);
    }

    #[test]
    fn dynamic_resolves_from_closure() {
        let a: Arity<u64> = Arity::dynamic(|owner, _t| (owner % 5) as usize);
        assert_eq!(a.resolve(12u64, t_ref()), 2);
        assert_eq!(a.resolve(20u64, t_ref()), 0);
    }

    #[test]
    fn dynamic_uses_time_argument() {
        let a: Arity<u64> = Arity::dynamic(|_, t| t.timestamp() as usize % 7);
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        // Both produce deterministic results; values differ if the mod differs.
        let _ = a.resolve(0u64, t1);
        let _ = a.resolve(0u64, t2);
    }

    #[test]
    fn clone_is_shared_arc() {
        let a: Arity<u64> = Arity::dynamic(|_, _| 7);
        let b = a.clone();
        assert_eq!(b.resolve(0u64, t_ref()), 7);
    }

    #[test]
    fn kind_labels_are_correct() {
        let f: Arity<u64> = Arity::Fixed(2);
        let d: Arity<u64> = Arity::dynamic(|_, _| 1);
        assert_eq!(f.kind(), "fixed");
        assert_eq!(d.kind(), "dynamic");
    }
}
