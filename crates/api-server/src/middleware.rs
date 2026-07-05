//! Custom middleware demonstrating Tower layer patterns.
//!
//! Shows a request-ID middleware that injects a unique identifier
//! into every request/response for distributed tracing.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Middleware that adds a unique `x-request-id` header to each request and response.
///
/// If the request already carries the header (e.g., from an API gateway), it is preserved.
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(&X_REQUEST_ID)
        .cloned()
        .unwrap_or_else(|| {
            // A hyphenated UUID is always a valid ASCII header value, so this
            // fallback is unreachable — it exists to keep library code panic-free.
            HeaderValue::from_str(&Uuid::new_v4().to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("invalid-request-id"))
        });

    req.headers_mut().insert(X_REQUEST_ID.clone(), id.clone());

    let mut res = next.run(req).await;
    res.headers_mut().insert(X_REQUEST_ID.clone(), id);
    res
}
