//! Scalar reverse-mode automatic differentiation (micrograd-style).
//!
//! A [`Value`] is one node in a *computation graph*: it stores the scalar it
//! evaluated to (`data`), the derivative of the final output with respect to
//! it (`grad`), and which operation produced it from which inputs. Building an
//! expression out of `Value`s therefore records, as a side effect, everything
//! needed to differentiate it — that recording is the entire trick behind
//! every deep-learning framework.
//!
//! # Why `Rc<RefCell<...>>`
//!
//! The graph is a **DAG, not a tree**: one node can feed several consumers
//! (`let b = &a + &a;` — both operands are the *same* node). That rules out
//! single ownership (`Box`) and plain references (the graph outlives any one
//! stack frame), so nodes are shared via `Rc`. The backward pass then needs to
//! *mutate* every node's `grad` while walking the shared graph, which is
//! exactly what `RefCell`'s interior mutability provides. This
//! `Rc<RefCell<Node>>` shape is the standard Rust answer to "shared mutable
//! graph without unsafe"; production tensor libraries (candle, burn, tch) hide
//! the same bookkeeping behind tensor handles.
//!
//! # Why gradients *accumulate* (`+=`) instead of assign (`=`)
//!
//! In a diamond-shaped graph a node contributes to the output along several
//! paths, and the multivariate chain rule says its gradient is the **sum** of
//! the contributions from each path. Concretely, for `b = a + a` we must end
//! with `a.grad() == 2`: each `+` operand deposits `1`. If `backprop` used
//! `=`, the second deposit would overwrite the first and silently produce
//! `1` — the single most common bug in hand-rolled autograd. The flip side of
//! `+=` is that gradients from *successive* `backward()` calls also add up,
//! which is why an optimizer must call [`Value::zero_grad`] on the parameters
//! between steps.
//!
//! # Why the graph cannot cycle (and so `Rc` cannot leak)
//!
//! `Rc` leaks memory if you create a reference cycle. Here every operation is
//! a *builder*: it allocates a **fresh** node whose children are the (already
//! existing) operands, and nothing ever mutates a node's operand list
//! afterwards. A node can therefore only point at nodes created strictly
//! before it, which makes a cycle impossible by construction — the graph
//! stays a DAG and every node is freed when the expression is dropped.
//!
//! # Why the `RefCell` borrows cannot panic
//!
//! `RefCell` panics only on *reentrant* conflicting borrows. Every method
//! here either (a) takes a short borrow, copies the plain `f64`/`Op` data
//! out, and releases it before touching any other node, or (b) mutates
//! exactly one node at a time. No borrow is ever held across a call that
//! could borrow the same node again, so `borrow()`/`borrow_mut()` are
//! panic-free. Keep that invariant when editing: never call another `Value`
//! method while a `borrow()` on `self` is still live.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::rc::Rc;

/// The operation that produced a node, holding handles to its operand nodes.
///
/// Only four rules need bespoke calculus (`Add`, `Mul`, `Powf`, plus the
/// unary functions); `-`, binary `-` and `/` are *compositions* of these (see
/// the `Neg`/`Sub`/`Div` trait impls), so they get their derivatives for free
/// from the chain rule — the same economy micrograd uses.
#[derive(Clone)]
enum Op {
    /// A leaf: user-created input, parameter, or constant. Owns no operands.
    Leaf,
    /// `out = lhs + rhs`
    Add(Value, Value),
    /// `out = lhs * rhs`
    Mul(Value, Value),
    /// `out = base ^ n` for a *constant* exponent `n`.
    Powf(Value, f64),
    /// `out = e^x`
    Exp(Value),
    /// `out = ln(x)` (natural logarithm).
    Ln(Value),
    /// `out = tanh(x)`
    Tanh(Value),
    /// `out = max(0, x)`
    Relu(Value),
}

/// The shared state of one graph node. Lives behind `Rc<RefCell<...>>`.
struct Node {
    /// The scalar this node evaluated to (forward pass, computed eagerly).
    data: f64,
    /// d(output)/d(this node), filled in by [`Value::backward`]. Starts at 0.
    grad: f64,
    /// How this node was produced; owns the `Rc` handles to its operands.
    op: Op,
}

/// A scalar in a computation graph: holds its value, its gradient, and the
/// operation that produced it.
///
/// `Value` is a cheap handle (`Rc` clone) to a shared node — cloning it does
/// **not** copy the scalar into a new node, it aliases the same one, which is
/// exactly what lets one node feed many consumers.
///
/// Arithmetic is defined on `&Value` (e.g. `&a + &b`) and each operation
/// eagerly computes its result while recording its inputs, so after building
/// an expression you can read the answer with [`Value::data`] and then call
/// [`Value::backward`] on it to fill every node's [`Value::grad`].
///
/// ```
/// use ml::Value;
///
/// let a = Value::new(2.0);
/// let b = Value::new(3.0);
/// let y = &(&a * &b) + &a; // y = a*b + a = 8
/// y.backward();
/// assert_eq!(y.data(), 8.0);
/// assert_eq!(a.grad(), 4.0); // dy/da = b + 1
/// assert_eq!(b.grad(), 2.0); // dy/db = a
/// ```
#[derive(Clone)]
pub struct Value(Rc<RefCell<Node>>);

impl Value {
    /// Creates a leaf node (an input, parameter, or constant) with `grad` 0.
    pub fn new(data: f64) -> Self {
        Self::from_op(data, Op::Leaf)
    }

    /// Allocates a fresh node. This is the *only* place nodes are created,
    /// and operand lists are never mutated afterwards — that is what keeps
    /// the graph acyclic (see module docs on the `Rc` cycle risk).
    fn from_op(data: f64, op: Op) -> Self {
        Value(Rc::new(RefCell::new(Node {
            data,
            grad: 0.0,
            op,
        })))
    }

    /// The scalar this node evaluated to.
    pub fn data(&self) -> f64 {
        self.0.borrow().data
    }

    /// d(output)/d(this node) as filled in by the last [`Value::backward`]
    /// call(s). Zero until then. Accumulates across calls — see
    /// [`Value::zero_grad`].
    pub fn grad(&self) -> f64 {
        self.0.borrow().grad
    }

    /// Nudges the stored scalar by `delta`. This is the SGD update hook:
    /// `param.adjust(-lr * param.grad())` moves the parameter downhill.
    ///
    /// Note it changes `data` only — downstream nodes computed from the old
    /// value are stale afterwards, which is fine because training rebuilds
    /// the graph fresh on every forward pass.
    pub fn adjust(&self, delta: f64) {
        self.0.borrow_mut().data += delta;
    }

    /// Resets this node's gradient to zero.
    ///
    /// Required between optimization steps: `backward()` *accumulates* into
    /// `grad` (see module docs), so skipping this silently sums gradients
    /// from different steps.
    pub fn zero_grad(&self) {
        self.0.borrow_mut().grad = 0.0;
    }

    /// Raises to a constant power: `x^n`. Derivative: `n * x^(n-1)`.
    ///
    /// The exponent is a plain `f64`, not a `Value` — differentiating with
    /// respect to the exponent is a different (rarely needed) rule.
    pub fn powf(&self, n: f64) -> Value {
        Value::from_op(self.data().powf(n), Op::Powf(self.clone(), n))
    }

    /// Exponential `e^x`. Derivative: `e^x` (the output itself).
    pub fn exp(&self) -> Value {
        Value::from_op(self.data().exp(), Op::Exp(self.clone()))
    }

    /// Natural logarithm `ln(x)`. Derivative: `1/x`.
    ///
    /// Only meaningful for `x > 0`; callers such as
    /// [`crate::loss::binary_cross_entropy`] must clamp inputs away from zero
    /// or the `-inf` output poisons every gradient upstream.
    pub fn ln(&self) -> Value {
        Value::from_op(self.data().ln(), Op::Ln(self.clone()))
    }

    /// Hyperbolic tangent, the classic smooth squashing activation.
    /// Derivative: `1 - tanh(x)^2`.
    pub fn tanh(&self) -> Value {
        Value::from_op(self.data().tanh(), Op::Tanh(self.clone()))
    }

    /// Rectified linear unit `max(0, x)`. Derivative: `1` for `x > 0`, else
    /// `0` (we adopt the usual convention of `0` at exactly `x == 0`).
    pub fn relu(&self) -> Value {
        Value::from_op(self.data().max(0.0), Op::Relu(self.clone()))
    }

    /// Runs reverse-mode differentiation from this node.
    ///
    /// Seeds `d(self)/d(self) = 1`, then visits every node in reverse
    /// topological order (each node strictly before anything it depends on)
    /// applying the chain rule, so by the time a node distributes gradient
    /// to its operands its own gradient is complete.
    ///
    /// Gradients **accumulate**: call [`Value::zero_grad`] on leaves you
    /// intend to re-read before backpropagating a new graph.
    pub fn backward(&self) {
        let order = self.topological_order();
        self.0.borrow_mut().grad = 1.0;
        for value in order.iter().rev() {
            value.apply_chain_rule();
        }
    }

    /// Post-order (operands-before-consumers) listing of the graph reachable
    /// from `self`. Iterative DFS so deep graphs cannot overflow the stack;
    /// nodes are deduplicated by pointer identity, so shared (diamond) nodes
    /// appear exactly once.
    fn topological_order(&self) -> Vec<Value> {
        let mut order = Vec::new();
        let mut visited: HashSet<*const RefCell<Node>> = HashSet::new();
        // (node, expanded): a node is pushed once to expand its operands and
        // once more (expanded = true) to emit it after they are all emitted.
        let mut stack = vec![(self.clone(), false)];
        while let Some((value, expanded)) = stack.pop() {
            if expanded {
                order.push(value);
                continue;
            }
            if !visited.insert(Rc::as_ptr(&value.0)) {
                continue;
            }
            let operands = value.operands();
            stack.push((value, true));
            for operand in operands {
                stack.push((operand, false));
            }
        }
        order
    }

    /// Clones out the operand handles of this node (empty for leaves).
    fn operands(&self) -> Vec<Value> {
        match &self.0.borrow().op {
            Op::Leaf => Vec::new(),
            Op::Add(a, b) | Op::Mul(a, b) => vec![a.clone(), b.clone()],
            Op::Powf(a, _) | Op::Exp(a) | Op::Ln(a) | Op::Tanh(a) | Op::Relu(a) => {
                vec![a.clone()]
            }
        }
    }

    /// One local step of the chain rule: takes this node's (already final)
    /// gradient and adds each operand's share to that operand.
    ///
    /// Borrow discipline: copy `grad`/`op` out of `self` first and release
    /// the borrow, *then* touch operands one at a time — so even `a * a`
    /// (both operands the same node) never double-borrows.
    fn apply_chain_rule(&self) {
        let (grad, op) = {
            let node = self.0.borrow();
            (node.grad, node.op.clone())
        };
        match op {
            Op::Leaf => {}
            Op::Add(a, b) => {
                // d(a+b)/da = 1, d(a+b)/db = 1
                a.accumulate_grad(grad);
                b.accumulate_grad(grad);
            }
            Op::Mul(a, b) => {
                // d(a*b)/da = b, d(a*b)/db = a
                let (a_data, b_data) = (a.data(), b.data());
                a.accumulate_grad(b_data * grad);
                b.accumulate_grad(a_data * grad);
            }
            Op::Powf(a, n) => {
                // d(a^n)/da = n * a^(n-1)
                let a_data = a.data();
                a.accumulate_grad(n * a_data.powf(n - 1.0) * grad);
            }
            Op::Exp(a) => {
                // d(e^a)/da = e^a, which is exactly this node's output.
                a.accumulate_grad(self.data() * grad);
            }
            Op::Ln(a) => {
                // d(ln a)/da = 1/a
                a.accumulate_grad(grad / a.data());
            }
            Op::Tanh(a) => {
                // d(tanh a)/da = 1 - tanh(a)^2, reusing the cached output.
                let t = self.data();
                a.accumulate_grad((1.0 - t * t) * grad);
            }
            Op::Relu(a) => {
                // Gradient passes through only where the unit was active.
                if self.data() > 0.0 {
                    a.accumulate_grad(grad);
                }
            }
        }
    }

    /// Adds (never assigns — see module docs) `delta` to this node's grad.
    fn accumulate_grad(&self, delta: f64) {
        self.0.borrow_mut().grad += delta;
    }

    /// `self + other` as a private helper so the operator trait impls below
    /// can delegate without writing arithmetic inside the impl bodies.
    fn add_val(&self, other: &Value) -> Value {
        Value::from_op(
            self.data() + other.data(),
            Op::Add(self.clone(), other.clone()),
        )
    }

    /// `self * other`; see [`Value::add_val`].
    fn mul_val(&self, other: &Value) -> Value {
        Value::from_op(
            self.data() * other.data(),
            Op::Mul(self.clone(), other.clone()),
        )
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = self.0.borrow();
        f.debug_struct("Value")
            .field("data", &node.data)
            .field("grad", &node.grad)
            .finish()
    }
}

/// `&a + &b`. Implemented on references because operands stay in the graph —
/// taking them by value would move parameters out of the model.
impl Add for &Value {
    type Output = Value;

    fn add(self, rhs: &Value) -> Value {
        self.add_val(rhs)
    }
}

/// `&a * &b`; see the [`Add`] impl for why references.
impl Mul for &Value {
    type Output = Value;

    fn mul(self, rhs: &Value) -> Value {
        self.mul_val(rhs)
    }
}

/// `-&a`, composed as `a * (-1)` so it needs no backward rule of its own.
impl Neg for &Value {
    type Output = Value;

    fn neg(self) -> Value {
        self.mul_val(&Value::new(-1.0))
    }
}

/// `&a - &b`, composed as `a + (b * -1)` — the chain rule differentiates the
/// composition for free.
impl Sub for &Value {
    type Output = Value;

    fn sub(self, rhs: &Value) -> Value {
        self.add_val(&rhs.mul_val(&Value::new(-1.0)))
    }
}

/// `&a / &b`, composed as `a * b^(-1)` — again no bespoke backward rule.
impl Div for &Value {
    type Output = Value;

    fn div(self, rhs: &Value) -> Value {
        self.mul_val(&rhs.powf(-1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < TOL,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn add_gradients_are_one() {
        let a = Value::new(2.0);
        let b = Value::new(3.0);
        let c = &a + &b;
        c.backward();
        assert_close(c.data(), 5.0);
        assert_close(a.grad(), 1.0);
        assert_close(b.grad(), 1.0);
    }

    #[test]
    fn mul_gradients_are_the_other_operand() {
        let a = Value::new(2.0);
        let b = Value::new(-3.0);
        let c = &a * &b;
        c.backward();
        assert_close(c.data(), -6.0);
        assert_close(a.grad(), -3.0);
        assert_close(b.grad(), 2.0);
    }

    #[test]
    fn neg_flips_sign_and_gradient() {
        let a = Value::new(4.0);
        let b = -&a;
        b.backward();
        assert_close(b.data(), -4.0);
        assert_close(a.grad(), -1.0);
    }

    #[test]
    fn sub_gradients_are_plus_and_minus_one() {
        let a = Value::new(7.0);
        let b = Value::new(3.0);
        let c = &a - &b;
        c.backward();
        assert_close(c.data(), 4.0);
        assert_close(a.grad(), 1.0);
        assert_close(b.grad(), -1.0);
    }

    #[test]
    fn div_gradients_match_quotient_rule() {
        let a = Value::new(6.0);
        let b = Value::new(2.0);
        let c = &a / &b;
        c.backward();
        assert_close(c.data(), 3.0);
        assert_close(a.grad(), 0.5); // 1/b
        assert_close(b.grad(), -1.5); // -a/b^2
    }

    #[test]
    fn powf_gradient_is_power_rule() {
        let a = Value::new(3.0);
        let b = a.powf(2.0);
        b.backward();
        assert_close(b.data(), 9.0);
        assert_close(a.grad(), 6.0); // 2 * a
    }

    #[test]
    fn exp_gradient_is_its_own_output() {
        let a = Value::new(1.5);
        let b = a.exp();
        b.backward();
        assert_close(b.data(), 1.5f64.exp());
        assert_close(a.grad(), 1.5f64.exp());
    }

    #[test]
    fn ln_gradient_is_reciprocal() {
        let a = Value::new(4.0);
        let b = a.ln();
        b.backward();
        assert_close(b.data(), 4.0f64.ln());
        assert_close(a.grad(), 0.25);
    }

    #[test]
    fn tanh_gradient_at_zero_is_one() {
        let a = Value::new(0.0);
        let b = a.tanh();
        b.backward();
        assert_close(b.data(), 0.0);
        assert_close(a.grad(), 1.0);
    }

    #[test]
    fn tanh_saturates_for_large_inputs() {
        let a = Value::new(20.0);
        let b = a.tanh();
        b.backward();
        assert!((b.data() - 1.0).abs() < 1e-12);
        assert!(a.grad().abs() < 1e-12, "saturated tanh should have ~0 grad");
    }

    #[test]
    fn relu_passes_gradient_when_active() {
        let a = Value::new(2.5);
        let b = a.relu();
        b.backward();
        assert_close(b.data(), 2.5);
        assert_close(a.grad(), 1.0);
    }

    #[test]
    fn relu_blocks_gradient_when_inactive_and_at_zero() {
        let a = Value::new(-1.0);
        let b = a.relu();
        b.backward();
        assert_close(b.data(), 0.0);
        assert_close(a.grad(), 0.0);

        let z = Value::new(0.0);
        let r = z.relu();
        r.backward();
        assert_close(r.data(), 0.0);
        assert_close(z.grad(), 0.0); // convention: dead at exactly 0
    }

    #[test]
    fn diamond_graph_accumulates_gradient() {
        // b = a + a: two paths from a to b, so da must receive 1 + 1 = 2.
        let a = Value::new(5.0);
        let b = &a + &a;
        b.backward();
        assert_close(b.data(), 10.0);
        assert_close(a.grad(), 2.0);

        // c = a * a: product rule via accumulation gives 2a.
        let a = Value::new(3.0);
        let c = &a * &a;
        c.backward();
        assert_close(a.grad(), 6.0);
    }

    #[test]
    fn wider_diamond_sums_all_paths() {
        // y = (a + 1) * (a + 2): dy/da = (a + 2) + (a + 1) = 2a + 3.
        let a = Value::new(4.0);
        let left = &a + &Value::new(1.0);
        let right = &a + &Value::new(2.0);
        let y = &left * &right;
        y.backward();
        assert_close(y.data(), 30.0);
        assert_close(a.grad(), 11.0);
    }

    #[test]
    fn gradients_accumulate_across_backward_calls_until_zeroed() {
        let a = Value::new(2.0);
        let b1 = &a * &Value::new(3.0);
        b1.backward();
        assert_close(a.grad(), 3.0);

        // A second backward pass without zero_grad adds on top.
        let b2 = &a * &Value::new(3.0);
        b2.backward();
        assert_close(a.grad(), 6.0);

        a.zero_grad();
        assert_close(a.grad(), 0.0);
    }

    #[test]
    fn adjust_nudges_data_in_place() {
        let a = Value::new(1.0);
        a.adjust(-0.25);
        assert_close(a.data(), 0.75);
    }

    #[test]
    fn debug_format_shows_data_and_grad() {
        let a = Value::new(1.5);
        let text = format!("{a:?}");
        assert!(text.contains("data"), "got {text}");
        assert!(text.contains("grad"), "got {text}");
    }

    /// The test that proves the whole crate: compare every analytic gradient
    /// against central finite differences on a compound expression that
    /// exercises add, mul, sub, div, neg, powf, exp, ln, tanh, and relu.
    #[test]
    fn gradient_check_against_finite_differences() {
        // f(a, b) = tanh(a*b + a^3) / (relu(a - b) + ln(exp(b))) - a
        fn build(a_val: f64, b_val: f64) -> (Value, Value, Value) {
            let a = Value::new(a_val);
            let b = Value::new(b_val);
            let numerator = (&(&a * &b) + &a.powf(3.0)).tanh();
            let denominator = &(&a - &b).relu() + &b.exp().ln();
            let out = &(&numerator / &denominator) - &a;
            (a, b, out)
        }
        let f = |a: f64, b: f64| build(a, b).2.data();

        let (a0, b0) = (1.3, 0.6);
        let (a, b, out) = build(a0, b0);
        out.backward();

        let h = 1e-5;
        let fd_a = (f(a0 + h, b0) - f(a0 - h, b0)) / (2.0 * h);
        let fd_b = (f(a0, b0 + h) - f(a0, b0 - h)) / (2.0 * h);

        assert!(
            (a.grad() - fd_a).abs() < 1e-6,
            "d/da: autograd {} vs finite diff {}",
            a.grad(),
            fd_a
        );
        assert!(
            (b.grad() - fd_b).abs() < 1e-6,
            "d/db: autograd {} vs finite diff {}",
            b.grad(),
            fd_b
        );
    }
}
