//! The crate's error type.

use std::fmt;

/// An error from loading, saving, profiling or measuring. The message
/// already carries its context (which file, which benchmark), so callers
/// print it as is.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    /// An error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
