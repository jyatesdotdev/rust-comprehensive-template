//! Database interaction patterns with sqlx, connection pooling, and migrations.
//!
//! # Modules
//! - [`pool`] — Connection pool configuration and creation
//! - [`migrate`] — Schema migration runner
//! - [`repo`] — Repository pattern with CRUD + transactions
//!
//! # Quick Start
//! ```no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use database::{pool, migrate, repo};
//!
//! let pool = pool::create_pool(&pool::PoolConfig::default()).await?;
//! migrate::run(&pool).await?;
//! let repo = repo::EntityRepo::new(pool);
//! let entity = repo.create("example").await?;
//! # Ok(())
//! # }
//! ```

pub mod migrate;
pub mod pool;
pub mod repo;
