# Toolchain & Tools

Required tools and editor setup for working with this workspace.

## Rust Toolchain

The workspace pins its toolchain in `rust-toolchain.toml`:

| Setting | Value |
|---------|-------|
| Channel | `stable` |
| MSRV | `1.75` (set in `Cargo.toml` and `clippy.toml`) |
| Components | `rustfmt`, `clippy` |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After install, `rustup` will automatically read `rust-toolchain.toml` and install the correct toolchain and components when you run any `cargo` command in this workspace.

Verify:

```bash
rustc --version    # should be >= 1.75
cargo --version
cargo clippy --version
cargo fmt --version
```

## Core Tools

These ship with the Rust toolchain (installed automatically via `rust-toolchain.toml`):

| Tool | Purpose | Config file |
|------|---------|-------------|
| `cargo` | Build, test, run, bench | `Cargo.toml` |
| `clippy` | Linting (security + style) | `clippy.toml`, `Cargo.toml` `[workspace.lints.clippy]` |
| `rustfmt` | Code formatting | `rustfmt.toml` |
| `rustdoc` | Documentation generation | Inline `///` and `//!` comments |

### Clippy

Lint levels are configured at the workspace level in `Cargo.toml` under `[workspace.lints.clippy]`. Thresholds (complexity, argument count, line count) are in `clippy.toml`. All crates inherit these via `[lints] workspace = true`.

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Rustfmt

Configured in `rustfmt.toml` (edition 2021, 100-char line width, 4-space tabs).

```bash
cargo fmt --all           # format everything
cargo fmt --all -- --check  # check without modifying
```

### Rustdoc

```bash
cargo doc --workspace --no-deps --open
```

## Security Scanning Tools

These are external `cargo` subcommands. Install all at once:

```bash
make install-security-tools
# or manually:
cargo install cargo-audit cargo-deny cargo-geiger cargo-semver-checks
```

| Tool | Purpose | Blocks CI? | Config |
|------|---------|-----------|--------|
| `cargo-audit` | CVE scan against RustSec advisory DB | Yes | — |
| `cargo-deny` | License, advisory, ban, and source checks | Yes | `deny.toml` |
| `cargo-geiger` | Reports `unsafe` usage in dependency tree | No | — |
| `cargo-semver-checks` | Detects breaking API changes | No (PR only) | — |

Run all scans:

```bash
make security
```

See [SECURITY_SCANNING.md](SECURITY_SCANNING.md) for detailed usage and configuration.

## Editor Setup

### VS Code + rust-analyzer

1. Install [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension.
2. Recommended `settings.json` for this workspace:

```jsonc
// .vscode/settings.json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.allTargets": true,
  "rust-analyzer.check.extraArgs": ["--all-features"],
  "rust-analyzer.cargo.allFeatures": true,
  "rust-analyzer.rustfmt.extraArgs": ["--edition", "2021"],
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer",
    "editor.rulers": [100]
  }
}
```

Useful extensions:
- [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml) — syntax highlighting for `Cargo.toml`, `deny.toml`
- [crates](https://marketplace.visualstudio.com/items?itemName=serayuzgur.crates) — inline dependency version info
- [Error Lens](https://marketplace.visualstudio.com/items?itemName=usernamehw.errorlens) — inline clippy/compiler diagnostics

### RustRover

[RustRover](https://www.jetbrains.com/rust/) has built-in support for Cargo workspaces, clippy, and rustfmt. Open the workspace root directory and it will auto-detect the project structure.

Key settings:
- **Languages & Frameworks → Rust → Rustfmt**: enable "Use rustfmt instead of built-in formatter"
- **Languages & Frameworks → Rust → External Linters**: enable Clippy, set to run on save
- **Editor → Code Style → Rust**: set hard wrap at 100 (matches `rustfmt.toml`)

### Neovim

With [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig), add rust-analyzer:

```lua
require("lspconfig").rust_analyzer.setup({
  settings = {
    ["rust-analyzer"] = {
      check = { command = "clippy", allTargets = true },
      cargo = { allFeatures = true },
    },
  },
})
```

## Configuration Files Reference

| File | Tool | Purpose |
|------|------|---------|
| `rust-toolchain.toml` | rustup | Pins toolchain channel and components |
| `Cargo.toml` | cargo | Workspace members, dependencies, lint levels, profiles |
| `clippy.toml` | clippy | Complexity and threshold settings |
| `rustfmt.toml` | rustfmt | Formatting rules |
| `deny.toml` | cargo-deny | License allowlist, advisory settings, bans, sources |
| `Makefile` | make | Security scan targets, `install-security-tools` |
| `.github/workflows/security.yml` | GitHub Actions | CI security scanning pipeline |

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough
- [ARCHITECTURE.md](ARCHITECTURE.md) — workspace layout and crate structure
- [EXTENDING.md](EXTENDING.md) — adding crates, dependencies, feature flags
- [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) — safety patterns and concurrency guide
- [SECURITY_SCANNING.md](SECURITY_SCANNING.md) — detailed security tool configuration and exceptions
- [cli.md](cli.md) — CLI development patterns
