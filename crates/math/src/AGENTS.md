# AGENTS.md — crates/math/src

Read the root `/AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75).

## Why this crate exists

`math` is a hand-rolled, pure-`std` linear algebra library — the math that
`glam` and `nalgebra` hide behind SIMD and macros, written out longhand so it
can be read against a textbook. It is one of only **two crates other crates
may depend on** (the other is `common`); the forthcoming `render` crate
builds its entire camera/clipping pipeline on it.

That makes the crate's conventions **load-bearing contracts**, not style
choices. Every dependent assumes:

- **All scalars are `f64`** — no `f32` anywhere.
- **Matrices are column-major**: `cols[c][r]` is row `r` of column `c`;
  matrices multiply column vectors on the right (`M * v`).
- **Right-handed coordinates**, camera looks down **−Z** in view space.
- **Projections target OpenGL-style `[-1, 1]` NDC** on all three axes
  (not Vulkan/D3D `[0, 1]` depth).

Changing any of these compiles fine and silently breaks `render` — every
transform starts behaving like its own transpose, or depth clipping goes
wrong at a distance. If a convention genuinely must change, change it loudly:
crate docs, this file, and every dependent in the same commit.

Zero external dependencies (criterion is dev-only, for benches). No `unsafe`.

## Files

### lib.rs

Crate docs stating the conventions above, module declarations, root
re-exports of the main types, and `EPSILON` — the single crate-wide
"treat as zero" threshold used by `normalize()` and `inverse()`. Keep the
threshold in one place; do not scatter per-file magic epsilons. New public
types dependents should use must be re-exported here.

### vec.rs

`Vec2`/`Vec3`/`Vec4` with the std::ops the algebra needs, dot/cross,
length, `normalize() -> Option` (None below `EPSILON` — a near-zero vector
has no direction), lerp. The three types are deliberately written out
longhand, not macro-generated: the repetition is readable, a macro is not.
`Vec3::cross` follows the right-hand rule (`X × Y = Z`) — a test guards it;
flipping the sign re-hands the whole crate. Also holds the `#[cfg(test)]`
`approx_eq` helper every test module uses: never compare floats with `==`
except for exact identities.

### mat.rs

Column-major `Mat3`/`Mat4`. The #1 footgun in this crate: `cols[0]` is the
first **column**, never the first row — `Mat4`'s translation lives in
`cols[3]`. `Mul` is implemented as "column j of A·B = A · (column j of B)",
which only reads correctly because storage is column-major. Determinant and
inverse use Laplace/adjugate expansion via a shared `minor()` helper — chosen
for readability, not speed; do not "optimize" them into opaque unrolled
formulas. `inverse()` returns `None` when `|det| < EPSILON`.
`transform_point3` (affine, w=1, no divide) vs `project_point3` (projective,
divides by w, returns `Option`) is a deliberate split — collapsing them hides
the perspective divide the file exists to teach.

### quat.rs

Unit quaternions. The unit-length invariant is upheld by constructors and
documented on `new()`; `rotate_vec3` and `to_mat3` assume it. Two subtleties
with dedicated tests: the Hamilton product composes right-to-left like
matrices, and `slerp` must handle the double cover (`q` and `-q` are the
same rotation → negate one operand when `dot < 0`) plus the near-parallel
case (fall back to nlerp above `SLERP_DOT_THRESHOLD`, because slerp divides
by `sin θ → 0`). Removing either branch passes casual tests and fails in
production — keep both and their tests. `to_mat3` must stay in exact
agreement with `rotate_vec3`; a test asserts it.

### transform.rs

TRS `Transform` (baking to `T·R·S` — scale first, translate last; other
orders shear or orbit), `look_at_rh` (returns `Option`: `None` for
eye == target or up ∥ forward), and the two `_rh` projections. This file is
where the handedness/NDC contract is most concentrated: the `-1.0` in
`perspective_rh`'s third column *is* the right-handed perspective divide,
and the depth rows are tuned for `[-1, 1]` NDC. Tests pin the contract
(eye → origin, target → −Z axis, near/far → ∓1); if you touch a matrix here
and a convention test fails, the fix is your matrix, not the test.

## Editing rules

- **Do not change the conventions** (f64, column-major, right-handed, −Z
  forward, `[-1, 1]` NDC). They are contracts for `render` and any future
  dependent; the compiler will not catch a violation, only downstream
  geometry bugs will.
- Zero runtime dependencies and no `unsafe`. `criterion` stays dev-only.
- This crate must depend on no other workspace crate — foundation crates
  sit at the bottom of the dependency graph (`math` and `common` are the
  only two others may depend on).
- No `unwrap`/`expect`/`panic` in library paths. Degenerate numeric input
  is an `Option::None` (`normalize`, `inverse`, `look_at_rh`,
  `project_point3`) or a documented identity fallback (`from_axis_angle`
  with a zero axis) — follow that pattern for new API.
- Use `EPSILON` from `lib.rs` for zero tests; no exact `==` against 0.0 and
  no new ad-hoc thresholds without a comment justifying them.
- Every public item (including fields) carries a `///` doc comment; doc
  examples compile under `cargo test`.
- Tests are co-located in `#[cfg(test)] mod tests`; new behavior needs a
  test (CI enforces ≥80% line coverage). Compare floats with the `approx_eq`
  helper — exact equality only for exact identities (e.g. `I * m == m`).
- Footgun: when writing a matrix literal remember you are writing *columns*.
  A matrix from a textbook (written in rows) must be entered transposed.
- Footgun: `Default` for matrices/quaternions/`Transform` is identity, not
  zero — deriving `Default` instead of the manual impls would silently
  change it to zero and erase geometry.

## Verification

```bash
cargo fmt -p math
cargo test -p math
cargo clippy -p math --all-targets -- -D warnings
cargo bench -p math --no-run
```
