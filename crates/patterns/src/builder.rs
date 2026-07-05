//! Builder pattern with compile-time required-field enforcement.
//!
//! Uses generics + marker types so the compiler rejects incomplete builds.

use std::marker::PhantomData;

// Marker types for builder state.
/// Marker: required field has not been set yet.
pub struct Missing;
/// Marker: required field has been set.
pub struct Set;

/// A request with method, url, and optional headers/body.
#[derive(Debug, Clone)]
pub struct Request {
    /// HTTP method (e.g. `"GET"`, `"POST"`).
    pub method: String,
    /// Target URL.
    pub url: String,
    /// Header key-value pairs.
    pub headers: Vec<(String, String)>,
    /// Optional request body.
    pub body: Option<Vec<u8>>,
}

/// Type-safe builder: `build()` is only available when both M and U are `Set`.
pub struct RequestBuilder<M, U> {
    method: Option<String>,
    url: Option<String>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    _marker: PhantomData<(M, U)>,
}

impl RequestBuilder<Missing, Missing> {
    /// Create a new builder with no fields set.
    pub fn new() -> Self {
        Self {
            method: None,
            url: None,
            headers: Vec::new(),
            body: None,
            _marker: PhantomData,
        }
    }
}

impl Default for RequestBuilder<Missing, Missing> {
    fn default() -> Self {
        Self::new()
    }
}

impl<U> RequestBuilder<Missing, U> {
    /// Set the HTTP method, transitioning `M` from `Missing` to `Set`.
    pub fn method(self, method: impl Into<String>) -> RequestBuilder<Set, U> {
        RequestBuilder {
            method: Some(method.into()),
            url: self.url,
            headers: self.headers,
            body: self.body,
            _marker: PhantomData,
        }
    }
}

impl<M> RequestBuilder<M, Missing> {
    /// Set the target URL, transitioning `U` from `Missing` to `Set`.
    pub fn url(self, url: impl Into<String>) -> RequestBuilder<M, Set> {
        RequestBuilder {
            method: self.method,
            url: Some(url.into()),
            headers: self.headers,
            body: self.body,
            _marker: PhantomData,
        }
    }
}

impl<M, U> RequestBuilder<M, U> {
    /// Add a header key-value pair.
    pub fn header(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.headers.push((key.into(), val.into()));
        self
    }

    /// Set the request body.
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }
}

impl RequestBuilder<Set, Set> {
    /// Only callable when both method and url have been set.
    pub fn build(self) -> Request {
        Request {
            // INVARIANT: `M = Set` / `U = Set` are only reachable through
            // `method()` / `url()`, which store `Some(..)`, so these unwraps
            // cannot panic. This is the one sanctioned unwrap in this crate.
            method: self.method.unwrap(),
            url: self.url.unwrap(),
            headers: self.headers,
            body: self.body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_complete_request() {
        let req = RequestBuilder::new()
            .method("GET")
            .url("https://example.com")
            .header("Accept", "application/json")
            .build();
        assert_eq!(req.method, "GET");
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.headers.len(), 1);
    }

    #[test]
    fn order_independent() {
        let req = RequestBuilder::new()
            .url("/api")
            .method("POST")
            .body(b"hello".to_vec())
            .build();
        assert_eq!(req.method, "POST");
        assert!(req.body.is_some());
    }
}
