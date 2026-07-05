# AGENTS.md — crates/testing/benches

`benchmarks.rs` demonstrates Criterion conventions; the numbers matter less
than the structure. Criterion is wired via `[[bench]] name = "benchmarks"`
with `harness = false` in `Cargo.toml` — do not remove that, or `cargo
bench` silently runs the (useless) built-in harness instead.

Conventions to preserve:

- Wrap every benchmarked input and any consumed output in `black_box` so
  the optimizer cannot delete the work being measured.
- Use a `benchmark_group` + `BenchmarkId::from_parameter` when sweeping an
  input size (see `fibonacci` and `sorted_set_insert`); use a plain
  `bench_function` with the concrete input in its name for single cases
  (see `gcd`). Always `group.finish()`.
- Do setup (building inputs) outside `b.iter(...)`; only the operation under
  measurement goes inside the closure. Returning the built value from the
  closure (as `sorted_set_insert` does) is the idiomatic way to keep it
  alive without `black_box` gymnastics.
- Register new benchmarks in `criterion_group!`/`criterion_main!` or they
  will never run. Keep inputs small enough that a full `cargo bench` stays
  in seconds, not minutes.
- Benches may use `unwrap`/`expect` freely, but must only touch the crate's
  public API.

CI never executes benchmarks, but they must always **compile**:

```bash
cargo bench -p testing --no-run
```

Also run `cargo fmt` and
`cargo clippy -p testing --all-targets -- -D warnings` (clippy's
`--all-targets` covers benches) before finishing.
