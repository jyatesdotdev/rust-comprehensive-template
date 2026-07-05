# Architecture

This document describes the workspace layout, crate relationships, build targets, and configuration of the Rust comprehensive template.

## Workspace Layout

```
rust-template/
├── Cargo.toml              # Workspace root — shared deps, lints, profiles
├── rust-toolchain.toml     # Pins stable toolchain + rustfmt, clippy components
├── rustfmt.toml            # Formatting rules
├── clippy.toml             # Lint thresholds (complexity, arg count, line count)
├── deny.toml               # cargo-deny: licenses, advisories, bans, sources
├── Makefile                # Security scanning targets
├── .github/
│   └── workflows/
│       ├── ci.yml          # CI: tests with coverage gate (cargo-llvm-cov, ≥80% lines)
│       └── security.yml    # CI: audit, deny, clippy, geiger, semver-checks, trivy
├── docs/
│   ├── ARCHITECTURE.md             # ← this file
│   ├── TOOLCHAIN.md                # Required tools and editor setup
│   ├── EXTENDING.md                # Adding crates, deps, feature flags
│   ├── TUTORIAL.md                 # New developer walkthrough
│   ├── MEMORY_SAFETY_AND_CONCURRENCY.md
│   ├── SECURITY_SCANNING.md
│   └── cli.md                      # CLI development patterns
└── crates/
    ├── common/             # Shared types and error handling
    ├── api-server/         # Axum REST API + reqwest client
    ├── database/           # sqlx pool, migrations, CRUD repository
    ├── hpc/                # Rayon, tokio async, SIMD, zero-cost abstractions
    ├── etl/                # Iterator chains, parallel batch, streaming pipelines
    ├── systems/            # Unsafe Rust, FFI, manual memory management
    ├── patterns/           # Builder, newtype, typestate, strategy patterns
    ├── simulation/         # Numerical methods, physics, ECS
    ├── testing/            # Unit, property-based, integration tests, benchmarks
    └── cli/                # Clap CLI binary with config, completions, interactive mode
```

## Crate Dependency Graph

```
                    ┌──────────┐
                    │  common  │
                    └────┬─────┘
          ┌──────┬───────┼────────┬──────┬──────┬──────┐
          ▼      ▼       ▼        ▼      ▼      ▼      ▼
     api-server database hpc     etl  systems simulation testing

     ┌──────────┐
     │ patterns │  (no internal deps)
     └──────────┘

     ┌──────────┐
     │   cli    │  (no internal deps)
     └──────────┘
```

- `common` is the foundation crate — all domain crates depend on it for `AppError`, `Result`, and `Entity`.
- `patterns` is standalone — it has zero dependencies (not even `common`).
- `cli` is standalone — it depends only on external crates (clap, figment, etc.).
- No crate depends on another domain crate (flat hierarchy rooted at `common`).

## Libraries vs Binaries

| Crate | Type | Output |
|-------|------|--------|
| `common` | Library | `libcommon` |
| `api-server` | Library | `libapi_server` |
| `database` | Library | `libdatabase` |
| `hpc` | Library + Benchmarks | `libhpc`, Criterion bench `benchmarks` |
| `etl` | Library | `libetl` |
| `systems` | Library | `libsystems` |
| `patterns` | Library | `libpatterns` |
| `simulation` | Library | `libsimulation` |
| `testing` | Library + Benchmarks | `libtesting`, Criterion bench `benchmarks` |
| `cli` | Binary | `demo-cli` |

The only binary in the workspace is `demo-cli` (from `crates/cli/src/main.rs`).

## Feature Flags

The workspace does not define custom feature flags. Dependency features are configured at the workspace level in the root `Cargo.toml`:

- `tokio` — `full`
- `axum` — `macros`
- `reqwest` — `json`
- `tower-http` — `cors`, `trace`
- `sqlx` — `runtime-tokio`, `postgres`, `sqlite`, `migrate`, `chrono`, `uuid`
- `serde` — `derive`
- `tracing-subscriber` — `env-filter`
- `criterion` — `html_reports`
- `clap` — `derive`, `env`
- `figment` — `toml`, `env`
- `uuid` — `v4`, `serde`
- `chrono` — `serde`

## Build Targets

### Cargo Commands

```bash
cargo build                     # Build all crates
cargo build -p cli              # Build only the CLI binary
cargo test --workspace          # Run all tests across all crates
cargo test -p testing           # Run tests for a specific crate
cargo test -p testing proptest  # Run only property-based tests
cargo bench --workspace         # Run all benchmarks (hpc + testing)
cargo bench -p hpc              # Run benchmarks for a specific crate
cargo clippy --workspace        # Run lints
cargo fmt --workspace           # Format code
cargo doc --workspace --no-deps # Generate rustdoc
```

### Makefile Targets

The `Makefile` provides security scanning targets:

| Target | Command | Description |
|--------|---------|-------------|
| `make security` | Runs all below | All security scans |
| `make audit` | `cargo audit` | CVE scan (RustSec advisory DB) |
| `make deny` | `cargo deny check` | License, advisory, ban checks |
| `make geiger` | `cargo geiger --all-features --all-targets` | Unsafe code usage report |
| `make clippy-security` | `cargo clippy ... -- -D warnings` | Clippy with warnings as errors |
| `make semver-checks` | `cargo semver-checks check-release` | API compatibility check |
| `make install-security-tools` | `cargo install ...` | Install all scanning tools |

### CI Pipeline

`.github/workflows/ci.yml` runs on push to `main` and PRs. It executes the full
test suite under `cargo llvm-cov --workspace --fail-under-lines 80` — the build
fails if line coverage drops below 80%, so new code must ship with tests.

`.github/workflows/security.yml` runs on push to `main`, PRs, weekly schedule, and manual dispatch.

**Blocking jobs** (fail the build):
- `audit` — CVE scanning
- `deny` — License/advisory/ban checks
- `clippy-security` — Lint enforcement

**Informational jobs** (continue-on-error):
- `geiger` — Unsafe code report
- `semver-checks` — API compatibility (PR only)

## Configuration Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace members, shared dependencies, lint levels, release profile |
| `rust-toolchain.toml` | Pins `stable` channel with `rustfmt` + `clippy` components |
| `rustfmt.toml` | Formatting rules |
| `clippy.toml` | Lint thresholds: MSRV 1.75, complexity 25, max args 7, max lines 100 |
| `deny.toml` | cargo-deny: allowed licenses, advisory settings, ban rules, source restrictions |

## Workspace Lints

Lint levels are set in `Cargo.toml` under `[workspace.lints.clippy]` and inherited by all crates via `[lints] workspace = true`. Key categories:

- **Baseline:** `all` at `warn` — and because the security CI job runs clippy with `-D warnings`, every warning is a hard error in CI
- **Panic vectors:** `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` at `allow` — deliberately, because `-D warnings` would otherwise reject idiomatic test code and the intentionally-unsafe `systems` crate; library code is still expected to avoid them by convention (see `AGENTS.md`)
- **Arithmetic/type safety:** `arithmetic_side_effects`, `as_conversions` at `allow` for the same reason
- **Hygiene:** `todo`, `unimplemented`, `unreachable`, `dbg_macro` at `warn` (i.e. errors in CI); `print_stdout`/`print_stderr` allowed because the CLI crate prints by design
- **Filesystem security:** `filetype_is_file`, `verbose_file_reads` at `warn`

## Release Profile

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

Optimized for binary size and performance: thin LTO, single codegen unit for maximum optimization, debug symbols stripped.

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough
- [TOOLCHAIN.md](TOOLCHAIN.md) — required tools and editor setup
- [EXTENDING.md](EXTENDING.md) — adding crates, dependencies, feature flags
- [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) — safety patterns and concurrency guide
- [SECURITY_SCANNING.md](SECURITY_SCANNING.md) — security tools and CI integration
- [cli.md](cli.md) — CLI development patterns
