# Security Scanning

This project integrates five security scanning tools into the build process. Three block CI on failure (cargo-audit, cargo-deny, clippy); two are informational (cargo-geiger, cargo-semver-checks).

## Quick Start

```bash
# Install all tools
make install-security-tools

# Run all scans
make security

# Run individually
make audit            # CVE scanning
make deny             # Licenses, advisories, bans
make geiger           # Unsafe code report
make clippy-security  # Security-focused lints
make semver-checks    # API compatibility
```

## Tools

### cargo-audit — CVE Scanning

Checks dependencies against the [RustSec Advisory Database](https://rustsec.org/).

```bash
cargo audit
```

Fails on any known vulnerability. The advisory database is fetched from `~/.cargo/advisory-db` and updated automatically.

### cargo-deny — License, Advisory, and Ban Checks

Configured via `deny.toml` at the workspace root. Runs four checks:

| Check | What it does | Failure mode |
|-------|-------------|--------------|
| `advisories` | RustSec CVE scan (overlaps with cargo-audit) | Deny on vulnerabilities, warn on unmaintained/yanked |
| `licenses` | Validates dependency licenses against allowlist | Deny on unlicensed or copyleft |
| `bans` | Detects duplicate versions, banned crates | Warn on duplicates |
| `sources` | Ensures crates come from crates.io only | Deny on unknown registries/git |

```bash
cargo deny check              # All checks
cargo deny check advisories   # Single check
```

**Allowed licenses:** MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-DFS-2016, Unicode-3.0, OpenSSL, BSL-1.0, CC0-1.0, MPL-2.0.

### cargo-geiger — Unsafe Code Reporting

Reports `unsafe` usage across the dependency tree. Informational only — does not fail the build.

```bash
cargo geiger --all-features --all-targets
```

### Clippy Security Lints

Workspace-level clippy lints are configured in `Cargo.toml` under `[workspace.lints.clippy]`. All workspace crates inherit these via `[lints] workspace = true`. Key security lints:

| Category | Lints |
|----------|-------|
| Panic vectors | `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `unreachable` |
| Arithmetic/type safety | `arithmetic_side_effects`, `as_conversions`, `lossy_float_literal` |
| Unsafe hygiene | `undocumented_unsafe_blocks` (deny), `multiple_unsafe_ops_per_block`, `mem_forget` |
| Production hygiene | `dbg_macro`, `print_stdout`, `print_stderr`, `exit` |

Thresholds are in `clippy.toml` (complexity, argument count, line count).

### cargo-semver-checks — API Compatibility

Detects accidental breaking changes in public APIs. Runs on PRs only (needs a published baseline to compare against). Informational — does not block CI.

```bash
cargo semver-checks check-release
```

## CI Integration

The GitHub Actions workflow (`.github/workflows/security.yml`) runs on:
- Push to `main`
- Pull requests to `main`
- Weekly schedule (Mondays 06:00 UTC) for advisory DB freshness
- Manual dispatch

**Blocking jobs** (fail the build): `audit`, `deny`, `clippy-security`
**Informational jobs** (continue-on-error): `geiger`, `semver-checks`

## Adding Exceptions

### Ignore a specific CVE (cargo-deny)

In `deny.toml` under `[advisories]`:

```toml
[advisories]
ignore = [
    "RUSTSEC-2024-0001",  # reason: not exploitable in our usage
]
```

### Allow a non-standard license (cargo-deny)

Add a per-crate exception in `deny.toml`:

```toml
[[licenses.exceptions]]
name = "some-crate"
allow = ["LGPL-3.0"]  # reason: used only as dynamic library
```

### Ban a crate (cargo-deny)

```toml
[bans]
deny = [
    { crate = "openssl-sys", reason = "use rustls instead" },
]
```

### Skip duplicate version warnings (cargo-deny)

```toml
[bans]
skip = [
    { crate = "bitflags@1.3.2", reason = "transitive dep not yet updated" },
]
```

### Allow unsafe in a specific block (clippy)

```rust
// SAFETY: pointer is guaranteed non-null by the allocator contract
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe { ... }
```

### Suppress a clippy lint for a function

```rust
#[allow(clippy::unwrap_used)]  // reason: infallible parse of static string
fn parse_static() -> Config {
    toml::from_str(STATIC_CONFIG).unwrap()
}
```

## File Reference

| File | Purpose |
|------|---------|
| `deny.toml` | cargo-deny configuration |
| `clippy.toml` | Clippy thresholds |
| `Cargo.toml` | Workspace lint levels (`[workspace.lints.clippy]`) |
| `Makefile` | `make security` and individual targets |
| `.github/workflows/security.yml` | CI workflow |

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough including security scanning steps
- [ARCHITECTURE.md](ARCHITECTURE.md) — workspace layout, CI pipeline, and Makefile targets
- [TOOLCHAIN.md](TOOLCHAIN.md) — required tools and install instructions
- [EXTENDING.md](EXTENDING.md) — adding cargo-deny exceptions when extending the workspace
- [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) — safety patterns and unsafe Rust guide
- [cli.md](cli.md) — CLI development patterns
