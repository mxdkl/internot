//! Type-coercion policy shared between `Space::context()` validation
//! and `search::candidates` scoring.
//!
//! `Space::context()` rejects dim attributes whose value type cannot be
//! coerced to `f64`. The actual coercion (downcast to a known concrete
//! type, then cast) lives in `search::candidates::coerce_fn_for`; this
//! module exists to keep both paths in sync via a single source of
//! truth for the supported-type set.

use std::any::TypeId;

/// True iff this attribute value type can be coerced to `f64` for
/// similarity scoring. Mirrors the dispatch in
/// `search::candidates::coerce_fn_for`: `f64`/`f32`, `u8..u64`,
/// `i8..i64`, `bool`. Anything else returns false.
pub(crate) fn is_coercable_numeric(type_id: TypeId) -> bool {
    type_id == TypeId::of::<f64>()
        || type_id == TypeId::of::<f32>()
        || type_id == TypeId::of::<u64>()
        || type_id == TypeId::of::<u32>()
        || type_id == TypeId::of::<u16>()
        || type_id == TypeId::of::<u8>()
        || type_id == TypeId::of::<i64>()
        || type_id == TypeId::of::<i32>()
        || type_id == TypeId::of::<i16>()
        || type_id == TypeId::of::<i8>()
        || type_id == TypeId::of::<bool>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_types_pass() {
        assert!(is_coercable_numeric(TypeId::of::<f64>()));
        assert!(is_coercable_numeric(TypeId::of::<f32>()));
        assert!(is_coercable_numeric(TypeId::of::<u64>()));
        assert!(is_coercable_numeric(TypeId::of::<u32>()));
        assert!(is_coercable_numeric(TypeId::of::<u16>()));
        assert!(is_coercable_numeric(TypeId::of::<u8>()));
        assert!(is_coercable_numeric(TypeId::of::<i64>()));
        assert!(is_coercable_numeric(TypeId::of::<i32>()));
        assert!(is_coercable_numeric(TypeId::of::<i16>()));
        assert!(is_coercable_numeric(TypeId::of::<i8>()));
        assert!(is_coercable_numeric(TypeId::of::<bool>()));
    }

    #[test]
    fn unsupported_types_fail() {
        assert!(!is_coercable_numeric(TypeId::of::<String>()));
        assert!(!is_coercable_numeric(TypeId::of::<&'static str>()));
        assert!(!is_coercable_numeric(TypeId::of::<usize>())); // not in the list
        assert!(!is_coercable_numeric(TypeId::of::<()>()));
        assert!(!is_coercable_numeric(TypeId::of::<Vec<f64>>()));
    }
}
