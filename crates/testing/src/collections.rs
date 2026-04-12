//! A simple sorted-set collection — used as a subject for integration tests.

use serde::{Deserialize, Serialize};

/// A sorted, deduplicated collection of `T`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SortedSet<T: Ord> {
    inner: Vec<T>,
}

impl<T: Ord> SortedSet<T> {
    /// Creates an empty `SortedSet`.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Insert `value`, maintaining sorted order. Returns `true` if inserted.
    pub fn insert(&mut self, value: T) -> bool {
        match self.inner.binary_search(&value) {
            Ok(_) => false,
            Err(pos) => {
                self.inner.insert(pos, value);
                true
            }
        }
    }

    /// Returns `true` if the set contains `value`.
    pub fn contains(&self, value: &T) -> bool {
        self.inner.binary_search(value).is_ok()
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no elements.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the elements as a sorted slice.
    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    /// Merge another set into this one.
    pub fn merge(&mut self, other: SortedSet<T>) {
        for item in other.inner {
            self.insert(item);
        }
    }
}

impl<T: Ord> Default for SortedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> FromIterator<T> for SortedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        for item in iter {
            set.insert(item);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_contains() {
        let mut s = SortedSet::new();
        assert!(s.insert(3));
        assert!(s.insert(1));
        assert!(!s.insert(3)); // duplicate
        assert!(s.contains(&1));
        assert!(!s.contains(&2));
        assert_eq!(s.as_slice(), &[1, 3]);
    }

    #[test]
    fn from_iterator_deduplicates() {
        let s: SortedSet<i32> = vec![5, 3, 5, 1, 3].into_iter().collect();
        assert_eq!(s.as_slice(), &[1, 3, 5]);
    }

    // -- proptest ------------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// After inserting, the set always contains the value.
            #[test]
            fn insert_then_contains(values in prop::collection::vec(0i32..1000, 0..100)) {
                let mut set = SortedSet::new();
                for &v in &values {
                    set.insert(v);
                }
                for &v in &values {
                    prop_assert!(set.contains(&v));
                }
            }

            /// The internal vec is always sorted.
            #[test]
            fn always_sorted(values in prop::collection::vec(0i32..1000, 0..100)) {
                let set: SortedSet<i32> = values.into_iter().collect();
                let slice = set.as_slice();
                for w in slice.windows(2) {
                    prop_assert!(w[0] <= w[1]);
                }
            }

            /// Length never exceeds the number of unique values.
            #[test]
            fn len_le_unique(values in prop::collection::vec(0i32..50, 0..200)) {
                let set: SortedSet<i32> = values.iter().copied().collect();
                let mut unique = values.clone();
                unique.sort();
                unique.dedup();
                prop_assert_eq!(set.len(), unique.len());
            }
        }
    }
}
