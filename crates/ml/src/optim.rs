//! Stochastic gradient descent with classical momentum.
//!
//! The optimizer is deliberately dumb: it neither builds graphs nor computes
//! gradients. It just reads `param.grad()` (filled in by `backward()`) and
//! nudges `param.data()` the other way via [`Value::adjust`]. The training
//! loop contract is always the same three beats:
//!
//! 1. `optimizer.zero_grad(&params)` — because gradients *accumulate*
//! 2. build loss, `loss.backward()`
//! 3. `optimizer.step(&params)`
//!
//! Skipping step 1 sums gradients from every previous epoch and training
//! quietly diverges — the classic autograd footgun (PyTorch users know it as
//! the forgotten `optimizer.zero_grad()`).

use crate::autograd::Value;

/// SGD with momentum: `v ← momentum·v − lr·grad; param ← param + v`.
///
/// Momentum keeps a running velocity per parameter, which smooths the
/// zig-zag of raw SGD and powers through small flat regions — for tiny
/// networks it often makes the difference between converging in hundreds
/// versus thousands of epochs. `momentum = 0.0` is plain SGD.
pub struct Sgd {
    /// Learning rate: how far to move along the (negative) gradient.
    pub lr: f64,
    /// Momentum coefficient in `[0, 1)`; fraction of last step's velocity
    /// carried into this step. `0.9` is the traditional default.
    pub momentum: f64,
    /// Per-parameter velocity, lazily sized on the first `step` call.
    velocity: Vec<f64>,
}

impl Sgd {
    /// Creates an optimizer. Velocities start at zero.
    pub fn new(lr: f64, momentum: f64) -> Self {
        Self {
            lr,
            momentum,
            velocity: Vec::new(),
        }
    }

    /// Applies one update to every parameter from its current gradient.
    ///
    /// Callers must pass the same parameter slice (same order) every step —
    /// velocities are matched to parameters by index.
    pub fn step(&mut self, params: &[Value]) {
        if self.velocity.len() != params.len() {
            self.velocity = vec![0.0; params.len()];
        }
        for (v, param) in self.velocity.iter_mut().zip(params) {
            *v = self.momentum * *v - self.lr * param.grad();
            param.adjust(*v);
        }
    }

    /// Zeroes every parameter's gradient. Call before each `backward()` —
    /// see the module docs for why this is not optional.
    pub fn zero_grad(&self, params: &[Value]) {
        for param in params {
            param.zero_grad();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_moves_parameter_against_gradient() {
        let param = Value::new(1.0);
        // Loss = param^2, so d(loss)/d(param) = 2 at param = 1.
        let loss = param.powf(2.0);
        loss.backward();

        let mut sgd = Sgd::new(0.1, 0.0);
        sgd.step(std::slice::from_ref(&param));
        // param ← 1.0 − 0.1 · 2.0 = 0.8
        assert!((param.data() - 0.8).abs() < 1e-12);
    }

    #[test]
    fn momentum_carries_velocity_between_steps() {
        let param = Value::new(0.0);
        let mut sgd = Sgd::new(0.1, 0.5);

        // Fake a constant gradient of 1.0 across two steps.
        let g = &param * &Value::new(1.0);
        g.backward(); // param.grad = 1
        sgd.step(std::slice::from_ref(&param)); // v = -0.1, param = -0.1

        sgd.zero_grad(std::slice::from_ref(&param));
        let g = &param * &Value::new(1.0);
        g.backward(); // param.grad = 1 again
        sgd.step(std::slice::from_ref(&param)); // v = 0.5·(-0.1) − 0.1 = -0.15

        assert!((param.data() - (-0.25)).abs() < 1e-12);
    }

    #[test]
    fn zero_grad_resets_all_parameters() {
        let a = Value::new(2.0);
        let b = Value::new(3.0);
        let loss = &a * &b;
        loss.backward();
        assert_ne!(a.grad(), 0.0);
        assert_ne!(b.grad(), 0.0);

        let sgd = Sgd::new(0.1, 0.0);
        sgd.zero_grad(&[a.clone(), b.clone()]);
        assert_eq!(a.grad(), 0.0);
        assert_eq!(b.grad(), 0.0);
    }

    #[test]
    fn repeated_steps_descend_a_quadratic() {
        // Minimize (param − 3)^2; SGD should approach 3.
        let param = Value::new(0.0);
        let mut sgd = Sgd::new(0.1, 0.9);
        let params = [param.clone()];
        // Heavy momentum overshoots and rings around the minimum, so give it
        // enough steps for the oscillation to damp out.
        for _ in 0..300 {
            sgd.zero_grad(&params);
            let loss = (&param - &Value::new(3.0)).powf(2.0);
            loss.backward();
            sgd.step(&params);
        }
        assert!((param.data() - 3.0).abs() < 1e-3, "got {}", param.data());
    }
}
