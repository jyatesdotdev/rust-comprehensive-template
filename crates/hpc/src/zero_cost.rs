//! Zero-cost abstractions: generics, trait-based dispatch, and newtype wrappers
//! that compile away to the same code as hand-written implementations.

/// A generic accumulator that works over any numeric type via a trait.
/// Monomorphization ensures no dynamic dispatch overhead.
pub trait Accumulate: Default + Copy {
    /// Combine two values of this type (e.g. addition).
    fn add(self, other: Self) -> Self;
}

impl Accumulate for f64 {
    fn add(self, other: Self) -> Self { self + other }
}

impl Accumulate for i64 {
    fn add(self, other: Self) -> Self { self + other }
}

/// Generic sum — compiles to the same code as a hand-written loop for each type.
pub fn generic_sum<T: Accumulate>(data: &[T]) -> T {
    let mut acc = T::default();
    for &item in data {
        acc = acc.add(item);
    }
    acc
}

/// Newtype wrapper providing unit safety with zero runtime cost.
/// `Meters` and `Seconds` are distinct types at compile time but just `f64` at runtime.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Meters(pub f64);

/// Newtype wrapper for time in seconds — zero-cost unit safety.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

/// Newtype wrapper for velocity in meters per second — zero-cost unit safety.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MetersPerSecond(pub f64);

impl Meters {
    /// Compute velocity by dividing distance by time.
    pub fn per(self, time: Seconds) -> MetersPerSecond {
        MetersPerSecond(self.0 / time.0)
    }
}

/// Iterator adapter that fuses map + filter in a single pass (zero allocation).
pub fn sum_positive_squares(data: &[f64]) -> f64 {
    data.iter()
        .filter(|&&x| x > 0.0)
        .map(|x| x * x)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_sum_f64() {
        let data = [1.0, 2.0, 3.0, 4.0];
        assert!((generic_sum(&data) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn generic_sum_i64() {
        let data = [1i64, 2, 3, 4];
        assert_eq!(generic_sum(&data), 10);
    }

    #[test]
    fn newtype_units() {
        let speed = Meters(100.0).per(Seconds(9.58));
        assert!((speed.0 - 10.438).abs() < 0.001);
    }

    #[test]
    fn positive_squares() {
        let data = [-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        assert!((sum_positive_squares(&data) - 14.0).abs() < f64::EPSILON);
    }
}
