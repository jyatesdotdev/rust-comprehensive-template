//! CRUD handlers demonstrating Axum extractors.
//!
//! Each handler shows a different extractor pattern:
//! - [`list_items`] — `Query` extractor for filtering
//! - [`create_item`] — `Json` extractor for request body
//! - [`get_item`] — `Path` extractor for URL parameters
//! - [`delete_item`] — `Path` + `State` for mutations

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use common::AppError;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::{AppState, Item};

/// Query parameters for filtering items.
#[derive(Debug, serde::Deserialize)]
pub struct ListParams {
    /// If set, only return items containing this tag.
    pub tag: Option<String>,
}

/// Request body for creating an item.
#[derive(Debug, serde::Deserialize)]
pub struct CreateItem {
    /// Required item name (must not be empty).
    pub name: String,
    /// Optional tags; defaults to an empty list.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// `GET /items?tag=rust` — List items with optional tag filter.
pub async fn list_items(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let items = state.items.read().await;
    let result: Vec<Item> = items
        .values()
        .filter(|item| params.tag.as_ref().map_or(true, |t| item.tags.contains(t)))
        .cloned()
        .collect();
    Json(result)
}

/// `POST /items` — Create a new item from JSON body.
pub async fn create_item(
    State(state): State<AppState>,
    Json(payload): Json<CreateItem>,
) -> Result<impl IntoResponse, ApiError> {
    if payload.name.is_empty() {
        return Err(AppError::validation("name must not be empty").into());
    }

    let item = Item {
        id: Uuid::new_v4(),
        name: payload.name,
        tags: payload.tags,
    };

    let id = item.id;
    state.items.write().await.insert(id, item);

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// `GET /items/:id` — Get a single item by ID.
pub async fn get_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Item>, ApiError> {
    state
        .items
        .read()
        .await
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("item {id}")).into())
}

/// `DELETE /items/:id` — Delete an item by ID.
pub async fn delete_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .items
        .write()
        .await
        .remove(&id)
        .map(|_| StatusCode::NO_CONTENT)
        .ok_or_else(|| AppError::not_found(format!("item {id}")).into())
}
