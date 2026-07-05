//! Descriptive statistics: mean, variance, median, percentiles, correlation.
//!
//! The error-handling lesson of this module: every function returns `Option`
//! and yields `None` for inputs it cannot answer (empty slices, too few
//! samples, mismatched lengths, zero variance) instead of panicking or —
//! worse — silently returning `NaN`. A `NaN` propagates through downstream
//! arithmetic and surfaces far from its cause; a `None` forces the caller to
//! decide *at the call site* what an undefined statistic should mean.

/// Arithmetic mean. Returns `None` for an empty slice.
pub fn mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    Some(data.iter().sum::<f64>() / data.len() as f64)
}

/// Sample variance with Bessel's correction (divide by `n - 1`, not `n`).
///
/// Why `n - 1`: the deviations are measured from the *sample* mean, which
/// was itself fit to the data, consuming one degree of freedom. Dividing by
/// `n` would systematically underestimate the population variance; `n - 1`
/// makes the estimator unbiased. Returns `None` for fewer than two samples —
/// one point has no spread to estimate.
pub fn variance(data: &[f64]) -> Option<f64> {
    if data.len() < 2 {
        return None;
    }
    let m = mean(data)?;
    let sum_sq: f64 = data.iter().map(|x| (x - m) * (x - m)).sum();
    Some(sum_sq / (data.len() - 1) as f64)
}

/// Sample standard deviation (square root of the sample [`variance`]).
///
/// Returns `None` for fewer than two samples.
pub fn std_dev(data: &[f64]) -> Option<f64> {
    variance(data).map(f64::sqrt)
}

/// Median: the 50th [`percentile`]. Returns `None` for an empty slice.
///
/// For even lengths this is the average of the two middle values (linear
/// interpolation lands exactly halfway between them).
pub fn median(data: &[f64]) -> Option<f64> {
    percentile(data, 50.0)
}

/// Percentile `p` in `[0, 100]` with linear interpolation between ranks.
///
/// Rank `p/100 · (n-1)` rarely lands on an integer, so the value is
/// interpolated between the two neighboring order statistics — the same
/// convention as NumPy's default. `p = 0` gives the minimum, `p = 100` the
/// maximum. Returns `None` for an empty slice or `p` outside `[0, 100]`.
///
/// Sorts a copy of the input: O(n log n) time and O(n) space. That is the
/// deliberate trade-off — selection algorithms reach O(n), but a sort-copy
/// is simple, leaves the caller's data untouched, and n log n is rarely the
/// bottleneck at the data sizes where a `Vec<f64>` of samples is the model.
pub fn percentile(data: &[f64], p: f64) -> Option<f64> {
    if data.is_empty() || !(0.0..=100.0).contains(&p) {
        return None;
    }
    let mut sorted = data.to_vec();
    // total_cmp gives a total order over floats (NaN sorts last) so the
    // sort cannot panic on incomparable values.
    sorted.sort_by(f64::total_cmp);
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * frac)
}

/// Sample covariance of two equal-length series (Bessel-corrected, `n - 1`).
///
/// Returns `None` if the lengths differ or there are fewer than two pairs.
pub fn covariance(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let mx = mean(x)?;
    let my = mean(y)?;
    let sum: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    Some(sum / (x.len() - 1) as f64)
}

/// Pearson correlation coefficient in `[-1, 1]`.
///
/// Covariance normalized by both standard deviations, so it measures linear
/// association independent of scale. Returns `None` if lengths differ, there
/// are fewer than two pairs, or either series is constant — correlation with
/// a zero-variance series is 0/0, which is undefined, not zero.
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    let cov = covariance(x, y)?;
    let sx = std_dev(x)?;
    let sy = std_dev(y)?;
    if sx == 0.0 || sy == 0.0 {
        return None;
    }
    Some(cov / (sx * sy))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn mean_hand_computed() {
        assert!((mean(&[1.0, 2.0, 3.0, 4.0]).unwrap() - 2.5).abs() < TOL);
    }

    #[test]
    fn variance_hand_computed() {
        // Deviations from mean 2.5: ±1.5, ±0.5 → sum of squares = 5,
        // divided by n-1 = 3 → 5/3.
        let v = variance(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert!((v - 5.0 / 3.0).abs() < TOL);
    }

    #[test]
    fn std_dev_hand_computed() {
        let s = std_dev(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert!((s - (5.0f64 / 3.0).sqrt()).abs() < TOL);
    }

    #[test]
    fn median_odd_length() {
        assert!((median(&[5.0, 1.0, 3.0]).unwrap() - 3.0).abs() < TOL);
    }

    #[test]
    fn median_even_length() {
        // Sorted: 1 2 3 4 → average of middle two = 2.5.
        assert!((median(&[4.0, 1.0, 3.0, 2.0]).unwrap() - 2.5).abs() < TOL);
    }

    #[test]
    fn percentile_edges_and_interpolation() {
        let data = [10.0, 20.0, 30.0, 40.0];
        assert!((percentile(&data, 0.0).unwrap() - 10.0).abs() < TOL);
        assert!((percentile(&data, 100.0).unwrap() - 40.0).abs() < TOL);
        // Rank at p=25 is 0.25 * 3 = 0.75 → 10 + 0.75·(20-10) = 17.5.
        assert!((percentile(&data, 25.0).unwrap() - 17.5).abs() < TOL);
    }

    #[test]
    fn percentile_out_of_range_is_none() {
        let data = [1.0, 2.0];
        assert!(percentile(&data, -0.1).is_none());
        assert!(percentile(&data, 100.1).is_none());
        assert!(percentile(&data, f64::NAN).is_none());
    }

    #[test]
    fn covariance_hand_computed() {
        // x deviations: -1, 0, 1; y = 2x deviations: -2, 0, 2.
        // Σ products = 4, / (n-1) = 2.
        let c = covariance(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap();
        assert!((c - 2.0).abs() < TOL);
    }

    #[test]
    fn correlation_of_linear_data() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let up: Vec<f64> = x.iter().map(|v| 3.0 * v + 1.0).collect();
        let down: Vec<f64> = x.iter().map(|v| -2.0 * v + 7.0).collect();
        assert!((pearson_correlation(&x, &up).unwrap() - 1.0).abs() < 1e-10);
        assert!((pearson_correlation(&x, &down).unwrap() + 1.0).abs() < 1e-10);
    }

    #[test]
    fn constant_series_correlation_is_none() {
        let x = [1.0, 2.0, 3.0];
        let flat = [4.0, 4.0, 4.0];
        assert!(pearson_correlation(&x, &flat).is_none());
    }

    #[test]
    fn mismatched_lengths_are_none() {
        assert!(covariance(&[1.0, 2.0], &[1.0]).is_none());
        assert!(pearson_correlation(&[1.0, 2.0], &[1.0]).is_none());
    }

    #[test]
    fn empty_and_short_inputs_are_none() {
        assert!(mean(&[]).is_none());
        assert!(variance(&[]).is_none());
        assert!(variance(&[1.0]).is_none());
        assert!(std_dev(&[]).is_none());
        assert!(median(&[]).is_none());
        assert!(percentile(&[], 50.0).is_none());
        assert!(covariance(&[], &[]).is_none());
        assert!(pearson_correlation(&[], &[]).is_none());
        assert!(pearson_correlation(&[1.0], &[1.0]).is_none());
    }
}
