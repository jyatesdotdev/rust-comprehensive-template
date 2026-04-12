//! ETL and data processing: iterator chains, parallel batch processing, streaming pipelines.
//!
//! # Modules
//!
//! - [`pipeline`] — Composable Extract→Transform→Load pipeline with trait-based stages
//! - [`iterators`] — Zero-cost iterator chains for data transformation and aggregation
//! - [`parallel`] — Rayon-based parallel batch ETL processing
//! - [`streaming`] — Async streaming pipelines with backpressure via tokio channels

pub mod iterators;
pub mod parallel;
pub mod pipeline;
pub mod streaming;
