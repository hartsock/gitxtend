//! Crate error type. The PyO3 conversion is gated behind the `python` feature so
//! the pure-Rust core has no PyO3 dependency.

use std::fmt;

/// Error raised by gitxtend read primitives that do not soft-fail.
///
/// On the Python side this surfaces as `GitxtendError` (a `RuntimeError`
/// subclass); see `docs/API.md`. Soft-fail methods return sentinels
/// (`None`/`0`/`[]`/`{}`) instead of producing this.
#[derive(Debug, Clone)]
pub struct GitxtendError {
    message: String,
}

impl GitxtendError {
    /// Build an error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Build an error from anything `Display` (e.g. a gix error).
    ///
    /// This is a named constructor rather than a blanket `From` impl, which
    /// would conflict with `From<Self>`. Use it as
    /// `.map_err(GitxtendError::from_err)`.
    pub fn from_err<E: fmt::Display>(e: E) -> Self {
        Self::new(e.to_string())
    }

    /// The human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for GitxtendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GitxtendError {}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, GitxtendError>;

#[cfg(feature = "python")]
impl From<GitxtendError> for pyo3::PyErr {
    fn from(e: GitxtendError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(e.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_message() {
        let e = GitxtendError::new("boom");
        assert_eq!(e.to_string(), "boom");
        assert_eq!(e.message(), "boom");
    }

    #[test]
    fn from_err_uses_display() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        assert_eq!(GitxtendError::from_err(io).message(), "nope");
    }
}
