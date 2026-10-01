//! Errors that say where: the pack, the file, and the line or field.

use std::fmt;

/// A world pack failed to load or validate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefError {
    /// The pack being loaded (the child, for merged sections).
    pub pack: String,
    /// The file within the pack, e.g. `mortality.ron`, if known.
    pub file: Option<String>,
    /// Where in the file: `line:col` for syntax errors, a field path such as
    /// `rate.anchors[3]` for type and validation errors.
    pub at: Option<String>,
    /// What is wrong.
    pub message: String,
}

impl DefError {
    pub(crate) fn new(pack: &str, file: Option<&str>, at: Option<String>, message: String) -> Self {
        Self {
            pack: pack.to_string(),
            file: file.map(str::to_string),
            at,
            message,
        }
    }

    /// A validation error in a section, at a field path.
    pub fn invalid(
        pack: &str,
        file: &str,
        at: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(pack, Some(file), Some(at.into()), message.into())
    }
}

impl fmt::Display for DefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "world pack `{}`", self.pack)?;
        if let Some(file) = &self.file {
            write!(f, ", {file}")?;
        }
        if let Some(at) = &self.at {
            write!(f, " at {at}")?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for DefError {}
