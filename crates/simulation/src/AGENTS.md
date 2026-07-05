# AGENTS.md — crates/simulation/src

Read the root `AGENTS.md` first for workspace-wide rules (coverage ≥ 80%,
clippy `-D warnings`, MSRV 1.75, no unwrap/panic in library paths — though
`assert!` on documented preconditions is the established style here).

## Why this crate exists

This crate teaches **numerical computing and simulation architecture** in
minimal form. Two ideas dominate:

1. **Numerical methods are chosen for their error behavior, not their
   simplicity.** `physics.rs` uses velocity-Verlet instead of naive Euler
   because Verlet is *symplectic*: its energy error stays bounded over long
   runs, so orbits neither spiral in nor fly apart, while Euler's error
   accumulates every step. Likewise `numerical.rs` exposes tolerance and
   iteration-count parameters because floating-point iteration never reaches
   "exact" — convergence is always *to within a tolerance*, and the caller
   must own that trade-off.
2. **Composition over inheritance for simulation state.** `ecs.rs` is a
   minimal Entity Component System: entities are bare IDs, behavior comes from
   which components an entity carries, and "systems" are plain functions that
   query components. The `TypeId + Box<dyn Any>` storage shows how to build a
   heterogeneous, type-safe store without macros or unsafe; real ECS crates
   replace the hash maps with dense arrays for cache-friendly iteration, and
   the module doc says so honestly.

## Files

### physics.rs

`Vec2` is a deliberately tiny value-type vector (Copy, operator overloads via
`Add`/`Sub`/`Mul<f64>`) — do not swap it for a linear-algebra crate; showing the
operator impls is part of the lesson. `step_nbody` is velocity-Verlet in its
standard two-pass form: drift positions with `x + v·dt + ½a·dt²`, recompute
accelerations at the *new* positions, then kick velocities with the *average*
of old and new acceleration. What must survive any edit:

- The two acceleration passes per step. Collapsing them back into a one-pass
  update silently degrades the method to Euler and reintroduces energy drift —
  this exact bug existed here once.
- The `softening` term inside `dist2`, which prevents the force singularity
  (division by ~zero) when bodies get arbitrarily close.
- The Newton's-third-law pairing in `accelerations` (each pair computed once,
  applied equal-and-opposite), which keeps momentum conserved to rounding.

The orbit test asserts *boundedness*, not exact position — the correct way to
test an integrator without over-constraining floating point.

### numerical.rs

Three standalone teaching functions. `integrate_trapezoidal` shows composite
quadrature; its accuracy is controlled by `n`, and the endpoints are
half-weighted — that is the method, not an optimization. `newton_raphson`
returns `Option`: `None` means "did not converge in `max_iter`" or "hit a
zero/non-finite derivative"; keep the `is_finite` bail-out, otherwise a flat
region turns the iteration into NaN churn. Convergence is judged by step size
against `tol` — do not replace it with an equality test. `Mat` is a row-major
dense matrix with the index formula `r * cols + c`; the constructor asserts
`data.len() == rows * cols` and `mul` asserts the inner dimensions match.
Naive triple-loop multiply is intentional — clarity over BLAS.

### ecs.rs

`World` maps `TypeId → (entity id → Box<dyn Any>)`. The API is the whole
lesson: `insert`/`get`/`get_mut` are type-driven via turbofish, `query<T>`
iterates all entities holding a `T`, and downcasting is safe because each inner
map only ever stores boxes of the type its `TypeId` key names — preserve that
invariant if you touch storage. `movement_system` demonstrates the borrow-split
idiom: collect the (Copy) `Velocity` values first, then mutate `Position`,
because you cannot hold a `query` borrow of the world while calling `get_mut`.
Do not "fix" this with unsafe or RefCell; the collect step is the teaching
point. Note that `query` iteration order is HashMap order — unspecified — so
systems and tests must not depend on ordering. There is deliberately no
`remove`/`despawn`; add one only with tests, and keep the API minimal.

## Editing rules

- **Floating-point comparisons need epsilons.** Never `assert_eq!` two computed
  floats; assert `(a - b).abs() < tol` with a justified tolerance, or assert
  bounds/invariants (energy bounded, orbit bounded) instead of exact values.
- **Keep integrators deterministic.** Same inputs must give bit-identical
  outputs: no randomness, no thread-order-dependent accumulation, no iteration
  over HashMap where the order feeds the math.
- **Do not "optimize" clarity out of the math.** The naive matrix multiply, the
  explicit Verlet passes, and the written-out force formula mirror the textbook
  presentation; keep variable names matching the math (`dt`, `dist2`,
  `inv_dist3`, `a0`/`a1`).
- Preserve documented preconditions as asserts (`n > 0`, dimension checks) and
  keep fallible numerics returning `Option`/`Result` rather than panicking.
- New public items need doc comments (missing docs are warnings, and warnings
  are errors in CI) and tests (coverage gate).

## Verification

```bash
cargo test -p simulation
cargo clippy -p simulation --all-targets -- -D warnings
cargo fmt
```
