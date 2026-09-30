//! `OverlayError` — public error type for `Session` operations.
//!
//! All public methods on [`Session`](crate::session::Session) return
//! `Result<_, OverlayError>`. No method panics on user-facing input.

use procedural_core::world::WorldError;
use std::fmt;

#[derive(Debug)]
pub enum OverlayError {
    /// The named space isn't registered in the underlying `World`.
    UnknownSpace(String),
    /// The named attribute isn't registered on the named space.
    UnknownAttribute { space: String, attr: String },
    /// Caller's `V` type generic doesn't match the attribute's
    /// registered value type.
    TypeMismatch {
        space: String,
        attr: String,
        expected: &'static str,
        actual: &'static str,
    },
    /// A read fell through to `World::attribute_value` (or another
    /// procedural_core entry point) and that returned an error.
    World(WorldError),
}

impl fmt::Display for OverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverlayError::UnknownSpace(s) => {
                write!(f, "unknown space: {}", s)
            }
            OverlayError::UnknownAttribute { space, attr } => write!(
                f,
                "space {} has no attribute named {}",
                space, attr
            ),
            OverlayError::TypeMismatch {
                space,
                attr,
                expected,
                actual,
            } => write!(
                f,
                "attribute {}.{} type mismatch: caller asked for {}, registered as {}",
                space, attr, expected, actual
            ),
            OverlayError::World(e) => write!(f, "procedural_core error: {}", e),
        }
    }
}

impl std::error::Error for OverlayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OverlayError::World(e) => Some(e),
            _ => None,
        }
    }
}

impl From<WorldError> for OverlayError {
    fn from(e: WorldError) -> Self {
        OverlayError::World(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_space_displays_the_space_name() {
        let e = OverlayError::UnknownSpace("inbox".into());
        assert_eq!(format!("{}", e), "unknown space: inbox");
    }

    #[test]
    fn unknown_attribute_displays_both_names() {
        let e = OverlayError::UnknownAttribute {
            space: "messages".into(),
            attr: "subject".into(),
        };
        let s = format!("{}", e);
        assert!(s.contains("messages"));
        assert!(s.contains("subject"));
    }

    #[test]
    fn type_mismatch_displays_expected_and_actual() {
        let e = OverlayError::TypeMismatch {
            space: "messages".into(),
            attr: "subject".into(),
            expected: "i64",
            actual: "alloc::string::String",
        };
        let s = format!("{}", e);
        assert!(s.contains("i64"));
        assert!(s.contains("String"));
        assert!(s.contains("messages.subject"));
    }

    #[test]
    fn world_error_chains_via_source() {
        let world_err = WorldError::UnknownSpace("nope".into());
        let e: OverlayError = world_err.into();
        // Display embeds the inner.
        assert!(format!("{}", e).contains("nope"));
        // source() returns the inner.
        let src = std::error::Error::source(&e).expect("has source");
        assert!(format!("{}", src).contains("nope"));
    }

    #[test]
    fn from_world_error_converts() {
        let world_err = WorldError::UnknownSpace("missing".into());
        let _: OverlayError = world_err.into();
    }

    #[test]
    fn debug_impl_works_on_all_variants() {
        // Smoke-test Debug for each variant — useful for assert_eq! diagnostics.
        let _ = format!("{:?}", OverlayError::UnknownSpace("s".into()));
        let _ = format!(
            "{:?}",
            OverlayError::UnknownAttribute {
                space: "s".into(),
                attr: "a".into(),
            }
        );
        let _ = format!(
            "{:?}",
            OverlayError::TypeMismatch {
                space: "s".into(),
                attr: "a".into(),
                expected: "x",
                actual: "y",
            }
        );
        let _ = format!(
            "{:?}",
            OverlayError::World(WorldError::UnknownSpace("s".into()))
        );
    }
}
