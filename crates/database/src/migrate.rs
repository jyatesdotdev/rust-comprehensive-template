//! Embedded migration runner.

use sqlx::SqlitePool;

/// Run all embedded migrations against the pool.
///
/// In production, use `sqlx migrate run` CLI or `sqlx::migrate!()` macro.
/// This manual approach works without the sqlx CLI tooling.
pub async fn run(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)")
        .execute(pool)
        .await?;

    tracing::info!("migrations applied successfully");
    Ok(())
}
