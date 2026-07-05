# AGENTS.md — workspace root

Guidance for AI agents working in this repository. Every directory that contains
source files has its own `AGENTS.md` with file-level rationale — **always read the
nearest `AGENTS.md` before editing a file**. This one covers rules that apply to
the whole workspace.

## Why this repository exists

This is a **teaching template**, not a product. Each of the ten crates under
`crates/` demonstrates one area of production Rust (web APIs, databases, HPC,
unsafe/FFI, design patterns, …) in the smallest form that still shows the real
pattern. The code's primary job is to be read and copied from.

Consequences for every edit you make:

- **Clarity beats cleverness.** If a change makes an example harder to read, it
  is wrong even if it is faster or shorter.
- **Preserve the pattern being taught.** Each module exists to demonstrate one
  named technique (see the crate's `src/AGENTS.md`). Do not refactor away the
  technique itself.
- **Keep crates self-contained.** A reader should be able to lift one crate out
  of the workspace. Never add a dependency from one domain crate to another —
  `common` is the only permitted internal dependency, and `patterns`/`cli`
  deliberately depend on nothing internal at all.

## Hard rules — CI rejects violations

1. **Coverage gate:** CI runs `cargo llvm-cov --workspace --fail-under-lines 80`.
   Any new code must ship with tests or the build fails.
2. **Warnings are errors:** the security workflow runs
   `cargo clippy --workspace -- -D warnings`. Anything the workspace lint table
   sets to `warn` (including `todo!`, `unimplemented!`, `dbg!`) breaks CI.
3. **Dependency hygiene:** `cargo audit` and `cargo deny` are blocking jobs.
   Do not add dependencies with open RustSec advisories or restrictive licenses.
   `sqlx` must stay at `>= 0.8` — 0.7 pulled in a vulnerable `rsa` version
   (that CVE's one accepted suppression lives in `.cargo/audit.toml`, which is
   intentionally tracked in git so CI sees it).
4. **MSRV is 1.75, edition 2021** (`rust-toolchain.toml`, `clippy.toml`). Do not
   use language or std features newer than 1.75.
5. **Formatting:** run `cargo fmt --workspace` before you finish; `rustfmt.toml`
   is authoritative.

## Dependency management — why it works this way

All external dependency versions are declared **once** in the root `Cargo.toml`
under `[workspace.dependencies]`; member crates reference them with
`foo = { workspace = true }`. This gives a single upgrade point and stops ten
crates from drifting apart. Never pin a version inside a member crate's
`Cargo.toml`.

## Lint policy — why the surprising `allow`s exist

The workspace lint table (root `Cargo.toml`) *allows* `unwrap_used`,
`expect_used`, `panic`, `indexing_slicing`, and friends. That is **not** an
endorsement: it is because CI's `-D warnings` would otherwise turn idiomatic
test code (where `unwrap` is correct) and the intentionally-unsafe `systems`
crate into hard errors.

The working convention you must follow:

- **In library code paths:** no `unwrap`/`expect`/`panic`. Return
  `common::Result` / `common::AppError` (or the crate's own error type).
- **In `#[cfg(test)]`, examples, and benches:** `unwrap`/`expect` are fine.

## Error handling convention

Layered, and consistent across crates:

- Libraries define precise error enums with `thiserror` (see
  `crates/common/src/error.rs` — `AppError` is the shared one).
- Application edges (binaries, handlers) may use `anyhow` for context.
- `api-server` converts errors to HTTP responses via its `ApiError` newtype —
  never leak internal error text into HTTP bodies from other layers.

## Unsafe code policy

`unsafe_code = "allow"` at the workspace level, but by convention unsafe lives
**only** in `crates/systems` (its whole purpose) and the SIMD intrinsics in
`crates/hpc`. Every `unsafe` block must carry a `// SAFETY:` comment explaining
the invariant that makes it sound. Do not introduce unsafe anywhere else.

## Verification before you finish any change

```bash
cargo fmt --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # or: cargo test -p <crate> while iterating
```

If you touched anything with a benchmark, confirm `cargo bench -p <crate> --no-run`
still compiles.

## Crate map

| Crate | Teaches | Details |
|---|---|---|
| `common` | shared error types, `Result`, `Entity` | `crates/common/src/AGENTS.md` |
| `api-server` | Axum REST API + typed reqwest client | `crates/api-server/src/AGENTS.md` |
| `database` | sqlx pool, migrations, repository pattern | `crates/database/src/AGENTS.md` |
| `hpc` | Rayon, tokio patterns, SIMD, zero-cost abstractions | `crates/hpc/src/AGENTS.md` |
| `etl` | iterator pipelines, parallel batch, async streaming | `crates/etl/src/AGENTS.md` |
| `systems` | unsafe Rust, FFI, manual memory management | `crates/systems/src/AGENTS.md` |
| `patterns` | builder, newtype, typestate, strategy | `crates/patterns/src/AGENTS.md` |
| `simulation` | numerical methods, physics, minimal ECS | `crates/simulation/src/AGENTS.md` |
| `testing` | unit / property / integration tests, Criterion | `crates/testing/src/AGENTS.md` |
| `cli` | clap binary, figment config, completions | `crates/cli/src/AGENTS.md` |

Reference docs for humans live in `docs/` (ARCHITECTURE, TUTORIAL, EXTENDING,
MEMORY_SAFETY_AND_CONCURRENCY, SECURITY_SCANNING, cli). Keep them in sync when
you change behavior they describe.
