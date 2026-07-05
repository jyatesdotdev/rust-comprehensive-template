//! Tiny deterministic pseudo-random number generator for weight init.
//!
//! Hand-rolled **xorshift64\*** in ~20 lines, so this crate keeps its
//! zero-dependency promise while still initializing weights "randomly".
//! What matters for teaching (and for CI) is not statistical quality but
//! *reproducibility*: the same seed must yield the same weights, the same
//! training trajectory, and the same final loss on every run and platform.
//! Production code should use the `rand` crate instead — this generator is
//! not cryptographically secure and has known statistical weaknesses that
//! simply do not matter for seeding a toy network.

/// Deterministic xorshift64* generator. Same seed ⇒ same sequence, always.
///
/// ```
/// use ml::Rng;
///
/// let mut a = Rng::new(42);
/// let mut b = Rng::new(42);
/// assert_eq!(a.next_u64(), b.next_u64());
/// ```
pub struct Rng {
    /// Internal 64-bit state; must never be zero (xorshift fixes 0 forever).
    state: u64,
}

impl Rng {
    /// Creates a generator from a seed. A zero seed is remapped to a fixed
    /// non-zero constant because an all-zero xorshift state stays zero
    /// forever.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Next raw 64-bit output (xorshift steps, then the `*` multiply that
    /// scrambles the weak low bits of plain xorshift).
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform `f64` in `[0, 1)`: the top 53 bits (an f64 mantissa's worth)
    /// scaled by 2^-53, the standard bit-exact conversion.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform `f64` in `[lo, hi)`.
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(123);
        let mut b = Rng::new(123);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn zero_seed_still_produces_output() {
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), 0, "zero state would be stuck forever");
    }

    #[test]
    fn next_f64_stays_in_unit_interval() {
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x), "out of range: {x}");
        }
    }

    #[test]
    fn range_f64_respects_bounds() {
        let mut rng = Rng::new(9);
        for _ in 0..1000 {
            let x = rng.range_f64(-1.0, 1.0);
            assert!((-1.0..1.0).contains(&x), "out of range: {x}");
        }
    }
}
