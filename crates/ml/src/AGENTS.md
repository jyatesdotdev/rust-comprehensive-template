# AGENTS.md — crates/ml/src

Read the root `/AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75).

## Why this crate exists

`ml` teaches how backpropagation actually works, micrograd-style: a scalar
reverse-mode autograd engine (`Value`) where every number is its own graph
node, plus the minimum machinery to train a real network with it (MLP, SGD
with momentum, MSE/BCE losses, a seeded RNG). Tensor frameworks — candle,
burn, tch — do exactly this, but behind tensor handles where the chain rule
is invisible; here the whole mechanism fits in one readable file. A second
engine (`Tensor`, in `tensor.rs`) then batches the same machinery to teach
the one step scalars cannot: matmul and broadcast backward — the conceptual
bridge from micrograd to PyTorch.

The crate is deliberately **pure std, zero dependencies, no unsafe**. That is
part of the lesson (autograd is just ownership, `Rc<RefCell<...>>`, and the
chain rule — no magic) and it keeps every file liftable into any project.
Do not add anything to `[dependencies]`.

## Key invariants (violating any of these breaks the crate quietly)

- **The graph must stay a DAG.** Every op is a *builder*: it allocates a
  fresh node (`Value::from_op`, the only construction site) whose operands
  already exist, and no API ever mutates a node's operand list. A node can
  therefore only reference strictly-older nodes, so `Rc` reference cycles —
  which would both leak memory and make the topological sort loop — are
  impossible by construction. Never add an op that rewires an existing
  node's inputs in place.
- **Gradients accumulate (`+=`), never assign.** The multivariate chain rule
  sums contributions over all paths; assignment silently computes wrong
  gradients for any diamond-shaped graph (`b = &a + &a` must give
  `a.grad() == 2`). The `diamond_graph_accumulates_gradient` test guards
  this.
- **`backward()` requires `zero_grad` between steps.** The flip side of
  accumulation: successive backward passes sum into the same `grad` fields.
  Training loops must run zero_grad → forward/backward → step, in that
  order (`Sgd::zero_grad` exists for exactly this).
- **No reentrant `RefCell` borrows.** `borrow()`/`borrow_mut()` in
  `autograd.rs` are panic-free because every method copies the plain data
  out of a short borrow *before* touching any other node (see
  `apply_chain_rule`). Never call another `Value` method while a borrow on
  `self` is live — that is the one way to make this crate panic.
- **All tests are deterministic.** Fixed RNG seeds, fixed epoch counts, no
  wall-clock, no threads. A flaky test here would poison CI for the whole
  workspace; if you add a training test, pin the seed and verify it passes
  repeatedly. Keep total crate test runtime well under ~2 s.

## Files

### lib.rs

Crate docs (what scalar autograd teaches that tensor libraries hide, and
what tensor autograd adds on top) and re-exports. Holds the three
end-to-end tests: a 2-4-1 MLP learning XOR with MSE, the same with
sigmoid+BCE, and the same network again as one batched `Tensor` pass (one
4×2 matmul per layer instead of neuron-by-neuron scalars — the scalar and
batched tests deliberately parallel each other so readers can diff them). XOR is chosen because it is the smallest
problem a linear model provably cannot solve. The BCE test builds sigmoid
out of primitives on purpose — it demonstrates composition, and it documents
that the BCE clamp is a numerical guard, *not* an activation (feeding raw
linear outputs into BCE stalls learning because clamped predictions become
constants with zero gradient).

### autograd.rs

The whole engine. `Value` is a cheap `Rc` handle to a shared
`RefCell<Node>`; `Node` stores `data`, `grad`, and the `Op` that produced it
(which owns the operand handles). Forward evaluation is eager; `backward()`
does an iterative post-order topological sort (pointer-identity visited set,
explicit stack — no recursion, so deep graphs cannot overflow) then applies
per-op chain rules in reverse. Only add/mul/powf/exp/ln/tanh/relu have
bespoke derivative rules; neg/sub/div are compositions (`a - b = a + b·(-1)`,
`a / b = a·b^(-1)`) that get differentiated for free — keep that economy.
Operators are implemented on `&Value` (operands stay in the graph); the
trait impls delegate to private `add_val`/`mul_val` helpers — that shape
avoids clippy's `suspicious_arithmetic_impl` on the composed ops, keep it.
The gradient-check test (autograd vs central finite differences) is the
crate's ground truth: any new op must be added to that compound expression.

### tensor.rs

The scalar engine batched: `Tensor` is 2D only (rows, cols, flat row-major
`Vec<f64>`) behind the same `Rc<RefCell<Node>>` graph. It exists to teach
the two lessons scalar autograd *cannot* express, and those two backward
identities are the invariants that must survive any edit:

- **Matmul backward:** for `C = A·B`, `dL/dA = dL/dC · Bᵀ` and
  `dL/dB = Aᵀ · dL/dC`. The `matmul_backward_matches_transpose_identities`
  test pins this on hand-checkable matrices.
- **Broadcast backward:** a `1×n` bias broadcast over `m` rows receives the
  **column-wise sum** of the upstream gradient (reduction is the adjoint of
  broadcast). Pinned by `broadcast_row_gradient_is_the_column_sum_of_upstream`.

Architecture is a **deliberate mirror of `autograd.rs`** — same builder-only
op construction, same topological sort, same `+=` accumulation, same borrow
discipline — so a reader can diff the two files and see that batching
changes only the payload (`f64` → shaped `Vec<f64>`) and adds those two
rules. Do not "improve" one file in a way that breaks the parallel; if you
change shared structure, change both.

Deliberate decisions to preserve: **2D only** (a batch of vectors is the
MLP-layer shape; N-d adds bookkeeping, not concepts — candle/burn/ndarray
for production); **broadcast is explicit** (`add_broadcast_row`, and `+`
demands equal shapes) because implicit numpy-style broadcasting plus
autograd silently sums gradients over the wrong axis instead of erroring;
`*` is elementwise, matrix product is spelled `matmul`; shape preconditions
are **documented asserts** — fail loudly at graph-build time, not silently
at backward time; `backward()` requires a `1×1` loss (a non-scalar seed is
ambiguous). All of `Value`'s DAG/accumulate/zero_grad/no-reentrant-borrow
invariants apply verbatim.

### rng.rs

Hand-rolled xorshift64* PRNG: `next_u64`, `next_f64` in [0,1) (top-53-bits
conversion), `range_f64`. Exists so weight init is reproducible with zero
deps — statistical quality is irrelevant here, determinism is everything.
Zero seeds are remapped (all-zero xorshift state is a fixed point).
Production code should use the `rand` crate.

### nn.rs

`Neuron` (w·x + b, optional tanh), `Layer` (independent neurons), `Mlp`
(stack of layers; hidden layers tanh, **output layer linear** — keep that,
losses/tests choose their own output squashing). Parameters are leaf
`Value`s created once and shared into every forward pass's graph; that
sharing is what makes batch gradients accumulate onto them. `parameters()`
order is stable (weights then bias, neuron by neuron, layer by layer) —
`Sgd` matches velocities to parameters by index, so reordering it breaks
momentum silently.

### optim.rs

`Sgd { lr, momentum }` with a lazily-sized per-parameter velocity vector.
`step()` reads `grad()` and applies `adjust()`; `zero_grad()` resets. The
optimizer deliberately knows nothing about graphs — that separation is the
lesson. Callers must pass the same parameter slice in the same order every
step.

### loss.rs

`mse` and `binary_cross_entropy`, both returning a `Value` at the tip of the
graph (targets are `f64` constants — we never differentiate w.r.t. labels).
BCE clamps predictions to `[ε, 1−ε]` because `ln(0) = −inf` poisons every
gradient upstream after one backward pass. The clamp exploits eager
evaluation: in-range predictions pass through untouched (gradient 1),
out-of-range ones are replaced by a constant leaf (gradient 0) — exactly the
derivative of a hard clamp. Do not "simplify" it into a data-only clamp on
the same node.

## Editing rules

- Zero dependencies, no unsafe, pure std. Do not add `common` either.
- No `unwrap`/`expect`/`panic` in library paths. `RefCell::borrow()`/
  `borrow_mut()` are sanctioned *only* under the no-reentrant-borrow
  discipline documented in `autograd.rs` — copy data out of short borrows
  before touching other nodes.
- Every public item carries a `///` doc comment; the derivative rule for
  each op is written next to its backward code — keep formulas and code
  adjacent.
- New ops need: the forward + backward arms, a known-case gradient test,
  and inclusion in the finite-difference gradient check. This applies to
  *both* engines — `autograd.rs` and `tensor.rs` each have their own
  `gradient_check_against_finite_differences`, and a new tensor op must be
  wired into the tensor one (matrix ops have too many index-shuffling ways
  to be subtly wrong for anything less).
- Shape preconditions on tensor ops are documented asserts, not `Option`s —
  a mismatch is a caller bug, and failing at graph-build time with both
  shapes in the message is the teachable behavior. Keep the asserts and
  their messages when refactoring.
- Tests are co-located in `#[cfg(test)] mod tests`; CI enforces ≥80% line
  coverage.
- Footgun: `Value::clone()` aliases the same node (that's the point); to
  copy a scalar into an independent leaf use `Value::new(v.data())`.
- Footgun: `adjust()` mutates a leaf's `data` but does not recompute
  downstream nodes — graphs are single-use; rebuild the forward pass after
  every optimizer step.

## Verification

```bash
cargo fmt -p ml
cargo test -p ml
cargo clippy -p ml --all-targets -- -D warnings
```
