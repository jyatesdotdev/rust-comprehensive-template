//! Common types, error handling, and shared utilities.
//!
//! Re-exports the most commonly used items for convenience:
//! ```rust
//! use common::{AppError, Result, Entity};
//! ```

pub mod error;
pub mod types;

// Re-export key types at crate root for ergonomic imports.
pub use error::{AppError, Result, ResultExt};
pub use types::Entity;
