//! Integration tests — run as a separate binary, only access the public API.

use testing::collections::SortedSet;
use testing::math;

// ---------------------------------------------------------------------------
// Cross-module integration
// ---------------------------------------------------------------------------

#[test]
fn sorted_set_serialization_roundtrip() {
    let set: SortedSet<i32> = vec![4, 1, 3, 1, 2].into_iter().collect();
    let json = serde_json::to_string(&set).unwrap();
    let deserialized: SortedSet<i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(set, deserialized);
}

#[test]
fn sorted_set_merge() {
    let mut a: SortedSet<i32> = vec![1, 3, 5].into_iter().collect();
    let b: SortedSet<i32> = vec![2, 3, 4].into_iter().collect();
    a.merge(b);
    assert_eq!(a.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn math_functions_compose() {
    // gcd of consecutive fibonacci numbers is always 1
    for n in 2..20 {
        let a = math::fibonacci(n);
        let b = math::fibonacci(n + 1);
        assert_eq!(math::gcd(a, b), 1, "gcd(fib({n}), fib({}))", n + 1);
    }
}

// ---------------------------------------------------------------------------
// Async integration test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn async_sorted_set_build() {
    // Simulate collecting results from async tasks into a SortedSet.
    let handles: Vec<_> = (0..5).map(|i| tokio::spawn(async move { i * 2 })).collect();

    let mut set = SortedSet::new();
    for h in handles {
        set.insert(h.await.unwrap());
    }
    assert_eq!(set.as_slice(), &[0, 2, 4, 6, 8]);
}
