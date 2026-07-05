# AGENTS.md — crates/common/src

Read the root `/AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75, workspace-level dependency versions).

## Why this crate exists

`common` is the **only crate other crates are allowed to depend on**. It exists
so the workspace has exactly one definition of "an error" and "an entity"
instead of ten drifting copies. Centralizing `AppError` means every crate's
fallible functions speak the same type, so `?` works across crate boundaries
and the API layer can map one error enum to HTTP statuses. The crate also
teaches the workspace's layered error-handling convention: `thiserror` for
typed domain errors, `anyhow` only at the edges (wrapped inside
`AppError::Internal`), and an extension trait to bridge the two.

Keep this crate tiny and dependency-light. Anything added here becomes a
transitive dependency of nearly every crate in the workspace — that is the
main reason to say no to new modules or dependencies.

## Files

### lib.rs

Exists only to declare the two modules and re-export `AppError`, `Result`,
`ResultExt`, and `Entity` at the crate root so downstream code can write
`use common::{AppError, Result}`. Any new public item that downstream crates
are expected to use must be re-exported here; do not remove or rename the
existing re-exports — other crates import through them.

### error.rs

The heart of the crate. It embodies three decisions that must survive edits:

1. `AppError` is a `thiserror` enum with one variant per failure mode, plus
   `#[from]` conversions for `std::io::Error`, `serde_json::Error`, and
   `anyhow::Error`. Those `#[from]` impls are what make `?` ergonomic
   workspace-wide — removing one breaks compilation in other crates.
2. `is_client_error()` is the single source of truth for "is this the
   caller's fault" — the API layer relies on it for 4xx vs 5xx mapping. A new
   variant must be classified here deliberately.
3. `ResultExt::context_app` is the sanctioned bridge from arbitrary errors
   into `AppError::Internal` with context. It exists so call sites do not
   invent ad-hoc `map_err` chains.

`HumanDuration` exists only so `Timeout` displays as `500ms` / `3.0s` instead
of Debug output; its `Display` format is asserted by tests.

### types.rs

Holds `Entity`, the shared base record (UUID v4 id, name, UTC timestamp). It
exists so database, API, and ETL examples all serialize the same shape. It
must stay `Serialize + Deserialize + Clone + Debug`; field names are part of
the wire/DB contract, so renaming a field silently breaks other crates'
fixtures and docs.

## Editing rules

- Define errors with `thiserror` derive; never hand-implement `Display` for
  error enums. Add a matching `snake_case` constructor when a variant takes a
  `String` (see the existing ones — they take `impl Into<String>`).
- When you add an `AppError` variant: add it to `is_client_error()` (decide
  4xx vs 5xx), add a constructor, and add a `Display` test.
- No `unwrap`/`expect`/`panic` in library code. They are fine inside
  `#[cfg(test)]`. The workspace lint table *allows* them, so clippy will not
  catch you — you must follow the convention yourself.
- Every public item needs a `///` doc comment; module docs use `//!`. Doc
  examples must compile — `cargo test` runs them.
- Tests live co-located in a `#[cfg(test)] mod tests` at the bottom of each
  file. New code needs tests (CI enforces ≥80% line coverage, and doctests do
  not count toward llvm-cov).
- Do not add dependencies to `Cargo.toml` unless truly unavoidable, and if
  you must, reference the workspace version (`foo = { workspace = true }`) —
  never pin a version here.
- Do not make `common` depend on any other workspace crate; the dependency
  arrow points only *into* this crate.
- Footgun: `#[from]` requires each source error type to appear in at most one
  variant. A second variant with `#[from] std::io::Error` will not compile.

## Verification

```bash
cargo test -p common
cargo clippy -p common --all-targets -- -D warnings
cargo fmt
```
