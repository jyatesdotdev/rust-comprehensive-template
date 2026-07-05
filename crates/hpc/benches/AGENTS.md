# AGENTS.md — crates/hpc/benches

`benchmarks.rs` exists to make the crate's performance claims *observable*:
scalar vs SSE dot product (is the unsafe worth it?), parallel sort throughput,
and `generic_sum` (proof that the generic compiles to loop speed). Criterion
is used because it gives statistically meaningful comparisons; the numbers are
the teaching payload, not the code.

Rules:

- Benches must always compile: CI-relevant check is
  `cargo bench -p hpc --no-run`. Run it after any change to `hpc`'s public
  API or to this file.
- Input sizes (1024 elements, 10k elements) are chosen to show a measurable
  signal while keeping runs fast. Do not inflate them for "realism".
- Wrap inputs in `black_box` so the compiler cannot const-fold the work away;
  use `iter_batched` when the benched function mutates its input (see the
  sort bench).
- Keep the `#[cfg(target_arch = "x86_64")]` gate on the SSE bench and its
  `// SAFETY:` comment on the unsafe call.
- Benches are excluded from the no-`unwrap`/`expect` convention — panicking
  setup code is fine here.
