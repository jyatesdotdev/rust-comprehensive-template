# New Developer Tutorial

A hands-on walkthrough: clone, build, test, run, extend, lint, and scan.

See [TOOLCHAIN.md](TOOLCHAIN.md) for install instructions and [ARCHITECTURE.md](ARCHITECTURE.md) for workspace layout.

## 1. Prerequisites

Install Rust via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The workspace pins its toolchain in `rust-toolchain.toml` — rustup will automatically install the correct version when you first run a cargo command.

## 2. Clone & Build

```bash
git clone <repo-url> && cd rust-template

# Build the entire workspace (debug mode)
cargo build

# Build in release mode (optimized, stripped — see Cargo.toml [profile.release])
cargo build --release
```

A successful build compiles all 10 crates. The `cli` crate produces a binary called `demo-cli`.

## 3. Run Tests

```bash
# Run all tests across the workspace
cargo test --workspace

# Run tests for a single crate
cargo test -p common
cargo test -p testing

# Run only property-based tests
cargo test -p testing proptest

# Run only tests matching a name
cargo test -p hpc parallel
```

## 4. Run the CLI Binary

The `cli` crate builds a binary named `demo-cli`:

```bash
# Run directly via cargo
cargo run -p cli -- --help

# Or build first, then run
cargo build -p cli
./target/debug/demo-cli --help

# Example subcommands (see docs/cli.md for full reference)
cargo run -p cli -- greet Alice
cargo run -p cli -- --verbose greet --shout Alice
cargo run -p cli -- math add 2 3
```

## 5. Run Benchmarks

The `hpc` and `testing` crates include Criterion benchmarks:

```bash
# Run all benchmarks
cargo bench --workspace

# Run benchmarks for a specific crate
cargo bench -p hpc
cargo bench -p testing

# Benchmark results with HTML reports appear in target/criterion/
```

## 6. Generate Documentation

```bash
# Build and open rustdoc in your browser
cargo doc --workspace --no-deps --open
```

Every public item should have `///` doc comments. Module-level docs use `//!`.

## 7. Lint with Clippy

The workspace enables strict lints (see `Cargo.toml` `[workspace.lints.clippy]`):

```bash
# Check all crates
cargo clippy --workspace

# Check with all targets (tests, benches, examples)
cargo clippy --workspace --all-targets

# Promote warnings to errors (CI mode)
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Common clippy fixes:
- Replace `.unwrap()` with `.expect("reason")` or proper error handling
- Add `#[must_use]` to pure functions returning values
- Use `&str` instead of `&String` in function parameters

## 8. Format Code

```bash
# Check formatting (dry run)
cargo fmt --workspace -- --check

# Apply formatting
cargo fmt --workspace
```

Formatting rules are in `rustfmt.toml`.

## 9. Security Scanning

Install the scanning tools once:

```bash
make install-security-tools
# Or manually: cargo install cargo-audit cargo-deny cargo-geiger cargo-semver-checks
```

Run all scans:

```bash
# All scans at once
make security

# Individual scans
make audit              # CVE scan (RustSec advisory DB)
make deny               # License, advisory, and ban checks
make geiger             # Report unsafe code usage
make semver-checks      # API compatibility check
```

See [SECURITY_SCANNING.md](SECURITY_SCANNING.md) for details on each tool.

## 10. Add a Feature — Walkthrough

Let's add a `greet_fancy` function to the `common` crate as a practical exercise.

### Step 1: Edit the source

Open `crates/common/src/lib.rs` and add:

```rust
/// Returns a greeting decorated with emoji.
///
/// # Examples
///
/// ```
/// assert_eq!(common::greet_fancy("World"), "✨ Hello, World! ✨");
/// ```
pub fn greet_fancy(name: &str) -> String {
    format!("✨ Hello, {name}! ✨")
}
```

### Step 2: Verify it compiles

```bash
cargo build -p common
```

### Step 3: Run the doc test

```bash
cargo test -p common --doc
```

### Step 4: Lint

```bash
cargo clippy -p common
```

### Step 5: Format

```bash
cargo fmt -p common
```

You've just added a documented, tested, linted function. For adding entire crates, feature flags, or dependencies, see [EXTENDING.md](EXTENDING.md).

## Quick Reference

| Task | Command |
|------|---------|
| Build all | `cargo build` |
| Test all | `cargo test --workspace` |
| Run CLI | `cargo run -p cli -- --help` |
| Benchmarks | `cargo bench --workspace` |
| Clippy | `cargo clippy --workspace` |
| Format | `cargo fmt --workspace` |
| Docs | `cargo doc --workspace --no-deps --open` |
| Security | `make security` |

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) to understand the crate dependency graph
- Read [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) for Rust safety patterns
- Read [EXTENDING.md](EXTENDING.md) when you're ready to add new crates
- Read [TOOLCHAIN.md](TOOLCHAIN.md) for editor setup (VS Code, RustRover, Neovim)
- Read [SECURITY_SCANNING.md](SECURITY_SCANNING.md) for security tool details and CI integration
- Read [cli.md](cli.md) for CLI development patterns (clap, figment, shell completions)
- Explore individual crate source code — each module has doc comments explaining the patterns it demonstrates
