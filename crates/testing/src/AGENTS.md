# AGENTS.md — crates/testing/src

Read the root `AGENTS.md` first for workspace-wide rules.

## Why this crate exists

This crate's subject matter is the testing pyramid itself. The library code
(`math`, `collections`) is intentionally small and boring — it exists to be
tested, not to be useful. What the crate actually teaches is where each kind
of test lives and why:

- **Unit tests** sit in `#[cfg(test)]` modules next to the code they test,
  because they are allowed to know implementation details and should break
  the moment the module they live beside changes meaning.
- **Property-based tests (proptest)** sit alongside the example-based unit
  tests to show that the two are complements, not alternatives: examples
  pin down specific known answers (`fibonacci(10) == 55`), while properties
  pin down invariants over the whole input space (`clamp` output is always
  in range, `gcd` divides both arguments). Properties catch the edge cases
  nobody thought to write an example for; examples catch the bugs where the
  property itself was stated wrong.
- **Integration tests** (`tests/`) compile as a separate crate and can only
  touch the public API — see `tests/AGENTS.md`.
- **Criterion benchmarks** (`benches/`) live in this crate because
  measurement is part of the same discipline as testing: both are executable
  claims about the code. See `benches/AGENTS.md`.

Keeping all four layers over the same two tiny modules is the point: a
reader can compare them side by side. Do not grow the library code beyond
what the tests need as subject matter.

## Files

### lib.rs

Two-line module list plus crate docs. It stays this small deliberately —
the crate root should tell a reader immediately that the interesting content
is the tests, not the API.

### math.rs

Pure functions (`clamp`, `gcd`, `fibonacci`) chosen because they have crisp,
classroom-checkable properties. The unit-test module below them demonstrates
the layout convention: example-based tests grouped per function, then a
nested `proptests` module. `fibonacci` uses `wrapping_add` and documents the
wrap past `n = 93` — if you change the overflow behavior, the doc comment,
the monotonicity property's input range (`1..80`), and the benchmarks all
depend on it. Property strategies use bounded ranges so failures shrink to
small, readable counterexamples; keep new strategies bounded too.

### collections.rs

`SortedSet<T>`, a `Vec`-backed sorted set, exists to demonstrate testing a
stateful data structure: example tests for the tricky single behaviors
(duplicate insert returns `false`), properties for the invariants that must
survive any operation sequence (slice always sorted, membership after
insert, len equals unique count). The internal `Vec` staying sorted is the
invariant every method relies on (`binary_search` in `insert`/`contains`) —
any new method must preserve it and get a property test saying so. The
serde derives are used by the integration tests' round-trip test; do not
remove them.

## Editing rules

- New library code here needs all applicable layers: unit tests, at least
  one property test if it has a stateable invariant, and a benchmark if it
  is performance-relevant. Code without tests defeats the crate's purpose.
- Write proptest strategies that shrink well: prefer bounded numeric ranges
  and `prop::collection::vec(elem, 0..N)` over unbounded `any::<T>()`, so a
  failing case minimizes to something a human can read.
- Keep every test deterministic: no timing assertions, no network, no
  filesystem, no randomness outside proptest's managed generators.
- `unwrap`/`expect` are fine inside test modules; the library functions
  themselves must stay panic-free on all inputs (note `clamp` does not
  assert `min <= max` — if you add such a contract, make it a documented
  error, not a panic).
- Dependencies used only by tests or benches belong in `[dev-dependencies]`
  (as `proptest`, `serde_json`, `criterion`, and `tokio` are), and always
  via `workspace = true`.

## Verification

```bash
cargo fmt
cargo clippy -p testing --all-targets -- -D warnings
cargo test -p testing
cargo bench -p testing --no-run
```

Coverage gate: CI enforces ≥80% workspace line coverage; this crate should
sit far above that, given its subject.
