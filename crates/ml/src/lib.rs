//! Machine learning from scratch: scalar reverse-mode autograd (micrograd
//! style) plus just enough neural-network machinery to train a real model
//! end to end — pure `std`, zero dependencies, no unsafe.
//!
//! # What scalar autograd teaches that tensor libraries hide
//!
//! In candle, burn, or tch, `loss.backward()` is a black box: gradients
//! appear on tensors and you take the chain rule on faith. Here every single
//! scalar is its own graph node ([`Value`]), so you can watch the whole
//! mechanism with no linear algebra in the way:
//!
//! - **A forward pass is graph construction.** Evaluating `w·x + b` *records*
//!   the expression; differentiation is just a second walk over that record.
//! - **Backprop is the chain rule plus a topological sort** — each node knows
//!   only its local derivative, and reverse order guarantees a node's
//!   gradient is complete before it hands shares to its inputs.
//! - **Gradients accumulate (`+=`)** because a shared node's gradient is the
//!   sum over all paths to the output — and that same fact is why optimizers
//!   must `zero_grad` between steps.
//!
//! The full loop lives in this crate's tests: build an [`Mlp`], compute
//! [`mse`] over its outputs, `backward()`, and let [`Sgd`] nudge 17
//! parameters until the network has learned XOR.
//!
//! # Module map
//!
//! | Module | Payload |
//! |---|---|
//! | [`autograd`] | [`Value`]: the computation graph and `backward()` |
//! | [`nn`] | [`Neuron`] / [`Layer`] / [`Mlp`] built from `Value`s |
//! | [`optim`] | [`Sgd`] with momentum |
//! | [`loss`] | [`mse`], [`binary_cross_entropy`] |
//! | [`rng`] | [`Rng`]: seeded xorshift64* for reproducible init |
//!
//! Production equivalents: [candle](https://github.com/huggingface/candle),
//! [burn](https://github.com/tracel-ai/burn), and
//! [tch](https://github.com/LaurentMazare/tch-rs) do exactly this over
//! tensors, with the graph taping and the `Rc<RefCell<...>>` bookkeeping
//! hidden behind tensor handles.

pub mod autograd;
pub mod loss;
pub mod nn;
pub mod optim;
pub mod rng;

pub use autograd::Value;
pub use loss::{binary_cross_entropy, mse};
pub use nn::{Layer, Mlp, Neuron};
pub use optim::Sgd;
pub use rng::Rng;

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end proof that all the pieces compose: a 2-4-1 MLP learns XOR,
    /// the canonical not-linearly-separable problem (a single neuron
    /// provably cannot solve it; one hidden layer can).
    ///
    /// Everything is deterministic — fixed seed, fixed epoch count, no
    /// wall-clock — so this either always passes or always fails.
    #[test]
    fn mlp_learns_xor_end_to_end() {
        let inputs = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
        let targets = [0.0, 1.0, 1.0, 0.0];

        let mlp = Mlp::new(&[2, 4, 1], 42);
        let params = mlp.parameters();
        let mut sgd = Sgd::new(0.1, 0.9);

        let mut final_loss = f64::MAX;
        for _ in 0..500 {
            // The three beats of every training loop:
            // zero_grad -> forward + backward -> step.
            sgd.zero_grad(&params);
            let predictions: Vec<Value> = inputs
                .iter()
                .map(|xy| {
                    let xs = [Value::new(xy[0]), Value::new(xy[1])];
                    mlp.forward(&xs).remove(0)
                })
                .collect();
            let loss = mse(&predictions, &targets);
            loss.backward();
            sgd.step(&params);
            final_loss = loss.data();
        }

        assert!(
            final_loss < 0.01,
            "XOR failed to converge: final loss {final_loss}"
        );

        // The trained network must classify all four cases correctly with a
        // 0.5 decision threshold.
        for (xy, target) in inputs.iter().zip(targets) {
            let xs = [Value::new(xy[0]), Value::new(xy[1])];
            let prediction = mlp.forward(&xs).remove(0).data();
            let class = if prediction > 0.5 { 1.0 } else { 0.0 };
            assert_eq!(
                class, target,
                "wrong class for {xy:?}: predicted {prediction}"
            );
        }
    }

    /// Same pipeline but with binary cross-entropy driving the gradients.
    /// BCE wants probabilities, so the linear output is squashed with a
    /// sigmoid *composed from the autograd primitives* — no dedicated
    /// sigmoid op needed. (Feeding raw linear outputs into BCE would let
    /// the clamp turn out-of-range predictions into constants and stall
    /// learning — the clamp is a numerical guard, not an activation.)
    #[test]
    fn xor_also_trains_with_bce() {
        let inputs = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
        let targets = [0.0, 1.0, 1.0, 0.0];

        // sigmoid(x) = 1 / (1 + e^(-x)), built from neg, exp, add, div.
        fn sigmoid(x: &Value) -> Value {
            let one = Value::new(1.0);
            &one / &(&one + &(-x).exp())
        }

        let mlp = Mlp::new(&[2, 4, 1], 42);
        let params = mlp.parameters();
        let mut sgd = Sgd::new(0.5, 0.9);

        let mut final_loss = f64::MAX;
        for _ in 0..500 {
            sgd.zero_grad(&params);
            let predictions: Vec<Value> = inputs
                .iter()
                .map(|xy| {
                    let xs = [Value::new(xy[0]), Value::new(xy[1])];
                    sigmoid(&mlp.forward(&xs).remove(0))
                })
                .collect();
            let loss = binary_cross_entropy(&predictions, &targets);
            loss.backward();
            sgd.step(&params);
            final_loss = loss.data();
        }

        assert!(
            final_loss < 0.05,
            "XOR (BCE) failed to converge: final loss {final_loss}"
        );
        for (xy, target) in inputs.iter().zip(targets) {
            let xs = [Value::new(xy[0]), Value::new(xy[1])];
            let prediction = sigmoid(&mlp.forward(&xs).remove(0)).data();
            let class = if prediction > 0.5 { 1.0 } else { 0.0 };
            assert_eq!(
                class, target,
                "wrong class for {xy:?}: predicted {prediction}"
            );
        }
    }
}
