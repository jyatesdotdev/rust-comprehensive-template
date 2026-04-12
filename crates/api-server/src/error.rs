//! Maps domain errors to Axum HTTP responses.
//!
//! Uses a newtype wrapper to satisfy Rust's orphan rule — we can't implement
//! a foreign trait (`IntoResponse`) on a foreign type (`AppError`) directly.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use common::AppError;

/// Newtype wrapper enabling `IntoResponse` for [`AppError`].
///
/// Handlers return `Result<T, ApiError>` and use `?` with automatic conversion.
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            AppError::Database(_) | AppError::Io(_) | AppError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Serialization(_) => StatusCode::BAD_REQUEST,
        };

        let body = serde_json::json!({
            "error": self.0.to_string(),
            "status": status.as_u16(),
        });

        (status, axum::Json(body)).into_response()
    }
}
