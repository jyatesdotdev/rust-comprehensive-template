//! Numerical computing: integration, root-finding, ODE solving, dense matrix.

/// Trapezoidal rule for ∫f(x)dx over [a, b] with `n` subdivisions.
pub fn integrate_trapezoidal(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    assert!(n > 0);
    let h = (b - a) / n as f64;
    let sum: f64 = (1..n).map(|i| f(a + i as f64 * h)).sum();
    h * (f(a) / 2.0 + sum + f(b) / 2.0)
}

/// Newton-Raphson root finding. Returns `Some(root)` if converged within `max_iter`,
/// `None` if it fails to converge or hits a zero/non-finite derivative.
pub fn newton_raphson(
    f: impl Fn(f64) -> f64,
    df: impl Fn(f64) -> f64,
    mut x: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    for _ in 0..max_iter {
        let dx = f(x) / df(x);
        // A zero derivative (or overflow) yields inf/NaN; bail out instead of
        // letting NaN silently poison the remaining iterations.
        if !dx.is_finite() {
            return None;
        }
        x -= dx;
        if dx.abs() < tol {
            return Some(x);
        }
    }
    None
}

/// Advance the ODE `dy/dt = f(t, y)` by one classical Runge-Kutta (RK4) step.
///
/// RK4 samples the derivative four times per step — at the start, twice at
/// the midpoint, and at the end — and blends them with weights 1:2:2:1.
/// That extra work buys a global error of O(dt⁴) versus Euler's O(dt), so at
/// the same step size RK4 is typically *orders of magnitude* more accurate
/// (see the `rk4_beats_euler_on_exponential_decay` test, which is the whole
/// lesson). Unlike velocity-Verlet in `physics.rs` it is not symplectic, so
/// for very long orbital runs Verlet still wins on energy drift.
pub fn rk4_step(f: impl Fn(f64, f64) -> f64, t: f64, y: f64, dt: f64) -> f64 {
    let k1 = f(t, y);
    let k2 = f(t + dt / 2.0, y + dt / 2.0 * k1);
    let k3 = f(t + dt / 2.0, y + dt / 2.0 * k2);
    let k4 = f(t + dt, y + dt * k3);
    y + dt / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)
}

/// Integrate `dy/dt = f(t, y)` from `(t0, y0)` over `steps` fixed RK4 steps
/// of size `dt`, returning the final `y`.
///
/// Fixed-step keeps the method transparent and deterministic; production
/// solvers add adaptive step-size control (e.g. RK45/Dormand-Prince) so the
/// step shrinks exactly where the solution demands it.
pub fn rk4(f: impl Fn(f64, f64) -> f64, t0: f64, y0: f64, dt: f64, steps: usize) -> f64 {
    let mut t = t0;
    let mut y = y0;
    for _ in 0..steps {
        y = rk4_step(&f, t, y, dt);
        t += dt;
    }
    y
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
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
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
    fn newton_zero_derivative_returns_none() {
        // f(x) = 1 has no root and df = 0 everywhere; must not loop on NaN.
        assert!(newton_raphson(|_| 1.0, |_| 0.0, 1.0, 1e-9, 50).is_none());
    }

    #[test]
    fn newton_no_convergence_returns_none() {
        // One iteration is not enough to reach √2 to 1e-12 from x = 1.
        assert!(newton_raphson(|x| x * x - 2.0, |x| 2.0 * x, 1.0, 1e-12, 1).is_none());
    }

    #[test]
    fn rk4_matches_analytic_exponential_decay() {
        // dy/dt = -y, y(0) = 1 → y(t) = e^{-t}.
        let y = rk4(|_, y| -y, 0.0, 1.0, 0.1, 30);
        assert!((y - (-3.0f64).exp()).abs() < 1e-6);
    }

    #[test]
    fn rk4_beats_euler_on_exponential_decay() {
        // The lesson: at the *same* step size, RK4's O(dt⁴) global error is
        // orders of magnitude below Euler's O(dt). Same ODE, same 30 steps.
        let exact = (-3.0f64).exp();
        let dt = 0.1_f64;
        let steps = 30;

        // Naive forward Euler: y ← y + dt·f(t, y).
        let mut y_euler = 1.0;
        let mut t = 0.0;
        for _ in 0..steps {
            y_euler += dt * -y_euler;
            t += dt;
        }
        assert!((t - 3.0).abs() < 1e-12);

        let y_rk4 = rk4(|_, y| -y, 0.0, 1.0, dt, steps);

        let err_euler = (y_euler - exact).abs();
        let err_rk4 = (y_rk4 - exact).abs();
        // Euler lands around 7e-3 off; RK4 around 2e-8 — better than three
        // orders of magnitude. Factor 1000 leaves deterministic headroom.
        assert!(
            err_rk4 * 1000.0 < err_euler,
            "RK4 error {err_rk4:e} not ≫ better than Euler error {err_euler:e}"
        );
    }

    #[test]
    fn rk4_time_dependent_rhs() {
        // dy/dt = t (independent of y) → y(t) = t²/2; RK4 is exact for
        // polynomials up to degree 4, so only rounding error remains.
        let y = rk4(|t, _| t, 0.0, 0.0, 0.25, 8);
        assert!((y - 2.0).abs() < 1e-12);
    }

    #[test]
    fn mat_multiply() {
        let a = Mat::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let b = Mat::new(2, 2, vec![5.0, 6.0, 7.0, 8.0]);
        let c = a.mul(&b);
        assert_eq!(c.data, vec![19.0, 22.0, 43.0, 50.0]);
    }
}
