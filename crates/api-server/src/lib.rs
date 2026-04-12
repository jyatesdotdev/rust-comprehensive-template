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

mod error; // AppError → IntoResponse mapping (orphan rule: impl is here)
pub mod client;
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
        .route("/items", get(handlers::list_items).post(handlers::create_item))
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
