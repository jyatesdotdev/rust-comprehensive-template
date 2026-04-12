//! Numerical computing: integration, root-finding, dense matrix.

/// Trapezoidal rule for ∫f(x)dx over [a, b] with `n` subdivisions.
pub fn integrate_trapezoidal(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    assert!(n > 0);
    let h = (b - a) / n as f64;
    let sum: f64 = (1..n).map(|i| f(a + i as f64 * h)).sum();
    h * (f(a) / 2.0 + sum + f(b) / 2.0)
}

/// Newton-Raphson root finding. Returns `Some(root)` if converged within `max_iter`.
pub fn newton_raphson(
    f: impl Fn(f64) -> f64,
    df: impl Fn(f64) -> f64,
    mut x: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    for _ in 0..max_iter {
        let dx = f(x) / df(x);
        x -= dx;
        if dx.abs() < tol {
            return Some(x);
        }
    }
    None
}

/// Row-major dense matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct Mat {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row-major element storage (`rows × cols` length).
    pub data: Vec<f64>,
}

impl Mat {
    /// Create a matrix from pre-existing data. Panics if `data.len() != rows * cols`.
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        assert_eq!(data.len(), rows * cols);
        Self { rows, cols, data }
    }

    /// Create a zero-filled matrix of the given dimensions.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self { rows, cols, data: vec![0.0; rows * cols] }
    }

    /// Get the element at row `r`, column `c`.
    pub fn get(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }

    /// Set the element at row `r`, column `c` to `v`.
    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[r * self.cols + c] = v;
    }

    /// Matrix multiply: self (m×k) * rhs (k×n) → (m×n).
    pub fn mul(&self, rhs: &Mat) -> Mat {
        assert_eq!(self.cols, rhs.rows);
        let mut out = Mat::zeros(self.rows, rhs.cols);
        for i in 0..self.rows {
            for j in 0..rhs.cols {
                let mut s = 0.0;
                for k in 0..self.cols {
                    s += self.get(i, k) * rhs.get(k, j);
                }
                out.set(i, j, s);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoidal_x_squared() {
        // ∫₀¹ x² dx = 1/3
        let result = integrate_trapezoidal(|x| x * x, 0.0, 1.0, 10_000);
        assert!((result - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn newton_sqrt2() {
        // x² - 2 = 0 → x = √2
        let root = newton_raphson(|x| x * x - 2.0, |x| 2.0 * x, 1.0, 1e-12, 100).unwrap();
        assert!((root - std::f64::consts::SQRT_2).abs() < 1e-10);
    }

    #[test]
    fn mat_multiply() {
        let a = Mat::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let b = Mat::new(2, 2, vec![5.0, 6.0, 7.0, 8.0]);
        let c = a.mul(&b);
        assert_eq!(c.data, vec![19.0, 22.0, 43.0, 50.0]);
    }
}
