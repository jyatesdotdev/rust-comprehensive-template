//! Newtype pattern for type-safe domain primitives.
//!
//! Wrapping raw types prevents mixing up semantically different values.

use std::fmt;

/// A validated email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    /// Parse and validate an email address. Returns `Err` if the input lacks `@` or is too short.
    pub fn new(s: impl Into<String>) -> Result<Self, &'static str> {
        let s = s.into();
        if s.contains('@') && s.len() > 3 {
            Ok(Self(s))
        } else {
            Err("invalid email")
        }
    }

    /// Return the email as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Strongly-typed identifiers prevent mixing user IDs with order IDs, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id<T>(u64, std::marker::PhantomData<T>);

impl<T> Id<T> {
    /// Create a new typed identifier from a raw `u64`.
    pub fn new(val: u64) -> Self {
        Self(val, std::marker::PhantomData)
    }

    /// Extract the underlying `u64` value.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Domain marker types — zero-sized, used only as type parameters.
/// Marker type for user identifiers.
pub struct UserTag;
/// Marker type for order identifiers.
pub struct OrderTag;

/// User identifier (type-safe alias for `Id<UserTag>`).
pub type UserId = Id<UserTag>;
/// Order identifier (type-safe alias for `Id<OrderTag>`).
pub type OrderId = Id<OrderTag>;

/// Unit-safe wrapper for distances (prevents mixing meters and kilometers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meters(pub f64);

/// Unit-safe wrapper for distances in kilometers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kilometers(pub f64);

impl From<Kilometers> for Meters {
    fn from(km: Kilometers) -> Self {
        Meters(km.0 * 1000.0)
    }
}

impl From<Meters> for Kilometers {
    fn from(m: Meters) -> Self {
        Kilometers(m.0 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_validation() {
        assert!(Email::new("a@b.c").is_ok());
        assert!(Email::new("bad").is_err());
    }

    #[test]
    fn typed_ids_not_interchangeable() {
        let uid: UserId = Id::new(1);
        let oid: OrderId = Id::new(1);
        // Same underlying value, but different types — can't accidentally swap.
        assert_eq!(uid.value(), oid.value());
        // uid == oid would not compile.
    }

    #[test]
    fn unit_conversion() {
        let km = Kilometers(5.0);
        let m: Meters = km.into();
        assert!((m.0 - 5000.0).abs() < f64::EPSILON);
    }
}
