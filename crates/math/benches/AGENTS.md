# AGENTS.md — crates/math/benches

`benchmarks.rs` measures the crate's per-frame hot paths — `Mat4 * Mat4`
(MVP chains), `Vec3::normalize` (lighting), and `Quat::slerp` (animation
blending) — so the cost of hand-rolled f64 math is observable next to what a
SIMD f32 library like `glam` would report. The numbers are the teaching
payload, not the code.

Rules:

- Benches must always compile: the CI-relevant check is
  `cargo bench -p math --no-run`. Run it after any change to `math`'s public
  API or to this file.
- Wrap all inputs in `black_box` so the compiler cannot const-fold the pure
  math away — every function here is a deterministic function of constants,
  so without `black_box` the benchmark measures nothing.
- Keep operands realistic but tiny (single matrices/quaternions, not
  arrays): these ops are O(1), and batching would only blur the signal.
- The slerp operands are deliberately far apart so the true slerp path runs,
  not the nlerp fallback — do not shrink the angle.
- Benches are excluded from the no-`unwrap`/`expect` convention — panicking
  setup code is fine here.
