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

        // Never leak internal error detail (connection strings, file paths, …)
        // to clients: log it, return a generic message for 500-class variants.
        let message = match &self.0 {
            AppError::Database(_) | AppError::Io(_) | AppError::Internal(_) => {
                tracing::error!(error = %self.0, "internal server error");
                "internal server error".to_owned()
            }
            other => other.to_string(),
        };

        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16(),
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn body_json(res: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn client_errors_keep_their_message() {
        let res = ApiError(AppError::not_found("item 42")).into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let body = body_json(res).await;
        assert_eq!(body["error"], "not found: item 42");
        assert_eq!(body["status"], 404);
    }

    #[tokio::test]
    async fn internal_error_detail_is_not_leaked() {
        let res =
            ApiError(AppError::database("password=hunter2 in connection string")).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = body_json(res).await;
        assert_eq!(body["error"], "internal server error");
    }
}
