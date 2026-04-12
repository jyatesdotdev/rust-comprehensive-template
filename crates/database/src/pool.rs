//! Connection pool configuration and creation.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

/// Pool configuration with sensible defaults.
pub struct PoolConfig {
    /// SQLite connection URL (e.g. `"sqlite::memory:"` or `"sqlite:data.db"`).
    pub url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Timeout when acquiring a connection from the pool.
    pub acquire_timeout: Duration,
    /// Create the database file if it does not exist.
    pub create_if_missing: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            url: "sqlite::memory:".into(),
            max_connections: 5,
            acquire_timeout: Duration::from_secs(5),
            create_if_missing: true,
        }
    }
}

/// Create a configured SQLite connection pool.
pub async fn create_pool(config: &PoolConfig) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(&config.url)?
        .create_if_missing(config.create_if_missing);

    SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .after_connect(|_conn, _meta| {
            Box::pin(async move {
                tracing::debug!("new database connection established");
                Ok(())
            })
        })
        .connect_with(options)
        .await
}
