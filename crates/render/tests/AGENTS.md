# AGENTS.md — render/tests

Read `../src/AGENTS.md` and the root `AGENTS.md` first.

## Why this directory exists

`nbody_compose.rs` runs the same simulate→render loop as
`../examples/nbody.rs`, but tiny (a few frames, low resolution) and entirely
**in memory** — no file I/O. It exists for two reasons:

1. Examples are never executed by `cargo test`, so without this test the
   crate-composition loop would be dead code to CI and to the 80% coverage
   gate.
2. It asserts the thing the example only shows visually: the set of lit
   pixels *changes* between frames, i.e. motion actually propagates from
   `simulation` through the camera to the framebuffer.

It uses `simulation` via `[dev-dependencies]` under the same root-AGENTS.md
composition exception as the example.

## Editing rules

- Keep it hermetic and fast: no filesystem writes, no timing dependence,
  deterministic initial conditions. Assert on pixel-set differences, not on
  exact pixel values — shading details may legitimately evolve; motion is
  the invariant under test.
- If the example's core loop changes shape, update this test to match, and
  vice versa — they are two renderings of the same lesson.

## Verification

```bash
cargo test -p render --test nbody_compose
```
