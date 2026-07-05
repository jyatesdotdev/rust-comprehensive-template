# AGENTS.md — render/examples

Read `../src/AGENTS.md` and the root `AGENTS.md` first.

## Why this directory exists

`nbody.rs` is the workspace's proof that domain crates **compose**: it drives
`simulation`'s velocity-Verlet N-body integrator and feeds each step through
`render`'s camera + ray tracer, writing an animation as PPM frames. That
cross-crate wiring is only legal here because examples run on
`[dev-dependencies]` — the root AGENTS.md sanctions sibling domain crates in
dev-deps for composition showcases, while library `[dependencies]` between
domain crates stay forbidden. Do not "promote" the simulation dependency out
of dev-deps to make an example more convenient.

## Editing rules

- Keep the example finishing in seconds (`cargo run -p render --example
  nbody --release`); it is a demo, not a benchmark. If you raise frame count
  or resolution, check the wall time.
- Frames go to `target/nbody-frames/` **only** — `target/` is gitignored, so
  the example can never pollute the repository. Never write output anywhere
  else.
- The core simulate→render loop is duplicated in miniature by
  `../tests/nbody_compose.rs` so it counts toward the coverage gate; if you
  change the loop's structure here, mirror the change there.
- Examples are exempt from the no-`unwrap`/no-print conventions (they are
  application edges), but keep physics parameters deterministic — a reader
  should get the same orbit every run.

## Verification

```bash
cargo clippy -p render --all-targets -- -D warnings   # compiles examples too
cargo run -p render --example nbody --release
```
