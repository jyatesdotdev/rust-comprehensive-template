# Extending the Workspace

How to add crates, dependencies, feature flags, and cargo-deny exceptions.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the current workspace layout and [TOOLCHAIN.md](TOOLCHAIN.md) for required tools.

## Adding a New Library Crate

1. Create the crate directory and files:

```bash
mkdir -p crates/my-crate/src
```

2. Create `crates/my-crate/Cargo.toml`:

```toml
[package]
name = "my-crate"
version.workspace = true
edition.workspace = true

[lints]
workspace = true

[dependencies]
# Add workspace deps as needed:
# serde.workspace = true
# To depend on the common crate:
# common = { path = "../common" }
```

3. Create `crates/my-crate/src/lib.rs`:

```rust
//! My crate — brief description of what it does.
```

4. Register it in the root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/my-crate",
]
```

5. Verify: `cargo check -p my-crate`

## Adding a New Binary Crate

Same as a library crate, but add a `[[bin]]` section and a `main.rs`:

```toml
# crates/my-tool/Cargo.toml
[package]
name = "my-tool"
version.workspace = true
edition.workspace = true

[[bin]]
name = "my-tool"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
clap.workspace = true
```

```rust
// crates/my-tool/src/main.rs
fn main() {
    println!("hello");
}
```

Don't forget to add `"crates/my-tool"` to `[workspace] members` in the root `Cargo.toml`.

## Adding a Binary to an Existing Library Crate

Add a `[[bin]]` section to the crate's `Cargo.toml` and create the source file:

```toml
[[bin]]
name = "my-binary"
path = "src/bin/my-binary.rs"
```

The crate keeps its `src/lib.rs` as the library entry point. The binary can `use` the library:

```rust
// crates/my-crate/src/bin/my-binary.rs
use my_crate::SomeType;

fn main() {
    // ...
}
```

## Adding a Dependency

### Workspace-level (shared across crates)

1. Add the dependency to the root `Cargo.toml` under `[workspace.dependencies]`:

```toml
[workspace.dependencies]
rand = "0.8"
```

2. Reference it in the crate's `Cargo.toml`:

```toml
[dependencies]
rand.workspace = true
```

### With features at the workspace level

```toml
# Root Cargo.toml
[workspace.dependencies]
rand = { version = "0.8", features = ["small_rng"] }
```

All crates that use `rand.workspace = true` get those features.

### Crate-local dependency

If only one crate needs a dependency, you can add it directly without the workspace indirection:

```toml
# crates/my-crate/Cargo.toml
[dependencies]
tempfile = "3"
```

This is the pattern used for `tempfile` in the `cli` crate's `[dev-dependencies]`.

### Dev-dependencies and build-dependencies

Same pattern — use `[dev-dependencies]` or `[build-dependencies]` sections:

```toml
[dev-dependencies]
proptest.workspace = true
```

## Adding a Feature Flag

### On your own crate

```toml
# crates/my-crate/Cargo.toml
[features]
default = []
postgres = ["sqlx/postgres"]
sqlite = ["sqlx/sqlite"]
```

Use `cfg` attributes in code:

```rust
#[cfg(feature = "postgres")]
pub mod postgres_backend;
```

### Enabling features on a workspace dependency per-crate

If the workspace declares a dependency without a feature you need in one crate, you can add it locally:

```toml
# crates/my-crate/Cargo.toml
[dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

This merges with the workspace-level features. The `testing` crate uses this pattern for tokio's `test-util` feature.

## Adding a Benchmark

1. Add `criterion` as a dev-dependency:

```toml
[dev-dependencies]
criterion.workspace = true
```

2. Add a `[[bench]]` section:

```toml
[[bench]]
name = "benchmarks"
harness = false
```

3. Create `benches/benchmarks.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn my_benchmark(c: &mut Criterion) {
    c.bench_function("example", |b| {
        b.iter(|| {
            // code to benchmark
        });
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```

4. Run: `cargo bench -p my-crate`

## Adding a cargo-deny Exception

All exceptions go in `deny.toml` at the workspace root.

### Allow a specific license for one crate

```toml
# deny.toml
[[licenses.exceptions]]
name = "some-crate"
allow = ["LGPL-3.0"]  # reason: used only as dynamic library
```

### Ignore a security advisory

```toml
# deny.toml
[advisories]
ignore = [
    "RUSTSEC-2024-XXXX",  # reason for ignoring
]
```

### Allow a banned or duplicate crate

```toml
# deny.toml
[bans]
deny = [
    { crate = "openssl-sys", reason = "use rustls instead" },
]
skip = [
    { crate = "bitflags@1.3.2", reason = "transitive dep not yet updated" },
]
```

### Allow a git source

```toml
# deny.toml
[sources]
allow-git = [
    "https://github.com/example/some-crate",
]
```

After any change, verify with `cargo deny check`.

## Checklist for New Crates

- [ ] Created `crates/<name>/Cargo.toml` with `version.workspace = true`, `edition.workspace = true`, `[lints] workspace = true`
- [ ] Added to `[workspace] members` in root `Cargo.toml`
- [ ] Added `//!` module-level doc comment in `lib.rs` or `main.rs`
- [ ] Added `///` doc comments on all public items
- [ ] `cargo check -p <name>` passes
- [ ] `cargo clippy -p <name>` passes
- [ ] `cargo test -p <name>` passes

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough
- [ARCHITECTURE.md](ARCHITECTURE.md) — workspace layout and crate dependency graph
- [TOOLCHAIN.md](TOOLCHAIN.md) — required tools and editor setup
- [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) — safety patterns and concurrency guide
- [SECURITY_SCANNING.md](SECURITY_SCANNING.md) — security tools and cargo-deny configuration
- [cli.md](cli.md) — CLI development patterns
