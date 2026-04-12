//! Repository pattern for entity CRUD operations.

use chrono::{DateTime, Utc};
use common::Entity;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Row type matching the database schema.
#[derive(Debug, sqlx::FromRow)]
struct EntityRow {
    id: String,
    name: String,
    created_at: String,
}

impl TryFrom<EntityRow> for Entity {
    type Error = common::AppError;

    fn try_from(row: EntityRow) -> Result<Self, Self::Error> {
        Ok(Entity {
            id: row.id.parse().map_err(|e| common::AppError::validation(format!("bad uuid: {e}")))?,
            name: row.name,
            created_at: row
                .created_at
                .parse::<DateTime<Utc>>()
                .map_err(|e| common::AppError::validation(format!("bad timestamp: {e}")))?,
        })
    }
}

/// Entity repository backed by SQLite.
pub struct EntityRepo {
    pool: SqlitePool,
}

impl EntityRepo {
    /// Create a new repository backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new entity, returning it.
    pub async fn create(&self, name: &str) -> Result<Entity, common::AppError> {
        let entity = Entity::new(name);
        let id = entity.id.to_string();
        let ts = entity.created_at.to_rfc3339();

        sqlx::query("INSERT INTO entities (id, name, created_at) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&entity.name)
            .bind(&ts)
            .execute(&self.pool)
            .await
            .map_err(|e| common::AppError::database(e.to_string()))?;

        Ok(entity)
    }

    /// Find an entity by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Entity>, common::AppError> {
        let row: Option<EntityRow> =
            sqlx::query_as("SELECT id, name, created_at FROM entities WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| common::AppError::database(e.to_string()))?;

        row.map(Entity::try_from).transpose()
    }

    /// List entities with pagination.
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Entity>, common::AppError> {
        let rows: Vec<EntityRow> = sqlx::query_as(
            "SELECT id, name, created_at FROM entities ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::AppError::database(e.to_string()))?;

        rows.into_iter().map(Entity::try_from).collect()
    }

    /// Update an entity's name. Returns true if a row was modified.
    pub async fn update_name(&self, id: Uuid, name: &str) -> Result<bool, common::AppError> {
        let result = sqlx::query("UPDATE entities SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| common::AppError::database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete an entity. Returns true if a row was removed.
    pub async fn delete(&self, id: Uuid) -> Result<bool, common::AppError> {
        let result = sqlx::query("DELETE FROM entities WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| common::AppError::database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Demonstrate a transaction: create multiple entities atomically.
    pub async fn create_batch(&self, names: &[&str]) -> Result<Vec<Entity>, common::AppError> {
        let mut tx: sqlx::Transaction<'_, sqlx::Sqlite> = self
            .pool
            .begin()
            .await
            .map_err(|e| common::AppError::database(e.to_string()))?;

        let mut entities = Vec::with_capacity(names.len());
        for name in names {
            let entity = Entity::new(*name);
            let id = entity.id.to_string();
            let ts = entity.created_at.to_rfc3339();

            sqlx::query("INSERT INTO entities (id, name, created_at) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(&entity.name)
                .bind(&ts)
                .execute(&mut *tx)
                .await
                .map_err(|e| common::AppError::database(e.to_string()))?;

            entities.push(entity);
        }

        tx.commit()
            .await
            .map_err(|e| common::AppError::database(e.to_string()))?;

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, pool};

    async fn setup() -> EntityRepo {
        let pool = pool::create_pool(&pool::PoolConfig::default()).await.unwrap();
        migrate::run(&pool).await.unwrap();
        EntityRepo::new(pool)
    }

    #[tokio::test]
    async fn create_and_find() {
        let repo = setup().await;
        let entity = repo.create("test").await.unwrap();
        let found = repo.find_by_id(entity.id).await.unwrap().unwrap();
        assert_eq!(found.name, "test");
    }

    #[tokio::test]
    async fn list_with_pagination() {
        let repo = setup().await;
        repo.create("a").await.unwrap();
        repo.create("b").await.unwrap();
        repo.create("c").await.unwrap();

        let page = repo.list(2, 0).await.unwrap();
        assert_eq!(page.len(), 2);

        let page2 = repo.list(2, 2).await.unwrap();
        assert_eq!(page2.len(), 1);
    }

    #[tokio::test]
    async fn update_and_delete() {
        let repo = setup().await;
        let entity = repo.create("old").await.unwrap();

        assert!(repo.update_name(entity.id, "new").await.unwrap());
        let updated = repo.find_by_id(entity.id).await.unwrap().unwrap();
        assert_eq!(updated.name, "new");

        assert!(repo.delete(entity.id).await.unwrap());
        assert!(repo.find_by_id(entity.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn batch_transaction() {
        let repo = setup().await;
        let entities = repo.create_batch(&["x", "y", "z"]).await.unwrap();
        assert_eq!(entities.len(), 3);

        let all = repo.list(10, 0).await.unwrap();
        assert_eq!(all.len(), 3);
    }
}
