//! Loss functions built as autograd expressions.
//!
//! A loss here is just another [`Value`] at the tip of the computation
//! graph: calling `backward()` on it propagates gradients through the loss
//! math, through the network, and into the parameters — no special casing.
//! Targets are plain `f64`s (constants), because we never differentiate with
//! respect to the labels.

use crate::autograd::Value;

/// How far predictions are clamped away from 0 and 1 in
/// [`binary_cross_entropy`].
///
/// Why clamp at all: BCE takes `ln(p)` and `ln(1 − p)`. At `p = 0` (or 1)
/// that is `ln(0) = −inf`, and its gradient `1/p` is infinite. One such node
/// poisons *every* gradient upstream of it — after one `backward()` the whole
/// parameter set is NaN/inf and training is unrecoverable. Clamping to
/// `[ε, 1 − ε]` caps the loss and its gradient at large-but-finite values.
/// Every production framework does the same (e.g. PyTorch's
/// `binary_cross_entropy` clamps its log outputs).
const BCE_EPSILON: f64 = 1e-7;

/// Mean squared error: `mean((prediction − target)²)`.
///
/// The workhorse regression loss; also fine for tiny classification demos
/// like XOR. Pairs `predictions` with `targets` positionally; callers should
/// pass equal lengths (extras are ignored). Empty input yields a constant 0.
///
/// ```
/// use ml::{mse, Value};
///
/// let preds = [Value::new(1.0), Value::new(3.0)];
/// let loss = mse(&preds, &[0.0, 5.0]); // (1 + 4) / 2
/// assert_eq!(loss.data(), 2.5);
/// ```
pub fn mse(predictions: &[Value], targets: &[f64]) -> Value {
    let count = predictions.len().min(targets.len());
    if count == 0 {
        return Value::new(0.0);
    }
    let mut total = Value::new(0.0);
    for (prediction, target) in predictions.iter().zip(targets) {
        let diff = prediction - &Value::new(*target);
        total = &total + &diff.powf(2.0);
    }
    &total * &Value::new(1.0 / count as f64)
}

/// Binary cross-entropy: `mean(−[t·ln(p) + (1 − t)·ln(1 − p)])`.
///
/// The right loss when predictions are probabilities and targets are 0/1
/// labels. Predictions are clamped to `[ε, 1 − ε]` first — see
/// [`BCE_EPSILON`] for why an unclamped `ln(0)` destroys training. Pairs
/// positionally like [`mse`]; empty input yields a constant 0.
pub fn binary_cross_entropy(predictions: &[Value], targets: &[f64]) -> Value {
    let count = predictions.len().min(targets.len());
    if count == 0 {
        return Value::new(0.0);
    }
    let one = Value::new(1.0);
    let mut total = Value::new(0.0);
    for (prediction, target) in predictions.iter().zip(targets) {
        let p = clamp_probability(prediction);
        let ln_p = p.ln();
        let ln_one_minus_p = (&one - &p).ln();
        let likelihood =
            &(&Value::new(*target) * &ln_p) + &(&Value::new(1.0 - *target) * &ln_one_minus_p);
        total = &total - &likelihood;
    }
    &total * &Value::new(1.0 / count as f64)
}

/// Clamps a predicted probability into `[ε, 1 − ε]`.
///
/// A hard clamp has derivative 1 inside the range and 0 outside, so:
/// inside we pass the original node through untouched (gradient flows),
/// and outside we substitute a fresh *constant* leaf — gradient stops, which
/// is exactly the derivative of a saturated clamp. Because this scalar
/// engine evaluates eagerly, we can make that choice by simply inspecting
/// `data()` at graph-build time.
fn clamp_probability(prediction: &Value) -> Value {
    let p = prediction.data();
    if p < BCE_EPSILON {
        Value::new(BCE_EPSILON)
    } else if p > 1.0 - BCE_EPSILON {
        Value::new(1.0 - BCE_EPSILON)
    } else {
        prediction.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mse_known_value() {
        let preds = [Value::new(2.0), Value::new(-1.0)];
        let loss = mse(&preds, &[0.0, 1.0]);
        // ((2)^2 + (-2)^2) / 2 = 4
        assert!((loss.data() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn mse_of_perfect_prediction_is_zero_with_zero_grad() {
        let pred = Value::new(0.7);
        let loss = mse(std::slice::from_ref(&pred), &[0.7]);
        loss.backward();
        assert!(loss.data().abs() < 1e-12);
        assert!(pred.grad().abs() < 1e-12);
    }

    #[test]
    fn mse_gradient_matches_hand_derivation() {
        // loss = (p − t)^2, d/dp = 2(p − t)
        let pred = Value::new(1.5);
        let loss = mse(std::slice::from_ref(&pred), &[1.0]);
        loss.backward();
        assert!((pred.grad() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn mse_empty_input_is_zero() {
        assert_eq!(mse(&[], &[]).data(), 0.0);
        assert_eq!(binary_cross_entropy(&[], &[]).data(), 0.0);
    }

    #[test]
    fn bce_known_value() {
        // p = 0.8, t = 1: loss = −ln(0.8)
        let pred = Value::new(0.8);
        let loss = binary_cross_entropy(&[pred], &[1.0]);
        assert!((loss.data() - (-(0.8f64.ln()))).abs() < 1e-9);
    }

    #[test]
    fn bce_gradient_matches_hand_derivation() {
        // For t = 1: loss = −ln(p), d/dp = −1/p.
        let pred = Value::new(0.25);
        let loss = binary_cross_entropy(std::slice::from_ref(&pred), &[1.0]);
        loss.backward();
        assert!((pred.grad() - (-4.0)).abs() < 1e-9);
    }

    #[test]
    fn bce_clamps_confident_wrong_prediction_to_finite_loss() {
        // p = 0 with t = 1 would be ln(0) = −inf without the clamp.
        let pred = Value::new(0.0);
        let loss = binary_cross_entropy(std::slice::from_ref(&pred), &[1.0]);
        assert!(loss.data().is_finite());
        loss.backward();
        // The clamp substitutes a constant, so the (infinite) gradient is
        // cut off rather than poisoning the graph.
        assert!(pred.grad().is_finite());
    }

    #[test]
    fn bce_clamps_at_the_upper_end_too() {
        // p = 1 with t = 0 is the mirror-image failure: ln(1 − p) = ln(0).
        let pred = Value::new(1.0);
        let loss = binary_cross_entropy(std::slice::from_ref(&pred), &[0.0]);
        assert!(loss.data().is_finite());
        loss.backward();
        assert!(pred.grad().is_finite());
    }

    #[test]
    fn bce_averages_over_samples() {
        let preds = [Value::new(0.9), Value::new(0.1)];
        let loss = binary_cross_entropy(&preds, &[1.0, 0.0]);
        let expected = (-(0.9f64.ln()) - (0.9f64.ln())) / 2.0;
        assert!((loss.data() - expected).abs() < 1e-9);
    }
}
