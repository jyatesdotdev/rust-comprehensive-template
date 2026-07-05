//! Typestate pattern: encode protocol/state-machine rules in the type system.
//!
//! A `Connection` must go through Disconnected → Connected → Authenticated.
//! Calling methods in the wrong order is a compile-time error.

use std::marker::PhantomData;

// State marker types.
/// Marker: connection is not yet established.
pub struct Disconnected;
/// Marker: connection is established but not authenticated.
pub struct Connected;
/// Marker: connection is established and authenticated.
pub struct Authenticated;

/// A stateful connection whose allowed operations depend on the type parameter `S`.
pub struct Connection<S> {
    addr: String,
    token: Option<String>,
    _state: PhantomData<S>,
}

impl Connection<Disconnected> {
    /// Create a new disconnected connection targeting `addr`.
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            token: None,
            _state: PhantomData,
        }
    }

    /// Transition: Disconnected → Connected.
    pub fn connect(self) -> Result<Connection<Connected>, &'static str> {
        if self.addr.is_empty() {
            return Err("empty address");
        }
        Ok(Connection {
            addr: self.addr,
            token: None,
            _state: PhantomData,
        })
    }
}

impl Connection<Connected> {
    /// Transition: Connected → Authenticated.
    pub fn authenticate(self, token: impl Into<String>) -> Connection<Authenticated> {
        Connection {
            addr: self.addr,
            token: Some(token.into()),
            _state: PhantomData,
        }
    }

    /// Can also disconnect back.
    pub fn disconnect(self) -> Connection<Disconnected> {
        Connection {
            addr: self.addr,
            token: None,
            _state: PhantomData,
        }
    }
}

impl Connection<Authenticated> {
    /// Only available in authenticated state.
    pub fn query(&self, q: &str) -> String {
        // INVARIANT: the only way to reach `Authenticated` is via
        // `authenticate()`, which stores `Some(token)`; the fallback is
        // unreachable but keeps this library path panic-free by convention.
        let token = self.token.as_deref().unwrap_or("<missing>");
        format!("executed '{q}' on {} with token {token}", self.addr)
    }

    /// Disconnect, returning to the `Disconnected` state.
    pub fn disconnect(self) -> Connection<Disconnected> {
        Connection {
            addr: self.addr,
            token: None,
            _state: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let conn = Connection::new("db://localhost")
            .connect()
            .unwrap()
            .authenticate("secret");
        let result = conn.query("SELECT 1");
        assert!(result.contains("SELECT 1"));
    }

    #[test]
    fn empty_addr_fails() {
        assert!(Connection::new("").connect().is_err());
    }

    #[test]
    fn disconnect_and_reconnect() {
        let conn = Connection::new("host")
            .connect()
            .unwrap()
            .disconnect()
            .connect()
            .unwrap()
            .authenticate("tok");
        assert!(conn.query("q").contains("tok"));
    }
}
