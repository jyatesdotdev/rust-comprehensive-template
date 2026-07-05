//! Batched reverse-mode autodiff over 2D matrices — the bridge from
//! micrograd to PyTorch.
//!
//! This module is a **deliberate mirror of [`crate::autograd`]**: same
//! `Rc<RefCell<Node>>` graph, same builder-only op constructors, same
//! iterative topological sort, same accumulate-don't-assign gradient rule.
//! Diff the two files side by side and everything that survives unchanged is
//! "autograd"; everything that changed is "batching". What changes is small:
//! `data`/`grad` become `Vec<f64>` with a shape, and two genuinely new
//! backward rules appear that scalar autograd cannot express:
//!
//! - **Matmul backward.** For `C = A·B`,
//!   `dL/dA = dL/dC · Bᵀ` and `dL/dB = Aᵀ · dL/dC`.
//!   This one identity is the workhorse of every deep-learning framework —
//!   a layer's entire weight gradient in one matrix product instead of one
//!   graph node per scalar multiply.
//! - **Broadcast backward.** When a `1×n` bias row is broadcast over an
//!   `m×n` batch in the forward pass, its gradient is the **column-wise sum**
//!   of the upstream gradient. Reduction is the adjoint of broadcast: the
//!   forward pass copies the bias into `m` rows, so `m` gradient paths flow
//!   back into each bias element and the chain rule sums them.
//!
//! # Why 2D only
//!
//! A batch of feature vectors *is* a matrix — `m` samples × `n` features —
//! and that is exactly the shape flowing through an MLP layer
//! (`X·W + b`). N-dimensional tensors add stride/axis bookkeeping but no new
//! autodiff concepts; both lessons above already appear in full at 2D.
//! Production crates (candle, burn, ndarray) handle the N-d generality.
//!
//! # Why broadcasting is explicit ([`Tensor::add_broadcast_row`])
//!
//! NumPy-style implicit broadcasting silently accepts shape combinations you
//! did not intend, and autograd makes the footgun worse: the wrong forward
//! shape still *runs*, and the backward pass then quietly sums gradients
//! over the wrong axis — a model that trains, converges poorly, and never
//! errors. Here `+` demands identical shapes and broadcasting only happens
//! when you name it, so the one reduction in the backward pass is the one
//! you asked for.
//!
//! # What batching buys (and what it costs)
//!
//! Scalar XOR builds thousands of graph nodes per epoch — one per scalar
//! op. The batched version builds about ten *tensor* nodes per epoch,
//! regardless of layer width: the graph size now scales with the number of
//! *operations*, not the number of *parameters*. The cost is that each node's
//! backward rule must handle a whole matrix at once — which is precisely the
//! calculus this module exists to teach.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::ops::{Add, Mul, Sub};
use std::rc::Rc;

use crate::rng::Rng;

/// The operation that produced a node, holding handles to its operand nodes.
///
/// Mirrors `autograd::Op` — but where every scalar op's backward rule was one
/// line of calculus, [`Op::Matmul`] and [`Op::AddBroadcastRow`] carry the two
/// matrix-shaped rules that scalar autograd cannot express (see module docs).
/// `-` is still a composition (`a - b = a + b·(-1)`), keeping the same
/// economy as the scalar engine.
#[derive(Clone)]
enum Op {
    /// A leaf: input batch, parameter matrix, or constant. Owns no operands.
    Leaf,
    /// `out = lhs + rhs`, elementwise; identical shapes.
    Add(Tensor, Tensor),
    /// `out[i][j] = lhs[i][j] + row[0][j]` — an `m×n` matrix plus a `1×n`
    /// row vector replicated down every row (the bias pattern).
    AddBroadcastRow(Tensor, Tensor),
    /// `out = lhs ∘ rhs` (Hadamard/elementwise product); identical shapes.
    Mul(Tensor, Tensor),
    /// `out = x · s` for a *constant* scalar `s`.
    MulScalar(Tensor, f64),
    /// `out = lhs · rhs`, the matrix product (`m×k` times `k×n`).
    Matmul(Tensor, Tensor),
    /// `out = max(0, x)`, elementwise.
    Relu(Tensor),
    /// `out = tanh(x)`, elementwise.
    Tanh(Tensor),
    /// `out = Σ x[i][j]`, a `1×1` tensor.
    Sum(Tensor),
    /// `out = (Σ x[i][j]) / (rows·cols)`, a `1×1` tensor.
    Mean(Tensor),
}

/// The shared state of one graph node. Lives behind `Rc<RefCell<...>>`.
///
/// The only structural difference from `autograd::Node`: `data` and `grad`
/// are flat row-major buffers with a shape instead of single `f64`s.
struct Node {
    /// Number of rows (≥ 1).
    rows: usize,
    /// Number of columns (≥ 1).
    cols: usize,
    /// Row-major values: element `(i, j)` lives at `data[i * cols + j]`.
    data: Vec<f64>,
    /// d(loss)/d(this element), same layout as `data`. Starts at zero.
    grad: Vec<f64>,
    /// How this node was produced; owns the `Rc` handles to its operands.
    op: Op,
}

/// A 2D matrix in a computation graph: holds its values, its gradients, and
/// the operation that produced it.
///
/// Like [`crate::Value`], `Tensor` is a cheap handle (`Rc` clone) to a shared
/// node — cloning aliases the same node rather than copying the data, which
/// is what lets one tensor feed several consumers. Every op eagerly computes
/// its result while recording its inputs; call [`Tensor::backward`] on a
/// `1×1` loss to fill every node's [`Tensor::grad`].
///
/// Shape preconditions are enforced with documented asserts so mismatches
/// fail loudly at graph-*build* time, not silently at backward time.
///
/// ```
/// use ml::Tensor;
///
/// let a = Tensor::new(1, 2, vec![1.0, 2.0]).unwrap();
/// let b = Tensor::new(2, 1, vec![3.0, 4.0]).unwrap();
/// let loss = a.matmul(&b); // 1×1: [1·3 + 2·4] = [11]
/// loss.backward();
/// assert_eq!(loss.item(), 11.0);
/// assert_eq!(a.grad(), vec![3.0, 4.0]); // dL/dA = dL/dC · Bᵀ
/// assert_eq!(b.grad(), vec![1.0, 2.0]); // dL/dB = Aᵀ · dL/dC
/// ```
#[derive(Clone)]
pub struct Tensor(Rc<RefCell<Node>>);

impl Tensor {
    /// Creates a leaf tensor from row-major data.
    ///
    /// Returns `None` if either dimension is zero or `data.len()` is not
    /// `rows * cols` — the one fallible construction site; every op
    /// downstream can then rely on shapes being consistent.
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Option<Self> {
        if rows == 0 || cols == 0 || data.len() != rows * cols {
            return None;
        }
        Some(Self::from_op(rows, cols, data, Op::Leaf))
    }

    /// Creates a leaf tensor of zeros.
    ///
    /// Precondition (asserted): `rows > 0 && cols > 0`.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        assert!(
            rows > 0 && cols > 0,
            "Tensor::zeros: dimensions must be > 0"
        );
        Self::from_op(rows, cols, vec![0.0; rows * cols], Op::Leaf)
    }

    /// Creates a leaf tensor from row slices, e.g.
    /// `Tensor::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]])`.
    ///
    /// Returns `None` if `rows` is empty, any row is empty, or the rows are
    /// ragged (unequal lengths).
    pub fn from_rows(rows: &[&[f64]]) -> Option<Self> {
        let cols = rows.first()?.len();
        if cols == 0 || rows.iter().any(|row| row.len() != cols) {
            return None;
        }
        let data = rows.iter().flat_map(|row| row.iter().copied()).collect();
        Some(Self::from_op(rows.len(), cols, data, Op::Leaf))
    }

    /// Creates a leaf tensor with entries drawn uniformly from `[-1, 1)` —
    /// the same init scheme as [`crate::Neuron`], and deterministic for the
    /// same [`Rng`] state.
    ///
    /// Precondition (asserted): `rows > 0 && cols > 0`.
    pub fn random(rows: usize, cols: usize, rng: &mut Rng) -> Self {
        assert!(
            rows > 0 && cols > 0,
            "Tensor::random: dimensions must be > 0"
        );
        let data = (0..rows * cols).map(|_| rng.range_f64(-1.0, 1.0)).collect();
        Self::from_op(rows, cols, data, Op::Leaf)
    }

    /// Allocates a fresh node. This is the *only* place nodes are created,
    /// and operand lists are never mutated afterwards — the same
    /// acyclicity-by-construction argument as `Value::from_op`.
    fn from_op(rows: usize, cols: usize, data: Vec<f64>, op: Op) -> Self {
        let grad = vec![0.0; data.len()];
        Tensor(Rc::new(RefCell::new(Node {
            rows,
            cols,
            data,
            grad,
            op,
        })))
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.0.borrow().rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.0.borrow().cols
    }

    /// A copy of the row-major values (element `(i, j)` at index
    /// `i * cols + j`).
    pub fn data(&self) -> Vec<f64> {
        self.0.borrow().data.clone()
    }

    /// A copy of the row-major gradients, as filled in by the last
    /// [`Tensor::backward`] call(s). Zeros until then; accumulates across
    /// calls — see [`Tensor::zero_grad`].
    pub fn grad(&self) -> Vec<f64> {
        self.0.borrow().grad.clone()
    }

    /// The single value of a `1×1` tensor (the loss shape).
    ///
    /// Precondition (asserted): `self` is `1×1`.
    pub fn item(&self) -> f64 {
        let node = self.0.borrow();
        assert!(
            node.rows == 1 && node.cols == 1,
            "Tensor::item: tensor is {}x{}, not 1x1",
            node.rows,
            node.cols
        );
        node.data[0]
    }

    /// Nudges every element by `step * grad`: the SGD update hook, the
    /// tensor analogue of `param.adjust(-lr * param.grad())` — call
    /// `w.adjust(-lr)` after `backward()` to move downhill.
    ///
    /// Like [`crate::Value::adjust`], this changes `data` only; downstream
    /// nodes are stale afterwards, so rebuild the forward pass every step.
    pub fn adjust(&self, step: f64) {
        let mut node = self.0.borrow_mut();
        // Reborrow as &mut Node so data (mut) and grad (shared) split.
        let node = &mut *node;
        for (d, g) in node.data.iter_mut().zip(&node.grad) {
            *d += step * g;
        }
    }

    /// Resets every element's gradient to zero. Required between
    /// optimization steps for the same reason as [`crate::Value::zero_grad`]:
    /// `backward()` accumulates.
    pub fn zero_grad(&self) {
        for g in self.0.borrow_mut().grad.iter_mut() {
            *g = 0.0;
        }
    }

    /// Adds a `1×n` row vector to every row of this `m×n` matrix — the bias
    /// pattern in `X·W + b`, and the one *deliberately explicit* broadcast
    /// in this module (module docs explain why implicit broadcasting is a
    /// footgun).
    ///
    /// Backward: the matrix operand receives the upstream gradient
    /// unchanged; the row operand receives its **column-wise sum**, because
    /// the forward copy into `m` rows means `m` chain-rule paths flow back
    /// into each row element. Reduction is the adjoint of broadcast.
    ///
    /// Precondition (asserted): `row` is `1×n` with `n == self.cols()`.
    pub fn add_broadcast_row(&self, row: &Tensor) -> Tensor {
        let (rows, cols) = (self.rows(), self.cols());
        assert!(
            row.rows() == 1 && row.cols() == cols,
            "add_broadcast_row: expected 1x{cols} row, got {}x{}",
            row.rows(),
            row.cols()
        );
        let row_data = row.data();
        let data = self
            .data()
            .iter()
            .enumerate()
            .map(|(i, x)| x + row_data[i % cols])
            .collect();
        Tensor::from_op(
            rows,
            cols,
            data,
            Op::AddBroadcastRow(self.clone(), row.clone()),
        )
    }

    /// Multiplies every element by a constant scalar.
    /// Derivative: `d(s·x)/dx = s`.
    pub fn mul_scalar(&self, s: f64) -> Tensor {
        let data = self.data().iter().map(|x| x * s).collect();
        Tensor::from_op(
            self.rows(),
            self.cols(),
            data,
            Op::MulScalar(self.clone(), s),
        )
    }

    /// Matrix product: `self (m×k) · rhs (k×n) → m×n`. One call replaces an
    /// entire layer's worth of scalar multiply-add graph nodes.
    ///
    /// Backward — *the* identity of deep learning:
    /// `dL/dA = dL/dC · Bᵀ` and `dL/dB = Aᵀ · dL/dC`
    /// (shapes check out: `(m×n)·(n×k) = m×k` and `(k×m)·(m×n) = k×n`).
    ///
    /// Precondition (asserted): `self.cols() == rhs.rows()`.
    pub fn matmul(&self, rhs: &Tensor) -> Tensor {
        let (m, k) = (self.rows(), self.cols());
        let (k2, n) = (rhs.rows(), rhs.cols());
        assert!(
            k == k2,
            "matmul: inner dimensions differ ({m}x{k} vs {k2}x{n})"
        );
        let data = mat_mul(&self.data(), m, k, &rhs.data(), n);
        Tensor::from_op(m, n, data, Op::Matmul(self.clone(), rhs.clone()))
    }

    /// Elementwise `max(0, x)`. Derivative: `1` where the output is
    /// positive, else `0` (same `0`-at-`0` convention as
    /// [`crate::Value::relu`]).
    pub fn relu(&self) -> Tensor {
        let data = self.data().iter().map(|x| x.max(0.0)).collect();
        Tensor::from_op(self.rows(), self.cols(), data, Op::Relu(self.clone()))
    }

    /// Elementwise hyperbolic tangent. Derivative: `1 - tanh(x)²`, reusing
    /// the cached output like the scalar engine does.
    pub fn tanh(&self) -> Tensor {
        let data = self.data().iter().map(|x| x.tanh()).collect();
        Tensor::from_op(self.rows(), self.cols(), data, Op::Tanh(self.clone()))
    }

    /// Sum of all elements as a `1×1` tensor. Backward: every element
    /// contributed with coefficient 1, so each receives the upstream
    /// gradient unchanged — sum is broadcast's adjoint in the other
    /// direction.
    pub fn sum(&self) -> Tensor {
        let total = self.data().iter().sum();
        Tensor::from_op(1, 1, vec![total], Op::Sum(self.clone()))
    }

    /// Mean of all elements as a `1×1` tensor — the loss shape. Backward:
    /// each element receives `upstream / (rows·cols)`.
    pub fn mean(&self) -> Tensor {
        let data = self.data();
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        Tensor::from_op(1, 1, vec![mean], Op::Mean(self.clone()))
    }

    /// Mean squared error against a target tensor, as a `1×1` loss:
    /// `mean((self − target)²)`.
    ///
    /// Pure composition — `sub`, elementwise `mul`, `mean` — so it needs no
    /// backward rule of its own, exactly like the scalar [`crate::mse`].
    /// Targets are normally constant leaves; gradients do flow into
    /// `target`, they are simply never read.
    ///
    /// Precondition (asserted, via `-`): shapes match.
    pub fn mse_loss(&self, target: &Tensor) -> Tensor {
        let diff = self - target;
        (&diff * &diff).mean()
    }

    /// Runs reverse-mode differentiation from this `1×1` loss.
    ///
    /// Seeds `d(loss)/d(loss) = 1`, then applies each node's chain rule in
    /// reverse topological order — structurally identical to
    /// [`crate::Value::backward`]. Backward from a non-scalar has no single
    /// well-defined seed (which output element is "the" objective?), so like
    /// PyTorch's argument-less `.backward()` we require a scalar.
    ///
    /// Gradients **accumulate**: call [`Tensor::zero_grad`] on leaves you
    /// intend to re-read before backpropagating a new graph.
    ///
    /// Precondition (asserted): `self` is `1×1`.
    pub fn backward(&self) {
        {
            let mut node = self.0.borrow_mut();
            assert!(
                node.rows == 1 && node.cols == 1,
                "backward: loss must be 1x1, got {}x{}",
                node.rows,
                node.cols
            );
            node.grad[0] = 1.0;
        }
        let order = self.topological_order();
        for tensor in order.iter().rev() {
            tensor.apply_chain_rule();
        }
    }

    /// Post-order (operands-before-consumers) listing of the graph reachable
    /// from `self` — verbatim the algorithm from `autograd.rs`: iterative
    /// DFS, pointer-identity visited set, shared nodes emitted exactly once.
    fn topological_order(&self) -> Vec<Tensor> {
        let mut order = Vec::new();
        let mut visited: HashSet<*const RefCell<Node>> = HashSet::new();
        // (node, expanded): a node is pushed once to expand its operands and
        // once more (expanded = true) to emit it after they are all emitted.
        let mut stack = vec![(self.clone(), false)];
        while let Some((tensor, expanded)) = stack.pop() {
            if expanded {
                order.push(tensor);
                continue;
            }
            if !visited.insert(Rc::as_ptr(&tensor.0)) {
                continue;
            }
            let operands = tensor.operands();
            stack.push((tensor, true));
            for operand in operands {
                stack.push((operand, false));
            }
        }
        order
    }

    /// Clones out the operand handles of this node (empty for leaves).
    fn operands(&self) -> Vec<Tensor> {
        match &self.0.borrow().op {
            Op::Leaf => Vec::new(),
            Op::Add(a, b) | Op::AddBroadcastRow(a, b) | Op::Mul(a, b) | Op::Matmul(a, b) => {
                vec![a.clone(), b.clone()]
            }
            Op::MulScalar(a, _) | Op::Relu(a) | Op::Tanh(a) | Op::Sum(a) | Op::Mean(a) => {
                vec![a.clone()]
            }
        }
    }

    /// One local step of the chain rule: takes this node's (already final)
    /// gradient and adds each operand's share to that operand.
    ///
    /// Same borrow discipline as the scalar engine: copy `grad`/`op` out of
    /// `self` first and release the borrow, *then* touch operands one at a
    /// time — so even `&a * &a` (both operands the same node) never
    /// double-borrows.
    fn apply_chain_rule(&self) {
        let (grad, op) = {
            let node = self.0.borrow();
            (node.grad.clone(), node.op.clone())
        };
        match op {
            Op::Leaf => {}
            Op::Add(a, b) => {
                // d(a+b)/da = 1, d(a+b)/db = 1, elementwise.
                a.accumulate_grad(&grad);
                b.accumulate_grad(&grad);
            }
            Op::AddBroadcastRow(a, row) => {
                // The matrix gets the gradient unchanged; the broadcast row
                // gets the COLUMN-WISE SUM — m forward copies means m
                // backward paths summing into each row element.
                a.accumulate_grad(&grad);
                let cols = row.cols();
                let mut row_grad = vec![0.0; cols];
                for (i, g) in grad.iter().enumerate() {
                    row_grad[i % cols] += g;
                }
                row.accumulate_grad(&row_grad);
            }
            Op::Mul(a, b) => {
                // Elementwise product rule: d(a∘b)/da = b, d(a∘b)/db = a.
                let (a_data, b_data) = (a.data(), b.data());
                a.accumulate_grad(&hadamard(&b_data, &grad));
                b.accumulate_grad(&hadamard(&a_data, &grad));
            }
            Op::MulScalar(a, s) => {
                // d(s·a)/da = s.
                let delta: Vec<f64> = grad.iter().map(|g| s * g).collect();
                a.accumulate_grad(&delta);
            }
            Op::Matmul(a, b) => {
                // dL/dA = dL/dC · Bᵀ ; dL/dB = Aᵀ · dL/dC.
                let (m, k) = (a.rows(), a.cols());
                let n = b.cols();
                let b_t = transpose(&b.data(), k, n); // n×k
                let a_t = transpose(&a.data(), m, k); // k×m
                a.accumulate_grad(&mat_mul(&grad, m, n, &b_t, k));
                b.accumulate_grad(&mat_mul(&a_t, k, m, &grad, n));
            }
            Op::Relu(a) => {
                // Gradient passes through only where the unit was active.
                let out = self.data();
                let delta: Vec<f64> = grad
                    .iter()
                    .zip(&out)
                    .map(|(g, o)| if *o > 0.0 { *g } else { 0.0 })
                    .collect();
                a.accumulate_grad(&delta);
            }
            Op::Tanh(a) => {
                // d(tanh x)/dx = 1 - tanh(x)², reusing the cached output.
                let out = self.data();
                let delta: Vec<f64> = grad
                    .iter()
                    .zip(&out)
                    .map(|(g, t)| (1.0 - t * t) * g)
                    .collect();
                a.accumulate_grad(&delta);
            }
            Op::Sum(a) => {
                // Every element fed the total with coefficient 1.
                let g = grad[0];
                let delta = vec![g; a.data().len()];
                a.accumulate_grad(&delta);
            }
            Op::Mean(a) => {
                // Like Sum, scaled by 1/count.
                let count = a.data().len();
                let g = grad[0] / count as f64;
                let delta = vec![g; count];
                a.accumulate_grad(&delta);
            }
        }
    }

    /// Adds (never assigns — same accumulation rule as the scalar engine)
    /// `delta` elementwise into this node's grad.
    fn accumulate_grad(&self, delta: &[f64]) {
        let mut node = self.0.borrow_mut();
        for (g, d) in node.grad.iter_mut().zip(delta) {
            *g += d;
        }
    }

    /// `self + other` (elementwise, identical shapes) as a private helper so
    /// the operator trait impls can delegate — same shape as
    /// `Value::add_val`, and it keeps clippy's `suspicious_arithmetic_impl`
    /// away from the composed `Sub`.
    ///
    /// Precondition (asserted): shapes match — a plain `+` never broadcasts
    /// here (see module docs).
    fn add_tensor(&self, other: &Tensor) -> Tensor {
        self.assert_same_shape(other, "add");
        let data = self
            .data()
            .iter()
            .zip(other.data())
            .map(|(x, y)| x + y)
            .collect();
        Tensor::from_op(
            self.rows(),
            self.cols(),
            data,
            Op::Add(self.clone(), other.clone()),
        )
    }

    /// `self ∘ other` (elementwise); see [`Tensor::add_tensor`].
    ///
    /// Precondition (asserted): shapes match.
    fn mul_tensor(&self, other: &Tensor) -> Tensor {
        self.assert_same_shape(other, "mul");
        let data = self
            .data()
            .iter()
            .zip(other.data())
            .map(|(x, y)| x * y)
            .collect();
        Tensor::from_op(
            self.rows(),
            self.cols(),
            data,
            Op::Mul(self.clone(), other.clone()),
        )
    }

    /// Shared precondition check: fail loudly at graph-build time with both
    /// shapes in the message, instead of silently mis-summing at backward
    /// time.
    fn assert_same_shape(&self, other: &Tensor, op: &str) {
        assert!(
            self.rows() == other.rows() && self.cols() == other.cols(),
            "{op}: shape mismatch {}x{} vs {}x{}",
            self.rows(),
            self.cols(),
            other.rows(),
            other.cols()
        );
    }
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = self.0.borrow();
        f.debug_struct("Tensor")
            .field("rows", &node.rows)
            .field("cols", &node.cols)
            .field("data", &node.data)
            .field("grad", &node.grad)
            .finish()
    }
}

/// `&a + &b`, elementwise. On references for the same reason as `&Value +
/// &Value`: operands stay in the graph. Never broadcasts — use
/// [`Tensor::add_broadcast_row`] and say so.
impl Add for &Tensor {
    type Output = Tensor;

    fn add(self, rhs: &Tensor) -> Tensor {
        self.add_tensor(rhs)
    }
}

/// `&a * &b`, the **elementwise** (Hadamard) product — matrix multiplication
/// is spelled [`Tensor::matmul`], the same split (`*` vs `@`/`matmul`) NumPy
/// and PyTorch settled on after decades of `*`-means-what confusion.
impl Mul for &Tensor {
    type Output = Tensor;

    fn mul(self, rhs: &Tensor) -> Tensor {
        self.mul_tensor(rhs)
    }
}

/// `&a - &b`, composed as `a + b·(-1)` — the chain rule differentiates the
/// composition for free, the same economy as the scalar engine.
impl Sub for &Tensor {
    type Output = Tensor;

    fn sub(self, rhs: &Tensor) -> Tensor {
        self.add_tensor(&rhs.mul_scalar(-1.0))
    }
}

/// Row-major matrix product of `a` (`m×k`) and `b` (`k×n`); shapes are the
/// caller's responsibility (all callers have just asserted them). The i-p-j
/// loop order keeps the inner loop streaming over contiguous rows of both
/// `out` and `b`.
fn mat_mul(a: &[f64], m: usize, k: usize, b: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; m * n];
    for i in 0..m {
        for p in 0..k {
            let a_ip = a[i * k + p];
            for j in 0..n {
                out[i * n + j] += a_ip * b[p * n + j];
            }
        }
    }
    out
}

/// Transpose of a row-major `rows×cols` buffer.
fn transpose(data: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let mut out = vec![0.0; data.len()];
    for i in 0..rows {
        for j in 0..cols {
            out[j * rows + i] = data[i * cols + j];
        }
    }
    out
}

/// Elementwise product of two equal-length buffers.
fn hadamard(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x * y).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    fn assert_all_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < TOL, "expected {expected:?}, got {actual:?}");
        }
    }

    #[test]
    fn new_validates_shape_against_data_length() {
        assert!(Tensor::new(2, 3, vec![0.0; 6]).is_some());
        assert!(Tensor::new(2, 3, vec![0.0; 5]).is_none());
        assert!(Tensor::new(0, 3, vec![]).is_none());
        assert!(Tensor::new(3, 0, vec![]).is_none());
    }

    #[test]
    fn from_rows_rejects_ragged_and_empty_input() {
        let t = Tensor::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]).unwrap();
        assert_eq!((t.rows(), t.cols()), (2, 2));
        assert_eq!(t.data(), vec![1.0, 2.0, 3.0, 4.0]);

        assert!(Tensor::from_rows(&[]).is_none());
        assert!(Tensor::from_rows(&[&[]]).is_none());
        assert!(Tensor::from_rows(&[&[1.0], &[1.0, 2.0]]).is_none());
    }

    #[test]
    fn zeros_and_random_have_requested_shape() {
        let z = Tensor::zeros(2, 3);
        assert_eq!((z.rows(), z.cols()), (2, 3));
        assert!(z.data().iter().all(|x| *x == 0.0));

        let mut rng_a = Rng::new(42);
        let mut rng_b = Rng::new(42);
        let a = Tensor::random(3, 2, &mut rng_a);
        let b = Tensor::random(3, 2, &mut rng_b);
        assert_eq!(a.data(), b.data(), "same seed must give same init");
        assert!(a.data().iter().all(|x| (-1.0..1.0).contains(x)));
    }

    #[test]
    fn add_gradients_are_one() {
        let a = Tensor::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let b = Tensor::new(2, 2, vec![10.0, 20.0, 30.0, 40.0]).unwrap();
        let loss = (&a + &b).sum();
        loss.backward();
        assert_eq!(loss.item(), 110.0);
        assert_all_close(&a.grad(), &[1.0; 4]);
        assert_all_close(&b.grad(), &[1.0; 4]);
    }

    #[test]
    fn sub_gradients_are_plus_and_minus_one() {
        let a = Tensor::new(1, 2, vec![5.0, 7.0]).unwrap();
        let b = Tensor::new(1, 2, vec![2.0, 3.0]).unwrap();
        let loss = (&a - &b).sum();
        loss.backward();
        assert_eq!(loss.item(), 7.0);
        assert_all_close(&a.grad(), &[1.0, 1.0]);
        assert_all_close(&b.grad(), &[-1.0, -1.0]);
    }

    #[test]
    fn elementwise_mul_gradients_are_the_other_operand() {
        let a = Tensor::new(1, 3, vec![1.0, 2.0, 3.0]).unwrap();
        let b = Tensor::new(1, 3, vec![4.0, 5.0, 6.0]).unwrap();
        let loss = (&a * &b).sum();
        loss.backward();
        assert_eq!(loss.item(), 32.0);
        assert_all_close(&a.grad(), &[4.0, 5.0, 6.0]);
        assert_all_close(&b.grad(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn mul_scalar_scales_data_and_gradient() {
        let a = Tensor::new(1, 2, vec![3.0, -1.0]).unwrap();
        let loss = a.mul_scalar(2.5).sum();
        loss.backward();
        assert_eq!(loss.item(), 5.0);
        assert_all_close(&a.grad(), &[2.5, 2.5]);
    }

    #[test]
    fn matmul_forward_matches_hand_computation() {
        // [1 2; 3 4] · [5 6; 7 8] = [19 22; 43 50]
        let a = Tensor::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]).unwrap();
        let b = Tensor::from_rows(&[&[5.0, 6.0], &[7.0, 8.0]]).unwrap();
        let c = a.matmul(&b);
        assert_eq!((c.rows(), c.cols()), (2, 2));
        assert_all_close(&c.data(), &[19.0, 22.0, 43.0, 50.0]);
    }

    /// The directed test for lesson #1: with loss = sum(A·B) the upstream
    /// gradient dL/dC is all ones, so the identities dL/dA = dL/dC·Bᵀ and
    /// dL/dB = Aᵀ·dL/dC reduce to hand-checkable row/column sums:
    /// dA[i][p] = Σ_j B[p][j] and dB[p][j] = Σ_i A[i][p].
    #[test]
    fn matmul_backward_matches_transpose_identities() {
        let a = Tensor::from_rows(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]]).unwrap(); // 2×3
        let b = Tensor::from_rows(&[&[1.0, -1.0], &[2.0, 0.5], &[0.0, 3.0]]).unwrap(); // 3×2
        let loss = a.matmul(&b).sum();
        loss.backward();

        // dA[i][p] = row-sum of B's row p: [0, 2.5, 3] in every row of dA.
        assert_all_close(&a.grad(), &[0.0, 2.5, 3.0, 0.0, 2.5, 3.0]);
        // dB[p][j] = column-sum of A's column p: [5, 7, 9] down both columns.
        assert_all_close(&b.grad(), &[5.0, 5.0, 7.0, 7.0, 9.0, 9.0]);
    }

    /// The directed test for lesson #2: the broadcast bias gradient is the
    /// column-wise sum of the upstream gradient. An elementwise multiply by
    /// a known matrix makes the upstream gradient non-uniform, so a wrong
    /// axis (or a wrong `=` instead of `+=`) cannot pass by accident.
    #[test]
    fn broadcast_row_gradient_is_the_column_sum_of_upstream() {
        let x = Tensor::zeros(3, 2);
        let bias = Tensor::new(1, 2, vec![0.5, -0.5]).unwrap();
        let weights = Tensor::from_rows(&[&[1.0, 10.0], &[2.0, 20.0], &[3.0, 30.0]]).unwrap();
        // loss = Σ (x .+ bias) ∘ weights ⇒ upstream grad of the broadcast
        // node is exactly `weights`.
        let loss = (&x.add_broadcast_row(&bias) * &weights).sum();
        loss.backward();

        // d(bias)[j] = Σ_i weights[i][j]: [1+2+3, 10+20+30].
        assert_all_close(&bias.grad(), &[6.0, 60.0]);
        // The matrix operand receives the upstream gradient unchanged.
        assert_all_close(&x.grad(), &weights.data());
    }

    #[test]
    fn relu_passes_gradient_only_where_active() {
        let a = Tensor::new(1, 4, vec![-2.0, -0.0, 0.5, 3.0]).unwrap();
        let loss = a.relu().sum();
        loss.backward();
        assert_all_close(&loss.data(), &[3.5]);
        // Dead at negatives and (by convention) at exactly zero.
        assert_all_close(&a.grad(), &[0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn tanh_gradient_is_one_minus_output_squared() {
        let a = Tensor::new(1, 2, vec![0.0, 20.0]).unwrap();
        let loss = a.tanh().sum();
        loss.backward();
        let grads = a.grad();
        assert!((grads[0] - 1.0).abs() < TOL, "tanh'(0) = 1");
        assert!(grads[1].abs() < 1e-12, "saturated tanh should have ~0 grad");
    }

    #[test]
    fn sum_and_mean_reduce_to_one_by_one() {
        let a = Tensor::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let s = a.sum();
        assert_eq!((s.rows(), s.cols()), (1, 1));
        assert_eq!(s.item(), 10.0);

        let m = a.mean();
        m.backward();
        assert_eq!(m.item(), 2.5);
        // d(mean)/d(a[i][j]) = 1/4.
        assert_all_close(&a.grad(), &[0.25; 4]);
    }

    #[test]
    fn mse_loss_matches_hand_computation_and_gradient() {
        // loss = mean((p − t)²); d/dp = 2(p − t)/count.
        let pred = Tensor::new(2, 1, vec![1.0, -1.0]).unwrap();
        let target = Tensor::new(2, 1, vec![0.0, 1.0]).unwrap();
        let loss = pred.mse_loss(&target);
        loss.backward();
        assert!((loss.item() - 2.5).abs() < TOL); // (1 + 4) / 2
        assert_all_close(&pred.grad(), &[1.0, -2.0]);
    }

    #[test]
    fn diamond_graph_accumulates_gradient() {
        // loss = Σ a ∘ a: both operands are the SAME node, so each element's
        // gradient must accumulate to 2a — the tensor version of the scalar
        // engine's diamond test.
        let a = Tensor::new(1, 3, vec![1.0, -2.0, 3.0]).unwrap();
        let loss = (&a * &a).sum();
        loss.backward();
        assert_eq!(loss.item(), 14.0);
        assert_all_close(&a.grad(), &[2.0, -4.0, 6.0]);
    }

    #[test]
    fn gradients_accumulate_across_backward_calls_until_zeroed() {
        let a = Tensor::new(1, 1, vec![2.0]).unwrap();
        a.mul_scalar(3.0).backward();
        assert_all_close(&a.grad(), &[3.0]);

        a.mul_scalar(3.0).backward();
        assert_all_close(&a.grad(), &[6.0]);

        a.zero_grad();
        assert_all_close(&a.grad(), &[0.0]);
    }

    #[test]
    fn adjust_steps_along_the_stored_gradient() {
        // loss = mean(a) over 2 elements ⇒ grad = [0.5, 0.5];
        // adjust(-0.1) ⇒ each element moves by -0.05.
        let a = Tensor::new(1, 2, vec![1.0, 2.0]).unwrap();
        a.mean().backward();
        a.adjust(-0.1);
        assert_all_close(&a.data(), &[0.95, 1.95]);
    }

    #[test]
    fn debug_format_shows_shape_data_and_grad() {
        let a = Tensor::new(1, 2, vec![1.5, 2.5]).unwrap();
        let text = format!("{a:?}");
        assert!(text.contains("rows"), "got {text}");
        assert!(text.contains("data"), "got {text}");
        assert!(text.contains("grad"), "got {text}");
    }

    #[test]
    #[should_panic(expected = "matmul: inner dimensions differ")]
    fn matmul_rejects_mismatched_inner_dimensions() {
        let a = Tensor::zeros(2, 3);
        let b = Tensor::zeros(2, 3); // needs 3×n
        let _ = a.matmul(&b);
    }

    #[test]
    #[should_panic(expected = "backward: loss must be 1x1")]
    fn backward_rejects_non_scalar_loss() {
        Tensor::zeros(2, 2).backward();
    }

    /// The ground-truth test for the module, mirroring the scalar engine's
    /// `gradient_check_against_finite_differences`: a compound graph
    /// exercising matmul, broadcast add, tanh, elementwise (diamond) mul,
    /// and mean, with every analytic gradient checked against central
    /// finite differences. Any new tensor op must be added here.
    #[test]
    fn gradient_check_against_finite_differences() {
        // f(X, W, b) = mean(h ∘ h) where h = tanh(X·W .+ b).
        let x0 = [0.4, -0.7, 1.2, 0.9, -0.3, 0.5]; // X: 2×3
        let w0 = [0.6, -1.1, 0.2, 0.8, -0.5, 0.3]; // W: 3×2
        let b0 = [0.1, -0.2]; // b: 1×2

        fn build(x: &[f64], w: &[f64], b: &[f64]) -> (Tensor, Tensor, Tensor, Tensor) {
            let x = Tensor::new(2, 3, x.to_vec()).unwrap();
            let w = Tensor::new(3, 2, w.to_vec()).unwrap();
            let b = Tensor::new(1, 2, b.to_vec()).unwrap();
            let h = x.matmul(&w).add_broadcast_row(&b).tanh();
            let out = (&h * &h).mean();
            (x, w, b, out)
        }
        let f = |x: &[f64], w: &[f64], b: &[f64]| build(x, w, b).3.item();

        let (x, w, b, out) = build(&x0, &w0, &b0);
        out.backward();

        let h = 1e-5;
        let check = |name: &str, values: &[f64], grads: &[f64], eval: &dyn Fn(&[f64]) -> f64| {
            for (i, grad) in grads.iter().enumerate() {
                let mut plus = values.to_vec();
                let mut minus = values.to_vec();
                plus[i] += h;
                minus[i] -= h;
                let fd = (eval(&plus) - eval(&minus)) / (2.0 * h);
                assert!(
                    (grad - fd).abs() < 1e-6,
                    "d/d{name}[{i}]: autograd {grad} vs finite diff {fd}"
                );
            }
        };
        check("X", &x0, &x.grad(), &|v| f(v, &w0, &b0));
        check("W", &w0, &w.grad(), &|v| f(&x0, v, &b0));
        check("b", &b0, &b.grad(), &|v| f(&x0, &w0, v));
    }
}
