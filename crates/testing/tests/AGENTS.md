# AGENTS.md — crates/testing/tests

`integration.rs` compiles as a separate crate, so it can only see the public
API of `testing` — that restriction is the lesson. What belongs here versus
in the `#[cfg(test)]` unit modules in `src/`:

- **Here:** behavior that crosses module or crate boundaries — the
  serde_json round-trip of `SortedSet` (exercises the derives through a real
  external format), `merge` combining two sets, math functions composed
  together (gcd of consecutive Fibonacci numbers), and the `#[tokio::test]`
  example of feeding async task results into a collection. If a test only
  needs `pub` items and reads like documentation of how a user combines the
  pieces, it goes here.
- **Not here:** anything needing private internals, per-function edge cases,
  or property-based sweeps — those stay in the unit-test modules next to
  the code so they break close to the change that caused it.

Rules for edits: keep tests deterministic (no sleeps, no timing asserts, no
network, no filesystem); `unwrap`/`expect` are fine in tests; async tests
must be self-contained `#[tokio::test]` functions that terminate without
external coordination.

Verification: `cargo test -p testing` and
`cargo clippy -p testing --all-targets -- -D warnings`.
