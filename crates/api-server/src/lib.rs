//! RESTful API server using Axum with middleware, extractors, and client examples.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Tower Middleware Stack                  │
//! │  ├─ TraceLayer (request/response logs)  │
//! │  ├─ CorsLayer (cross-origin policy)     │
//! │  └─ request_id (custom middleware)      │
//! ├─────────────────────────────────────────┤
//! │  Axum Router                            │
//! │  ├─ GET    /items       → list_items    │
//! │  ├─ POST   /items       → create_item   │
//! │  ├─ GET    /items/:id   → get_item      │
//! │  └─ DELETE /items/:id   → delete_item   │
//! ├─────────────────────────────────────────┤
//! │  AppState (Arc<RwLock<HashMap>>)        │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Running
//!
//! ```rust,no_run
//! #[tokio::main]
//! async fn main() {
//!     let app = api_server::app();
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```

pub mod client;
mod error; // AppError → IntoResponse mapping (orphan rule: impl is here)
pub mod handlers;
pub mod middleware;
pub mod state;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use state::AppState;

/// Build the application router with all routes and middleware.
pub fn app() -> Router {
    let state = AppState::new();

    Router::new()
        .route(
            "/items",
            get(handlers::list_items).post(handlers::create_item),
        )
        .route(
            "/items/:id",
            get(handlers::get_item).delete(handlers::delete_item),
        )
        .with_state(state)
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

/// Start the server with graceful shutdown on ctrl-c.
pub async fn serve(addr: &str) -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("api_server=debug,tower_http=debug")
        .init();

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl-c handler");
    tracing::info!("shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn body_json(res: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn create_get_delete_roundtrip() {
        let app = app();

        let res = app
            .clone()
            .oneshot(post_json(
                "/items",
                serde_json::json!({ "name": "widget", "tags": ["rust"] }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let id = body_json(res).await["id"].as_str().unwrap().to_owned();

        let res = app
            .clone()
            .oneshot(get(&format!("/items/{id}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_json(res).await["name"], "widget");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/items/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let res = app.oneshot(get(&format!("/items/{id}"))).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_with_empty_name_is_unprocessable() {
        let res = app()
            .oneshot(post_json("/items", serde_json::json!({ "name": "" })))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn get_unknown_item_is_not_found() {
        let res = app()
            .oneshot(get("/items/00000000-0000-0000-0000-000000000000"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_filters_by_tag() {
        let app = app();
        for (name, tags) in [("a", vec!["x"]), ("b", vec!["y"])] {
            let res = app
                .clone()
                .oneshot(post_json(
                    "/items",
                    serde_json::json!({ "name": name, "tags": tags }),
                ))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::CREATED);
        }

        let res = app.clone().oneshot(get("/items?tag=x")).await.unwrap();
        let items = body_json(res).await;
        assert_eq!(items.as_array().unwrap().len(), 1);
        assert_eq!(items[0]["name"], "a");

        let res = app.oneshot(get("/items")).await.unwrap();
        assert_eq!(body_json(res).await.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn request_id_is_added_and_preserved() {
        let res = app().oneshot(get("/items")).await.unwrap();
        assert!(res.headers().contains_key("x-request-id"));

        let req = Request::builder()
            .uri("/items")
            .header("x-request-id", "gateway-supplied")
            .body(Body::empty())
            .unwrap();
        let res = app().oneshot(req).await.unwrap();
        assert_eq!(res.headers()["x-request-id"], "gateway-supplied");
    }
}
