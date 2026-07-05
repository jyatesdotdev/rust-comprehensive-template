//! Embedded migration runner.

use sqlx::SqlitePool;

/// Migration files embedded at compile time, in apply order.
///
/// `include_str!` keeps the runner in lockstep with the files under
/// `migrations/` — the SQL is never duplicated in Rust source.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/001_create_entities.sql")];

/// Run all embedded migrations against the pool.
///
/// In production, use the `sqlx migrate run` CLI or the `sqlx::migrate!()`
/// macro (which also tracks applied versions). This manual approach works
/// without the sqlx CLI tooling and relies on the migrations being
/// idempotent (`CREATE ... IF NOT EXISTS`).
pub async fn run(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for migration in MIGRATIONS {
        // `raw_sql` executes multi-statement scripts; `query` cannot.
        sqlx::raw_sql(migration).execute(pool).await?;
    }

    tracing::info!("migrations applied successfully");
    Ok(())
}
