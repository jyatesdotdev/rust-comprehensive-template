//! Shared application state passed to handlers via Axum's `State` extractor.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Item stored in our in-memory database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Arbitrary tags for filtering.
    pub tags: Vec<String>,
}

/// Shared state accessible from all handlers.
#[derive(Clone)]
pub struct AppState {
    /// Concurrent map of items keyed by [`Uuid`].
    pub items: Arc<RwLock<HashMap<Uuid, Item>>>,
}

impl AppState {
    /// Create an empty application state.
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
