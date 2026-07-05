//! HTTP client patterns using `reqwest`.
//!
//! Demonstrates typed responses, error handling, and client reuse.

use common::{AppError, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;

/// Reusable API client wrapping [`reqwest::Client`].
///
/// The inner `Client` uses connection pooling — create once, reuse everywhere.
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new client targeting the given base URL (e.g. `http://localhost:3000`).
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::internal(e))?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }

    /// GET a resource and deserialize the JSON response.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::internal(e))?;

        if !resp.status().is_success() {
            return Err(AppError::internal(std::io::Error::other(format!(
                "HTTP {}",
                resp.status()
            ))));
        }

        resp.json().await.map_err(|e| AppError::internal(e))
    }

    /// POST a JSON body and deserialize the response.
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::internal(e))?;

        if !resp.status().is_success() {
            return Err(AppError::internal(std::io::Error::other(format!(
                "HTTP {}",
                resp.status()
            ))));
        }

        resp.json().await.map_err(|e| AppError::internal(e))
    }
}
