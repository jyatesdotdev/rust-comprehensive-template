//! Shared domain types used across workspace crates.
//!
//! These types form the common vocabulary for the application. All crates
//! depend on `common` and use these types to avoid duplicating definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A generic domain entity with a unique ID and timestamp.
///
/// `Entity` is the base record type shared across crates. It is
/// serializable and can be stored in a database or sent over the wire.
///
/// # Examples
///
/// ```rust
/// use common::Entity;
///
/// let entity = Entity::new("my-resource");
/// assert_eq!(entity.name, "my-resource");
/// assert!(!entity.id.is_nil());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier, generated as a v4 UUID.
    pub id: Uuid,
    /// Human-readable name for this entity.
    pub name: String,
    /// Timestamp of when the entity was created (UTC).
    pub created_at: DateTime<Utc>,
}

impl Entity {
    /// Creates a new `Entity` with a random UUID and the current timestamp.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_generates_unique_ids() {
        let a = Entity::new("a");
        let b = Entity::new("a");
        assert!(!a.id.is_nil());
        assert_ne!(a.id, b.id, "each entity must get its own UUID");
    }

    #[test]
    fn serde_round_trip() {
        let entity = Entity::new("resource");
        let json = serde_json::to_string(&entity).unwrap();
        let back: Entity = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, entity.id);
        assert_eq!(back.name, entity.name);
        assert_eq!(back.created_at, entity.created_at);
    }
}
