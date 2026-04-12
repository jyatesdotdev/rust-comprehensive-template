//! Math utilities — used as subjects for testing demonstrations.

/// Clamps `value` into `[min, max]`.
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Returns the greatest common divisor of two non-negative integers.
pub fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Fibonacci via iterative accumulation. Returns the `n`-th Fibonacci number.
pub fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let (mut a, mut b) = (0u64, 1u64);
            for _ in 2..=n {
                let next = a.wrapping_add(b);
                a = b;
                b = next;
            }
            b
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests — co-located with the module they test
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // -- clamp ---------------------------------------------------------------

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
    }

    #[test]
    fn clamp_below_min() {
        assert_eq!(clamp(-1.0, 0.0, 10.0), 0.0);
    }

    #[test]
    fn clamp_above_max() {
        assert_eq!(clamp(42.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn clamp_at_boundaries() {
        assert_eq!(clamp(0.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp(10.0, 0.0, 10.0), 10.0);
    }

    // -- gcd -----------------------------------------------------------------

    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 13), 1);
    }

    #[test]
    fn gcd_with_zero() {
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(5, 0), 5);
        assert_eq!(gcd(0, 0), 0);
    }

    // -- fibonacci -----------------------------------------------------------

    #[test]
    fn fibonacci_base_cases() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
    }

    #[test]
    fn fibonacci_known_values() {
        assert_eq!(fibonacci(10), 55);
        assert_eq!(fibonacci(20), 6765);
    }

    // -- proptest: property-based testing ------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// clamp output is always within [min, max].
            #[test]
            fn clamp_always_in_range(v in -1e6f64..1e6, lo in -1e3f64..0.0, hi in 0.0f64..1e3) {
                let result = clamp(v, lo, hi);
                prop_assert!(result >= lo);
                prop_assert!(result <= hi);
            }

            /// gcd(a, b) divides both a and b.
            #[test]
            fn gcd_divides_both(a in 1u64..10_000, b in 1u64..10_000) {
                let g = gcd(a, b);
                prop_assert!(g > 0);
                prop_assert_eq!(a % g, 0);
                prop_assert_eq!(b % g, 0);
            }

            /// gcd is commutative.
            #[test]
            fn gcd_commutative(a in 0u64..10_000, b in 0u64..10_000) {
                prop_assert_eq!(gcd(a, b), gcd(b, a));
            }

            /// Fibonacci is monotonically non-decreasing.
            #[test]
            fn fibonacci_monotonic(n in 1u32..80) {
                prop_assert!(fibonacci(n) >= fibonacci(n - 1));
            }
        }
    }
}
