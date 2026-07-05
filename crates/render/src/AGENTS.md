# AGENTS.md — crates/render/src

Read the root `/AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75).

## Why this crate exists

`render` teaches the **geometry of rendering**: how a 3D point becomes a
pixel (the model → world → view → NDC → screen pipeline) and how a pixel
becomes a ray (the same pipeline inverted, which is ray tracing). Everything
is pure `std` + the `math` crate, in `f64`, so every intermediate value can
be printed and checked by hand. Production equivalents to point readers at:
`wgpu` (the GPU owns the raster half of the pipeline), `glam` (the math),
`image` (the file I/O our PPM writer stands in for).

**`render` inherits `math`'s conventions and must never redefine them
locally**: f64 scalars, column-major matrices, right-handed coordinates with
the camera looking down −Z, OpenGL-style `[-1, 1]` NDC. Concretely: never
hand-roll a view or projection matrix here — always go through
`math::look_at_rh` / `math::perspective_rh` / `math::orthographic_rh`. A
locally built matrix that disagrees with `math` by a transpose or a
handedness flip compiles fine and produces mirrored/upside-down images that
are miserable to debug. The only piece of projection math written out in
this crate is `Camera::primary_ray`'s analytic *inverse* (tan(fov/2)
scaling), and its tests pin it against the forward matrices — if you change
one side, the round-trip tests catch the other.

## Files

### geometry.rs

`Ray`, `Hit`, and analytic intersections for `Sphere`, `Aabb`, `Plane`, each
returning the nearest hit with `t > EPSILON`.

The load-bearing invariant: **`Ray.direction` is unit length**, enforced by
`Ray::new` (which returns `Option` because a zero vector cannot be
normalized). Formulas that silently break without it: the sphere quadratic
drops its `a = d·d` term (assumed 1), `t` stops being world-space distance
(so nearest-hit comparisons across objects lie), and `n·l` shading assumes
unit vectors. If you construct a `Ray` literal, you take on the invariant
yourself — `Camera::primary_ray` does, and normalizes first.

Two documented numeric choices, do not "simplify" them away: the sphere uses
the cancellation-free quadratic form (`q = -(h + sign(h)·√disc)` + Vieta)
because the textbook `-h ± √disc` loses precision on grazing/distant rays
(there is a regression test); the AABB slab method guards axis-parallel
directions explicitly instead of leaning on IEEE `1/0 = ∞`, because an
origin exactly on a slab plane produces `0·∞ = NaN`, which poisons min/max
comparisons. `Hit.normal` is the geometric outward normal, never flipped
toward the ray — callers flip if they need to.

### camera.rs

`Camera` = eye/target/up + frustum, with the matrices built **only** via
`math::look_at_rh` and `math::perspective_rh` in `Camera::new` (which
returns `Option` — degenerate frame or frustum → `None`). Fields are private
because the cached basis vectors and matrices must stay consistent.

Forward pipeline: `world_to_ndc` (`None` at-or-behind the camera plane —
behind it the ÷w flips signs and points would reappear mirrored) and
`ndc_to_screen` (the viewport transform; **the Y flip lives here**: NDC +Y
is up, raster row 0 is the top). Inverse pipeline: `primary_ray`. These are
the same bijection run both ways; `project_then_raycast_round_trip` and
`raycast_then_project_returns_the_same_pixel` pin the symmetry. Keep them
passing by construction, not by tweaking tolerances.

### color.rs

`LinearRgb`: f64 channels in **linear light**, the working space where
add/scale/componentwise-multiply are physically meaningful. The one exit to
display space is `to_srgb_u8`, which clamps and applies the *real* piecewise
sRGB transfer function (linear segment below 0.0031308, then the 2.4-power
branch) — not the `pow(1/2.2)` approximation. Do not do color math on
sRGB-encoded values and do not quantize anywhere else; the anchor test
(linear 0.5 → 188, not 128) exists to catch exactly that regression.

### raytrace.rs

The composition proof: `Framebuffer` (row-major, row 0 at top, linear
pixels; `to_ppm` is the single place linear becomes sRGB), `Scene`
(spheres + albedo, one directional `light_dir` *toward* the light,
background), and `render` (primary ray per pixel, nearest hit, Lambertian
`albedo · max(n·l, 0)`). Deliberately minimal — no recursion, shadows, or
sampling. If you extend it, keep shading in `LinearRgb` and keep the
clamped cosine (removing the clamp makes away-facing surfaces subtract
light).

### lib.rs

Crate docs: the pipeline diagram, its ray-tracing inverse, and which stages
GPU hardware owns. Module declarations, root re-exports (new public types
dependents should use must be re-exported here), and the `#[cfg(test)]`
`test_util` module with the crate's `approx_eq` helpers (same tolerance
rationale as `math`).

## Editing rules

- **Never redefine `math`'s conventions locally.** No hand-rolled view or
  projection matrices; go through `math`'s constructors so a convention
  change there breaks loudly here (via the pinned tests), not silently.
- Dependencies: `math` only. No external crates, no `unsafe`, and no
  dependency on any other domain crate (workspace rule).
- No `unwrap`/`expect`/`panic` in library paths. Degenerate input is
  `Option::None` (`Ray::new`, `Plane::new`, `Camera::new`, `world_to_ndc`,
  all intersections) or a documented graceful fallback (`render` with a
  zero `light_dir`, `Framebuffer` out-of-range access). `unwrap` is fine in
  `#[cfg(test)]`.
- Color math stays in `LinearRgb` until the final quantize in `to_srgb_u8`
  — lighting arithmetic is only physically correct in linear space, and
  the sRGB curve must be applied exactly once.
- Uphold `Ray`'s unit-direction invariant when constructing rays directly;
  prefer `Ray::new`.
- Use `math::EPSILON` for zero/near-zero tests; no new ad-hoc thresholds
  without a comment justifying them (test tolerances in `test_util` are the
  documented exception).
- Every public item (including fields) carries a `///` doc comment; doc
  examples compile under `cargo test`.
- Tests are co-located in `#[cfg(test)] mod tests`; new behavior needs a
  test (CI enforces ≥80% line coverage). Compare floats with `test_util`'s
  helpers, never `==` (exact identities excepted).
- Footgun: the Y flip exists in exactly two places (`ndc_to_screen` forward,
  `primary_ray` inverse). Adding a third, or removing one, renders images
  upside down while every unit test of the *other* direction still passes —
  rely on the round-trip tests.

## Verification

```bash
cargo fmt -p render
cargo test -p render
cargo clippy -p render --all-targets -- -D warnings
```
