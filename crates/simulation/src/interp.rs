//! Interpolation: lerp/remap, Catmull-Rom splines, sorted lookup tables.
//!
//! Interpolation converts sparse samples into a continuous function — the
//! bread and butter of animation curves, physics tables, and unit
//! conversion. The recurring design decision here is what to do *outside*
//! the sampled range: this module clamps rather than extrapolates, and the
//! doc on [`LookupTable::sample`] explains why that is the safe default.

/// Linear interpolation: `a` at `t = 0`, `b` at `t = 1`.
///
/// `t` is not clamped — values outside `[0, 1]` extrapolate linearly, which
/// is occasionally what you want from a bare lerp (and trivially clamped by
/// the caller when it is not).
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Inverse of [`lerp`]: recover `t` such that `lerp(a, b, t) == v`.
///
/// Returns `None` when `a == b`: every `v` (or no `v`) would satisfy the
/// equation, so there is no meaningful parameter to return — the division
/// would produce `±inf`/`NaN` and poison downstream math.
pub fn inverse_lerp(a: f64, b: f64, v: f64) -> Option<f64> {
    if a == b {
        return None;
    }
    Some((v - a) / (b - a))
}

/// Map `v` from range `[in_a, in_b]` to range `[out_a, out_b]`.
///
/// Composition of [`inverse_lerp`] and [`lerp`]; returns `None` when the
/// input range is degenerate (`in_a == in_b`). Like `lerp`, values outside
/// the input range extrapolate.
pub fn remap(v: f64, in_a: f64, in_b: f64, out_a: f64, out_b: f64) -> Option<f64> {
    let t = inverse_lerp(in_a, in_b, v)?;
    Some(lerp(out_a, out_b, t))
}

/// Evaluate a uniform Catmull-Rom spline segment at `t` in `[0, 1]`.
///
/// The curve runs from `p1` (at `t = 0`) to `p2` (at `t = 1`); `p0` and `p3`
/// are neighboring control points that only shape the tangents. Catmull-Rom
/// is the workhorse for smooth paths through waypoints precisely because it
/// *passes through* its control points, unlike Bézier or B-splines whose
/// curves only approach theirs. Tangent at `p1` is `(p2 - p0) / 2`.
pub fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    // Standard expanded polynomial form (see e.g. Catmull & Rom 1974):
    // 0.5 · (2p1 + (p2-p0)t + (2p0-5p1+4p2-p3)t² + (3p1-p0-3p2+p3)t³)
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// A piecewise-linear lookup table over sorted knots.
///
/// Stores `(x, y)` pairs with strictly increasing `x` and answers
/// `sample(x)` by linear interpolation between the two bracketing knots.
/// This is the classic representation for measured curves (thrust vs. RPM,
/// gamma ramps, easing tables) where no closed form exists.
#[derive(Debug, Clone)]
pub struct LookupTable {
    /// Knot x-coordinates, strictly increasing (validated in `new`).
    xs: Vec<f64>,
    /// Knot y-values, one per x.
    ys: Vec<f64>,
}

impl LookupTable {
    /// Build a table from knot coordinates.
    ///
    /// Returns `None` unless `xs` and `ys` have the same non-zero length and
    /// `xs` is strictly increasing — the sampling code binary-searches `xs`,
    /// and that invariant is what makes the search (and the bracketing
    /// arithmetic) correct. Validating once at construction keeps `sample`
    /// infallible.
    pub fn new(xs: Vec<f64>, ys: Vec<f64>) -> Option<Self> {
        if xs.is_empty() || xs.len() != ys.len() {
            return None;
        }
        if xs.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        Some(Self { xs, ys })
    }

    /// Sample the table at `x`, clamping outside the knot range.
    ///
    /// Queries below the first knot return the first `y`; queries above the
    /// last knot return the last `y`. Clamping beats extrapolation as the
    /// safe default: a lookup table is only trustworthy where it was
    /// measured, and linear extrapolation from the outermost segment turns
    /// a slightly out-of-range query into an unbounded, data-free guess
    /// (a thrust table extrapolated past max RPM predicts thrust that the
    /// motor cannot produce). Callers who truly want extrapolation can do
    /// it explicitly with [`remap`] on the edge knots.
    pub fn sample(&self, x: f64) -> f64 {
        // Invariants from `new`: xs and ys are non-empty, equal length, and
        // xs is strictly increasing — so the indexing below cannot go out
        // of bounds and every interior x has a bracketing pair.
        let last = self.xs.len() - 1;
        if x <= self.xs[0] {
            return self.ys[0];
        }
        if x >= self.xs[last] {
            return self.ys[last];
        }
        // Index of the first knot strictly greater than x; x is interior,
        // so 1 <= hi <= last and the knot below it exists.
        let hi = self.xs.partition_point(|&knot| knot <= x);
        let lo = hi - 1;
        let t = (x - self.xs[lo]) / (self.xs[hi] - self.xs[lo]);
        lerp(self.ys[lo], self.ys[hi], t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn lerp_endpoints_and_midpoint() {
        assert!((lerp(2.0, 6.0, 0.0) - 2.0).abs() < TOL);
        assert!((lerp(2.0, 6.0, 1.0) - 6.0).abs() < TOL);
        assert!((lerp(2.0, 6.0, 0.5) - 4.0).abs() < TOL);
    }

    #[test]
    fn inverse_lerp_roundtrip() {
        let t = inverse_lerp(2.0, 6.0, 5.0).unwrap();
        assert!((t - 0.75).abs() < TOL);
        assert!((lerp(2.0, 6.0, t) - 5.0).abs() < TOL);
    }

    #[test]
    fn inverse_lerp_degenerate_is_none() {
        assert!(inverse_lerp(3.0, 3.0, 3.0).is_none());
    }

    #[test]
    fn remap_ranges() {
        // 50 on [0,100] → 0.5 → [-1,1] gives 0.
        let v = remap(50.0, 0.0, 100.0, -1.0, 1.0).unwrap();
        assert!(v.abs() < TOL);
        assert!(remap(1.0, 2.0, 2.0, 0.0, 1.0).is_none());
    }

    #[test]
    fn catmull_rom_passes_through_control_points() {
        let (p0, p1, p2, p3) = (0.0, 1.0, 4.0, 9.0);
        assert!((catmull_rom(p0, p1, p2, p3, 0.0) - p1).abs() < TOL);
        assert!((catmull_rom(p0, p1, p2, p3, 1.0) - p2).abs() < TOL);
    }

    #[test]
    fn catmull_rom_collinear_points_stay_linear() {
        // Control points on the line y = 2x: the spline must reproduce it.
        let mid = catmull_rom(0.0, 2.0, 4.0, 6.0, 0.5);
        assert!((mid - 3.0).abs() < TOL);
    }

    #[test]
    fn table_rejects_bad_input() {
        assert!(LookupTable::new(vec![], vec![]).is_none());
        assert!(LookupTable::new(vec![1.0, 2.0], vec![1.0]).is_none());
        // Not strictly increasing (equal and decreasing knots).
        assert!(LookupTable::new(vec![1.0, 1.0], vec![0.0, 1.0]).is_none());
        assert!(LookupTable::new(vec![2.0, 1.0], vec![0.0, 1.0]).is_none());
    }

    #[test]
    fn table_hits_knots_exactly() {
        let t = LookupTable::new(vec![0.0, 1.0, 3.0], vec![10.0, 20.0, 0.0]).unwrap();
        assert!((t.sample(0.0) - 10.0).abs() < TOL);
        assert!((t.sample(1.0) - 20.0).abs() < TOL);
        assert!((t.sample(3.0) - 0.0).abs() < TOL);
    }

    #[test]
    fn table_interpolates_between_knots() {
        let t = LookupTable::new(vec![0.0, 1.0, 3.0], vec![10.0, 20.0, 0.0]).unwrap();
        assert!((t.sample(0.5) - 15.0).abs() < TOL);
        assert!((t.sample(2.0) - 10.0).abs() < TOL);
    }

    #[test]
    fn table_clamps_out_of_range() {
        let t = LookupTable::new(vec![0.0, 1.0], vec![10.0, 20.0]).unwrap();
        assert!((t.sample(-100.0) - 10.0).abs() < TOL);
        assert!((t.sample(100.0) - 20.0).abs() < TOL);
    }

    #[test]
    fn single_knot_table_is_constant() {
        let t = LookupTable::new(vec![5.0], vec![42.0]).unwrap();
        assert!((t.sample(0.0) - 42.0).abs() < TOL);
        assert!((t.sample(5.0) - 42.0).abs() < TOL);
        assert!((t.sample(9.0) - 42.0).abs() < TOL);
    }
}
