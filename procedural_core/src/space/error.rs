//! Errors returned by Space registration and query operations.

use crate::space::context::Normalization;
use std::any::type_name;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SpaceError {
    DuplicateAttributeName(String),
    DuplicateRelationName(String),
    DuplicateContextName(String),
    UnknownAttribute(String),
    UnknownRelation(String),
    UnknownBitField(String),
    AttributeTypeMismatch {
        name: String,
        expected: &'static str,
        actual: &'static str,
    },
    MissingTime {
        name: String,
    },
    UnknownContextDimension {
        context: String,
        attribute: String,
    },
    /// A context dim's attribute type cannot be coerced to `f64` for
    /// similarity scoring — caught at `Space::context()` registration so
    /// the panic doesn't surface deep inside `candidates().take()`.
    UnsupportedDimType {
        context: String,
        attribute: String,
        type_name: &'static str,
    },
    /// A context dim requested a `Normalization` variant that is not
    /// implemented yet (only `Normalization::None` is supported in v0.1).
    UnsupportedNormalization {
        context: String,
        attribute: String,
        normalization: Normalization,
    },
    CrossSpaceRequiresWorld(String),
}

impl fmt::Display for SpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpaceError::DuplicateAttributeName(n) => write!(f, "duplicate attribute name: {}", n),
            SpaceError::DuplicateRelationName(n) => write!(f, "duplicate relation name: {}", n),
            SpaceError::DuplicateContextName(n) => write!(f, "duplicate context name: {}", n),
            SpaceError::UnknownAttribute(n) => write!(f, "unknown attribute: {}", n),
            SpaceError::UnknownRelation(n) => write!(f, "unknown relation: {}", n),
            SpaceError::UnknownBitField(n) => write!(f, "unknown bit-field: {}", n),
            SpaceError::AttributeTypeMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "attribute {} type mismatch: expected {}, got {}",
                name, expected, actual
            ),
            SpaceError::MissingTime { name } => {
                write!(f, "temporal attribute {} requires a time parameter", name)
            }
            SpaceError::UnknownContextDimension { context, attribute } => write!(
                f,
                "context {} references unknown attribute: {}",
                context, attribute
            ),
            SpaceError::UnsupportedDimType {
                context,
                attribute,
                type_name,
            } => write!(
                f,
                "context {} dim {} has type {} which is not coercable to f64 \
                 (supported: f64/f32/u8..u64/i8..i64/bool)",
                context, attribute, type_name
            ),
            SpaceError::UnsupportedNormalization {
                context,
                attribute,
                normalization,
            } => write!(
                f,
                "context {} dim {} requested Normalization::{:?}, which is \
                 not implemented in v0.1 — use Normalization::None",
                context, attribute, normalization
            ),
            SpaceError::CrossSpaceRequiresWorld(n) => write!(
                f,
                "attribute {} is cross-space; query via World::attribute_value",
                n
            ),
        }
    }
}

impl std::error::Error for SpaceError {}

pub(crate) fn type_label<T: 'static>() -> &'static str {
    type_name::<T>()
}
