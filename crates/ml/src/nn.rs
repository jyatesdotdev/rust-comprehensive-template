//! Neural-network building blocks composed from autograd [`Value`]s.
//!
//! Because every weight, bias, and activation is a graph node, a forward
//! pass *is* graph construction: `mlp.forward(&inputs)` leaves behind the
//! full computation graph, and calling `backward()` on the loss built from
//! its outputs yields gradients for every parameter. Nothing here knows
//! anything about differentiation — that separation (model defines the
//! function, autograd differentiates it) is exactly how candle/burn/tch
//! are structured, just with tensors instead of scalars.
//!
//! Everything is deterministically initialized from a caller-supplied seed
//! via [`Rng`], so tests and examples reproduce bit-for-bit.

use crate::autograd::Value;
use crate::rng::Rng;

/// A single neuron: `activation(w · x + b)`.
///
/// Weights and bias are leaf [`Value`]s created once and *shared* into every
/// forward pass's graph — that sharing is why gradients from a batch of
/// samples accumulate onto the same parameters.
pub struct Neuron {
    /// One weight per input, initialized uniformly in `[-1, 1)`.
    weights: Vec<Value>,
    /// Bias term, initialized uniformly in `[-1, 1)`.
    bias: Value,
    /// Whether to apply `tanh` to the pre-activation. Hidden layers say yes;
    /// the output layer stays linear so it can produce unbounded values.
    activated: bool,
}

impl Neuron {
    /// Creates a neuron with `inputs` weights drawn from `rng`.
    pub fn new(inputs: usize, activated: bool, rng: &mut Rng) -> Self {
        let weights = (0..inputs)
            .map(|_| Value::new(rng.range_f64(-1.0, 1.0)))
            .collect();
        let bias = Value::new(rng.range_f64(-1.0, 1.0));
        Self {
            weights,
            bias,
            activated,
        }
    }

    /// Weighted sum of `inputs` plus bias, then `tanh` if this neuron is
    /// activated. Extra inputs beyond the weight count are ignored;
    /// missing ones simply contribute nothing (callers are expected to
    /// match sizes — [`Mlp`] always does).
    pub fn forward(&self, inputs: &[Value]) -> Value {
        let mut sum = self.bias.clone();
        for (w, x) in self.weights.iter().zip(inputs) {
            sum = &sum + &(w * x);
        }
        if self.activated {
            sum.tanh()
        } else {
            sum
        }
    }

    /// All trainable parameters (weights then bias), as shared handles for
    /// the optimizer.
    pub fn parameters(&self) -> Vec<Value> {
        let mut params = self.weights.clone();
        params.push(self.bias.clone());
        params
    }
}

/// A fully-connected layer: independent [`Neuron`]s over the same inputs.
pub struct Layer {
    /// The neurons; one output per neuron.
    neurons: Vec<Neuron>,
}

impl Layer {
    /// Creates a layer mapping `inputs` values to `outputs` values.
    pub fn new(inputs: usize, outputs: usize, activated: bool, rng: &mut Rng) -> Self {
        let neurons = (0..outputs)
            .map(|_| Neuron::new(inputs, activated, rng))
            .collect();
        Self { neurons }
    }

    /// Applies every neuron to the same input slice.
    pub fn forward(&self, inputs: &[Value]) -> Vec<Value> {
        self.neurons.iter().map(|n| n.forward(inputs)).collect()
    }

    /// All trainable parameters of the layer, in neuron order.
    pub fn parameters(&self) -> Vec<Value> {
        self.neurons.iter().flat_map(Neuron::parameters).collect()
    }
}

/// A multi-layer perceptron: a stack of [`Layer`]s with `tanh` on every
/// hidden layer and a **linear** output layer.
///
/// ```
/// use ml::Mlp;
///
/// // 2 inputs -> 4 hidden (tanh) -> 1 linear output, seeded for determinism.
/// let mlp = Mlp::new(&[2, 4, 1], 42);
/// assert_eq!(mlp.parameters().len(), 4 * (2 + 1) + 1 * (4 + 1)); // 17
/// ```
pub struct Mlp {
    /// The layers, applied in order.
    layers: Vec<Layer>,
}

impl Mlp {
    /// Builds an MLP from layer sizes, e.g. `&[2, 4, 1]` = 2 inputs, one
    /// hidden layer of 4, one output. All weights come deterministically
    /// from `seed`. Fewer than two sizes yields an empty (identity) network.
    pub fn new(layer_sizes: &[usize], seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut layers = Vec::new();
        for window in layer_sizes.windows(2).enumerate() {
            let (index, pair) = window;
            let is_output_layer = index + 2 == layer_sizes.len();
            layers.push(Layer::new(pair[0], pair[1], !is_output_layer, &mut rng));
        }
        Self { layers }
    }

    /// Runs the forward pass, building the computation graph as it goes.
    pub fn forward(&self, inputs: &[Value]) -> Vec<Value> {
        let mut activations = inputs.to_vec();
        for layer in &self.layers {
            activations = layer.forward(&activations);
        }
        activations
    }

    /// Every trainable parameter of every layer — hand this to
    /// [`crate::optim::Sgd`].
    pub fn parameters(&self) -> Vec<Value> {
        self.layers.iter().flat_map(Layer::parameters).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neuron_parameter_count_is_weights_plus_bias() {
        let mut rng = Rng::new(1);
        let neuron = Neuron::new(3, true, &mut rng);
        assert_eq!(neuron.parameters().len(), 4);
    }

    #[test]
    fn activated_neuron_output_is_bounded_by_tanh() {
        let mut rng = Rng::new(2);
        let neuron = Neuron::new(2, true, &mut rng);
        let inputs = [Value::new(100.0), Value::new(100.0)];
        let out = neuron.forward(&inputs);
        assert!(out.data().abs() <= 1.0);
    }

    #[test]
    fn linear_neuron_matches_manual_dot_product() {
        let mut rng = Rng::new(3);
        let neuron = Neuron::new(2, false, &mut rng);
        let params = neuron.parameters();
        let (w0, w1, b) = (params[0].data(), params[1].data(), params[2].data());
        let inputs = [Value::new(0.5), Value::new(-2.0)];
        let out = neuron.forward(&inputs);
        let expected = w0 * 0.5 + w1 * -2.0 + b;
        assert!((out.data() - expected).abs() < 1e-12);
    }

    #[test]
    fn layer_produces_one_output_per_neuron() {
        let mut rng = Rng::new(4);
        let layer = Layer::new(3, 5, true, &mut rng);
        let inputs: Vec<Value> = (0..3).map(|i| Value::new(f64::from(i))).collect();
        assert_eq!(layer.forward(&inputs).len(), 5);
        assert_eq!(layer.parameters().len(), 5 * (3 + 1));
    }

    #[test]
    fn mlp_shape_and_parameter_count() {
        let mlp = Mlp::new(&[2, 4, 1], 42);
        assert_eq!(mlp.parameters().len(), 17);
        let inputs = [Value::new(1.0), Value::new(0.0)];
        let outputs = mlp.forward(&inputs);
        assert_eq!(outputs.len(), 1);
    }

    #[test]
    fn same_seed_gives_identical_networks() {
        let a = Mlp::new(&[2, 3, 1], 7);
        let b = Mlp::new(&[2, 3, 1], 7);
        let inputs = [Value::new(0.3), Value::new(-0.8)];
        assert_eq!(a.forward(&inputs)[0].data(), b.forward(&inputs)[0].data());
    }

    #[test]
    fn different_seeds_give_different_networks() {
        let a = Mlp::new(&[2, 3, 1], 7);
        let b = Mlp::new(&[2, 3, 1], 8);
        let inputs = [Value::new(0.3), Value::new(-0.8)];
        assert_ne!(a.forward(&inputs)[0].data(), b.forward(&inputs)[0].data());
    }

    #[test]
    fn degenerate_mlp_is_identity() {
        let mlp = Mlp::new(&[2], 1);
        let inputs = [Value::new(1.5), Value::new(-2.5)];
        let outputs = mlp.forward(&inputs);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].data(), 1.5);
        assert_eq!(outputs[1].data(), -2.5);
        assert!(mlp.parameters().is_empty());
    }

    #[test]
    fn backward_reaches_every_parameter() {
        let mlp = Mlp::new(&[2, 3, 1], 5);
        let inputs = [Value::new(0.5), Value::new(-0.5)];
        let out = &mlp.forward(&inputs)[0];
        out.backward();
        // With tanh activations and generic inputs, every weight influences
        // the output, so every gradient should be nonzero.
        for param in mlp.parameters() {
            assert_ne!(param.grad(), 0.0);
        }
    }
}
