# Rust Comprehensive Template

A Cargo workspace showcasing memory safety, performance, and modern systems programming in Rust. Nine crates cover web APIs, databases, high-performance computing, ETL pipelines, systems programming, design patterns, simulations, and testing — each with working examples and tests.

## Quick Start

```bash
# Build the entire workspace
cargo build

# Run all tests
cargo test --workspace

# Run benchmarks (hpc + testing crates)
cargo bench --workspace

# Check lints
cargo clippy --workspace

# Format code
cargo fmt --workspace
```

**Requirements:** Rust stable ≥ 1.75 (see `rust-toolchain.toml`)

## Workspace Structure

```
rust-template/
├── Cargo.toml                  # Workspace root with shared dependencies
├── rust-toolchain.toml         # Pinned stable toolchain
├── rustfmt.toml                # Formatting rules
├── clippy.toml                 # Lint configuration
├── docs/
│   └── MEMORY_SAFETY_AND_CONCURRENCY.md
└── crates/
    ├── common/          # Shared error types, Result, Entity
    ├── api-server/      # Axum REST API + reqwest client
    ├── database/        # sqlx connection pool, migrations, CRUD repo
    ├── hpc/             # Rayon, tokio async, SIMD, zero-cost abstractions
    ├── etl/             # Iterator chains, parallel batch, streaming pipelines
    ├── systems/         # Unsafe Rust, FFI, manual memory management
    ├── patterns/        # Builder, newtype, typestate, strategy patterns
    ├── simulation/      # Numerical methods, physics, ECS
    └── testing/         # Unit, integration, property-based tests, benchmarks
```

## Crates

### `common` — Shared Foundation

Custom `AppError` enum with `thiserror` derivations, `anyhow` integration, `ResultExt` trait for adding context, and a shared `Entity` type used across crates.

### `api-server` — RESTful APIs

- **Server:** Axum router with CRUD handlers, `State`/`Json`/`Path`/`Query` extractors, Tower middleware stack (tracing, CORS, request-ID), graceful shutdown
- **Client:** `ApiClient` wrapping `reqwest::Client` with connection pooling and typed responses
- **Error:** `ApiError` newtype implementing `IntoResponse` for automatic HTTP status mapping

### `database` — Database Interaction

- **Pool:** Configurable `PoolConfig` → `SqlitePool` via `sqlx`
- **Migrations:** File-based SQL migration runner
- **Repository:** `EntityRepo` with create, find, list (paginated), update, delete, and transactional batch insert

### `hpc` — High Performance Computing

| Module | What it demonstrates |
|---|---|
| `parallel` | Rayon `par_iter`, fold/reduce, custom thread pools |
| `async_runtime` | Tokio fan-out/fan-in, mpsc, oneshot, `select!` |
| `simd` | Auto-vectorized dot product, manual SSE intrinsics |
| `zero_cost` | Monomorphized generics, newtype units, fused iterators |

Includes Criterion benchmarks (`cargo bench -p hpc`).

### `etl` — ETL / Data Processing

- **pipeline** — Composable `Stage` trait with `Chain`, `map`/`filter` constructors
- **iterators** — CSV parsing, `group_sum`, `running_average`, `top_n`
- **parallel** — `par_map_reduce`, `par_group_sum`, chunked batch processing
- **streaming** — Async pipelines with bounded channels and fan-out workers

### `systems` — Systems Programming

- **unsafe_rust** — `raw_swap`, `RawStack` (manual alloc/dealloc/Drop), `DeepSizeOf` trait
- **ffi** — libc wrappers (`getpid`, `getenv`, `page_size`), `extern "C"` exports, closure-to-C trampoline
- **memory** — Arena bump allocator, RAII `Guard` with disarm, `HeapVal` (manual Box)

### `patterns` — Design Patterns

- **builder** — Type-safe builder with `PhantomData` compile-time required-field enforcement
- **newtype** — `Email` validation, generic `Id<T>`, `Meters`/`Kilometers` unit wrappers
- **typestate** — `Connection` state machine (Disconnected → Connected → Authenticated) encoded in types
- **strategy** — `Compressor` trait with dynamic dispatch and enum dispatch alternatives

### `simulation` — Numerical & Physics

- **numerical** — Trapezoidal integration, Newton-Raphson root finding, dense matrix multiply
- **physics** — `Vec2` with operator overloads, `Body` struct, velocity-Verlet N-body gravity
- **ecs** — Minimal Entity Component System with `TypeId`+`Any` sparse storage, `World`, systems

### `testing` — Testing & Benchmarking

- Co-located unit tests (9 tests)
- Property-based tests with `proptest` (7 properties)
- Integration tests (4 tests, including async)
- Criterion benchmarks with parameterized groups

```bash
# Run only property-based tests
cargo test -p testing proptest

# Run benchmarks with HTML reports
cargo bench -p testing
```

## Key Concepts Demonstrated

### Memory Safety
Ownership, borrowing, lifetimes, `Drop` implementations, arena allocation, and safe abstractions over unsafe code. See [`docs/MEMORY_SAFETY_AND_CONCURRENCY.md`](docs/MEMORY_SAFETY_AND_CONCURRENCY.md).

### Error Handling
Layered strategy: `thiserror` for library errors, `anyhow` for application context, `ResultExt` for ergonomic chaining, and `ApiError` for HTTP response mapping.

### Async / Concurrency
Tokio runtime patterns (fan-out, channels, select), Rayon parallel iterators, `Arc<Mutex<T>>` shared state, and async streaming with backpressure.

### Performance
Zero-cost abstractions, SIMD intrinsics, Criterion benchmarks, `lto = "thin"` + `codegen-units = 1` release profile, and iterator fusion.

## Configuration

Workspace-level settings in the root `Cargo.toml`:

- **Edition:** 2021
- **MSRV:** 1.75
- **Release profile:** `opt-level = 3`, thin LTO, single codegen unit, stripped binaries
- **Lints:** Clippy `warn` on `all`, `pedantic`, `nursery`; `warn` on `unwrap_used`/`expect_used`

## Documentation

```bash
# Generate and open rustdoc for the workspace
cargo doc --workspace --no-deps --open
```

| Document | Description |
|----------|-------------|
| [TUTORIAL.md](docs/TUTORIAL.md) | New developer walkthrough — clone, build, test, run, extend, lint, scan |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Workspace layout, crate dependency graph, build targets, configuration |
| [TOOLCHAIN.md](docs/TOOLCHAIN.md) | Required tools, install instructions, editor setup |
| [EXTENDING.md](docs/EXTENDING.md) | Adding crates, dependencies, feature flags, cargo-deny exceptions |
| [MEMORY_SAFETY_AND_CONCURRENCY.md](docs/MEMORY_SAFETY_AND_CONCURRENCY.md) | Memory safety patterns, concurrency, and unsafe Rust guide |
| [SECURITY_SCANNING.md](docs/SECURITY_SCANNING.md) | Security tools: cargo-audit, cargo-deny, cargo-geiger, clippy lints |
| [cli.md](docs/cli.md) | CLI development patterns: clap, figment, shell completions, testing |

## License

MIT
