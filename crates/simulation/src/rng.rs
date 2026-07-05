//! Deterministic pseudo-random numbers: PCG32 + Box-Muller normal sampling.
//!
//! Hand-rolled on purpose: teaching examples need *deterministic, seedable*
//! randomness with zero dependencies, so a fixed seed reproduces the exact
//! same simulation (and test) run on every machine. Production code should
//! use the `rand` / `rand_distr` crates instead — they provide audited
//! distributions, thread-local generators, and cryptographic options this
//! module deliberately does not attempt. PCG32 is **not** cryptographically
//! secure; never use it for keys, tokens, or anything adversarial.

use std::f64::consts::PI;

/// PCG32 multiplier from the reference implementation (O'Neill 2014).
const PCG_MULTIPLIER: u64 = 6364136223846793005;

/// Default stream (increment) constant from the PCG reference implementation.
const PCG_DEFAULT_STREAM: u64 = 1442695040888963407;

/// Scale factor mapping a 53-bit integer onto [0, 1): 2⁻⁵³.
const F64_SCALE: f64 = 1.0 / (1u64 << 53) as f64;

/// A PCG32 pseudo-random number generator (XSH-RR variant).
///
/// 64 bits of state, 32 bits of output per step. Small, fast, and passes
/// statistical test batteries far better than the classic LCGs it is built
/// on, because the output function scrambles the state before release.
#[derive(Debug, Clone)]
pub struct Pcg32 {
    /// Internal LCG state; advanced by one multiply-add per output.
    state: u64,
    /// Stream selector (must be odd — forced in the constructor).
    inc: u64,
    /// Cached second sample from the last Box-Muller transform, if unused.
    spare_normal: Option<f64>,
}

impl Pcg32 {
    /// Create a generator from a seed, using the default stream.
    ///
    /// The same seed always yields the same sequence — that determinism is
    /// the whole point of this module.
    pub fn new(seed: u64) -> Self {
        // Reference PCG seeding: absorb the seed into the state via two
        // advances so that nearby seeds diverge immediately.
        let mut rng = Self {
            state: 0,
            // `| 1` forces the increment odd, a requirement for the LCG to
            // achieve its full 2⁶⁴ period.
            inc: (PCG_DEFAULT_STREAM << 1) | 1,
            spare_normal: None,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Next 32 bits of output.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULTIPLIER).wrapping_add(self.inc);
        // XSH-RR output function: xorshift the high bits down, then rotate
        // by the top 5 bits. The rotation is what breaks the LCG's weak
        // low-bit patterns.
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Next 64 bits of output (two 32-bit draws glued together).
    pub fn next_u64(&mut self) -> u64 {
        let hi = u64::from(self.next_u32());
        let lo = u64::from(self.next_u32());
        (hi << 32) | lo
    }

    /// Uniform `f64` in `[0, 1)`.
    ///
    /// Uses the top 53 bits (an f64 mantissa holds exactly 53), so every
    /// representable output is equally likely and 1.0 is never produced.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * F64_SCALE
    }

    /// Uniform `f64` in `[min, max)`. Panics if `min > max`.
    pub fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        assert!(min <= max);
        min + (max - min) * self.next_f64()
    }

    /// Sample a normal (Gaussian) distribution via the Box-Muller transform.
    ///
    /// Box-Muller converts *two* uniform samples into *two* independent
    /// standard-normal samples. We return one and cache the spare so the
    /// next call is nearly free — discarding it would waste half the work
    /// and half the entropy drawn from the generator. The spare is cached
    /// as a *standard* normal and rescaled on use, so alternating calls
    /// with different `mean`/`std_dev` still get correct distributions.
    pub fn normal(&mut self, mean: f64, std_dev: f64) -> f64 {
        if let Some(z) = self.spare_normal.take() {
            return mean + std_dev * z;
        }
        // Map [0,1) to (0,1]: ln(0) is -inf, so u1 must never be zero.
        let u1 = 1.0 - self.next_f64();
        let u2 = self.next_f64();
        let radius = (-2.0 * u1.ln()).sqrt();
        let angle = 2.0 * PI * u2;
        self.spare_normal = Some(radius * angle.sin());
        mean + std_dev * radius * angle.cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Pcg32::new(42);
        let mut b = Pcg32::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Pcg32::new(1);
        let mut b = Pcg32::new(2);
        let seq_a: Vec<u32> = (0..16).map(|_| a.next_u32()).collect();
        let seq_b: Vec<u32> = (0..16).map(|_| b.next_u32()).collect();
        assert_ne!(seq_a, seq_b);
    }

    #[test]
    fn next_u64_is_deterministic() {
        let mut a = Pcg32::new(7);
        let mut b = Pcg32::new(7);
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn uniform_bounds_and_mean() {
        let mut rng = Pcg32::new(12345);
        let n = 10_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x), "sample {x} outside [0, 1)");
            sum += x;
        }
        let mean = sum / f64::from(n);
        // Std error of the mean for U[0,1) at n=10k is ~0.003; 0.02 is a
        // generous but non-trivial bound (and the seed is fixed anyway).
        assert!((mean - 0.5).abs() < 0.02, "mean drifted: {mean}");
    }

    #[test]
    fn range_f64_stays_in_bounds() {
        let mut rng = Pcg32::new(9);
        for _ in 0..1_000 {
            let x = rng.range_f64(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&x));
        }
    }

    #[test]
    fn range_f64_degenerate_interval() {
        let mut rng = Pcg32::new(9);
        let x = rng.range_f64(2.0, 2.0);
        assert!((x - 2.0).abs() < 1e-15);
    }

    #[test]
    fn normal_mean_and_std_dev() {
        let mut rng = Pcg32::new(2024);
        let n = 10_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.normal(5.0, 2.0)).collect();
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var = samples.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>()
            / (samples.len() - 1) as f64;
        let std_dev = var.sqrt();
        // Tolerances sized for n = 10k with a fixed seed: std error of the
        // mean is std/√n = 0.02, so 0.1 gives ample deterministic headroom.
        assert!((mean - 5.0).abs() < 0.1, "mean drifted: {mean}");
        assert!((std_dev - 2.0).abs() < 0.1, "std_dev drifted: {std_dev}");
    }

    #[test]
    fn normal_spare_sample_is_deterministic() {
        // Two generators with the same seed must agree even across the
        // cached-spare path (odd and even call counts).
        let mut a = Pcg32::new(11);
        let mut b = Pcg32::new(11);
        for _ in 0..5 {
            assert!((a.normal(0.0, 1.0) - b.normal(0.0, 1.0)).abs() < 1e-15);
        }
    }

    #[test]
    fn normal_spare_rescales_per_call() {
        // The cached spare is a *standard* normal; a second call with a huge
        // mean must reflect that call's parameters, not the first call's.
        let mut rng = Pcg32::new(3);
        let _first = rng.normal(0.0, 1.0);
        let second = rng.normal(1_000.0, 1.0);
        assert!((second - 1_000.0).abs() < 10.0);
    }
}
