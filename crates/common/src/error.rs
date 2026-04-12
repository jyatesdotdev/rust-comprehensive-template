//! Comprehensive error handling demonstrating idiomatic Rust patterns.
//!
//! # Error Handling Strategy
//!
//! This module uses a layered approach:
//! - [`AppError`] — Domain errors with [`thiserror`] for structured, typed errors
//! - [`anyhow::Error`] — Ad-hoc errors for application-level code via the `Internal` variant
//! - [`ResultExt`] — Extension trait for adding context to any `Result`
//!
//! # Examples
//!
//! ```rust
//! use common::error::{AppError, Result};
//!
//! fn find_user(id: u64) -> Result<String> {
//!     if id == 0 {
//!         return Err(AppError::validation("user ID must be non-zero"));
//!     }
//!     Err(AppError::not_found(format!("user {id}")))
//! }
//! ```
//!
//! Using the `?` operator with automatic conversion:
//!
//! ```rust
//! use common::error::{AppError, Result};
//!
//! fn parse_config(json: &str) -> Result<serde_json::Value> {
//!     // serde_json::Error automatically converts to AppError::Serialization
//!     let value = serde_json::from_str(json)?;
//!     Ok(value)
//! }
//! ```

use std::fmt;
use thiserror::Error;

/// Domain-specific errors using `thiserror` for derive macros.
///
/// Each variant maps to a specific failure mode. Use the constructor
/// methods (e.g., [`AppError::not_found`]) for ergonomic creation.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("timeout after {0}")]
    Timeout(HumanDuration),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience constructors to avoid `.to_string()` boilerplate.
impl AppError {
    /// Creates a [`NotFound`](Self::NotFound) error.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Creates a [`Validation`](Self::Validation) error.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Creates a [`Database`](Self::Database) error.
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Creates an [`Unauthorized`](Self::Unauthorized) error.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    /// Creates a [`Conflict`](Self::Conflict) error.
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    /// Creates a [`Timeout`](Self::Timeout) error with a human-readable duration.
    #[must_use]
    pub const fn timeout(duration: std::time::Duration) -> Self {
        Self::Timeout(HumanDuration(duration))
    }

    /// Wrap any error as an internal error via `anyhow`.
    pub fn internal(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Internal(anyhow::Error::new(err))
    }

    /// Returns true if this is a client error (not found, validation, auth).
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::NotFound(_)
                | Self::Validation(_)
                | Self::Unauthorized(_)
                | Self::Forbidden(_)
                | Self::Conflict(_)
        )
    }
}

/// Convenience Result type alias used throughout the crate.
pub type Result<T> = std::result::Result<T, AppError>;

/// A duration wrapper that displays in human-readable form.
#[derive(Debug)]
pub struct HumanDuration(pub std::time::Duration);

impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = self.0.as_millis();
        if ms < 1_000 {
            write!(f, "{ms}ms")
        } else {
            write!(f, "{:.1}s", self.0.as_secs_f64())
        }
    }
}

/// Extension trait for adding context to any `Result`.
///
/// This bridges the gap between typed `AppError` and ad-hoc `anyhow` context,
/// letting you annotate errors at call sites without defining new variants.
///
/// # Example
///
/// ```rust
/// use common::error::{AppError, ResultExt};
///
/// fn load_config() -> std::result::Result<String, std::io::Error> {
///     std::fs::read_to_string("config.toml")
/// }
///
/// fn init() -> common::error::Result<String> {
///     load_config().context_app("loading application config")
/// }
/// ```
pub trait ResultExt<T> {
    /// Convert any error into `AppError::Internal` with added context.
    ///
    /// # Errors
    /// Returns `AppError::Internal` wrapping the original error with the given context message.
    fn context_app(self, msg: &str) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T>
    for std::result::Result<T, E>
{
    fn context_app(self, msg: &str) -> Result<T> {
        self.map_err(|e| AppError::Internal(anyhow::Error::new(e).context(msg.to_owned())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = AppError::not_found("user 42");
        assert_eq!(err.to_string(), "not found: user 42");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn json_error_converts() {
        let result: std::result::Result<serde_json::Value, _> = serde_json::from_str("{bad");
        let app_err: AppError = result.unwrap_err().into();
        assert!(matches!(app_err, AppError::Serialization(_)));
    }

    #[test]
    fn client_error_classification() {
        assert!(AppError::not_found("x").is_client_error());
        assert!(AppError::validation("x").is_client_error());
        assert!(AppError::unauthorized("x").is_client_error());
        assert!(!AppError::database("x").is_client_error());
    }

    #[test]
    fn timeout_display() {
        let err = AppError::timeout(std::time::Duration::from_millis(500));
        assert_eq!(err.to_string(), "timeout after 500ms");

        let err = AppError::timeout(std::time::Duration::from_secs(3));
        assert_eq!(err.to_string(), "timeout after 3.0s");
    }

    #[test]
    fn context_ext_wraps_error() {
        let result: std::result::Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::Other, "disk full"));
        let app_result = result.context_app("writing cache");
        assert!(matches!(app_result, Err(AppError::Internal(_))));
        assert!(app_result.unwrap_err().to_string().contains("writing cache"));
    }

    #[test]
    fn question_mark_propagation() {
        fn parse(s: &str) -> Result<serde_json::Value> {
            Ok(serde_json::from_str(s)?)
        }
        assert!(parse("{}").is_ok());
        assert!(parse("bad").is_err());
    }
}
