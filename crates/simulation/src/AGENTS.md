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

`rk4_step`/`rk4` are the classical fixed-step Runge-Kutta solver. The four
derivative samples with 1:2:2:1 weighting *are* the method — do not collapse
or "simplify" them. The `rk4_beats_euler_on_exponential_decay` test is the
lesson this code exists for: at the same step size, RK4's O(dt⁴) global error
beats Euler's O(dt) by orders of magnitude, and the test asserts that ratio
(≥1000×) directly. If that assertion ever fails, the integrator is broken —
do not weaken the factor. RK4 is deliberately *not* symplectic; long-run
orbital energy behavior is velocity-Verlet's job in `physics.rs`, and the doc
comments cross-reference the trade-off.

### rng.rs

Deterministic, seedable PCG32 (XSH-RR) plus a Box-Muller `normal` sampler.
Hand-rolled **only** because teaching examples need reproducible sequences
with zero dependencies — production code uses `rand`/`rand_distr`, and the
module doc says so; it is not cryptographically secure. What must survive any
edit:

- **Bit-exact determinism.** Same seed → identical sequence on every machine.
  The multiplier/stream constants, the seeding dance (advance, add seed,
  advance), and the XSH-RR output function come from the PCG reference
  implementation; changing any of them silently invalidates every fixed-seed
  test and every reader's saved results. The increment must stay odd (`| 1`)
  or the LCG loses its full period.
- `next_f64` takes the top 53 bits (`>> 11`) because an f64 mantissa holds
  exactly 53 — that is why outputs land in `[0, 1)` uniformly and 1.0 never
  occurs.
- `normal` caches the spare Box-Muller sample as a *standard* normal and
  rescales at use, so interleaved calls with different mean/std stay correct.
  The `1.0 - next_f64()` shift keeps `u1` in `(0, 1]` — `ln(0)` is `-inf`;
  do not "simplify" it away.

### stats.rs

Descriptive statistics where the API design *is* the lesson: every function
returns `Option`, and undefined statistics (empty input, fewer than two
samples, mismatched lengths, zero-variance correlation) return `None` instead
of panicking or leaking `NaN` into downstream math. What must survive any
edit:

- **Bessel's correction stays.** `variance`/`covariance` divide by `n - 1`
  because deviations are measured from the sample mean, which consumed one
  degree of freedom; dividing by `n` biases the estimate low. The doc comment
  explaining this is part of the teaching content.
- `percentile` interpolates linearly between order-statistic ranks
  (`p/100 · (n-1)`, NumPy's default) and sorts a *copy* — O(n log n) chosen
  over O(n) selection for clarity and a non-mutated input; `median` is
  defined as the 50th percentile, which makes even-length averaging fall out
  for free. The `total_cmp` sort is what keeps NaN inputs from panicking.
- Correlation of a constant series is `None`, not `0.0` — it is 0/0,
  undefined, and papering over that is exactly the bug this module warns
  about.

### interp.rs

`lerp`/`inverse_lerp`/`remap` (degenerate input range → `None`, because the
division is 0-width), uniform Catmull-Rom segment evaluation, and
`LookupTable`, a validated sorted-knot piecewise-linear interpolator. What
must survive any edit:

- Catmull-Rom **passes through** `p1` and `p2` (that interpolating property
  is why it exists vs. Bézier/B-splines) and reproduces straight lines from
  collinear control points; the tests pin both.
- `LookupTable::new` validates once (non-empty, equal lengths, strictly
  increasing xs) so `sample` can be infallible and index/binary-search
  safely — keep validation and sampling in that relationship.
- `sample` **clamps** outside the knot range instead of extrapolating.
  Clamping is the safe default because a table is only trustworthy where it
  was measured; the doc comment's rationale (extrapolated thrust a motor
  cannot produce) is part of the lesson. Do not switch the default to
  extrapolation.

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
- **Statistical tests use fixed seeds and justified tolerances.** The RNG
  tests assert sample mean/std within tolerances chosen from the standard
  error at n = 10k. Never loosen a tolerance to make a "flaky" test pass:
  with a fixed seed these tests are fully deterministic, so a drift means the
  generator or the math changed — find out why before touching the number.
- **Fallible statistics return `None`, never `NaN`.** Any new stats/interp
  function must return `Option` for undefined inputs (empty, too short,
  degenerate range) — and get a test asserting the `None` case.
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
