# CLI Development in Rust

A guide to building command-line applications in Rust, based on the patterns in `crates/cli/`.

## Table of Contents

- [Clap Derive API](#clap-derive-api)
- [Configuration Merging with Figment](#configuration-merging-with-figment)
- [Interactive Output](#interactive-output)
- [Shell Completions](#shell-completions)
- [Testing](#testing)
- [Best Practices](#best-practices)

---

## Clap Derive API

The `clap` crate with derive macros is the standard way to define CLI interfaces in Rust. Derive macros generate the argument parser from struct/enum definitions at compile time.

### Basic Structure

```rust
use clap::{Parser, Subcommand, Args, ValueEnum};

#[derive(Parser)]
#[command(name = "demo-cli", version, about = "description")]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}
```

- `#[derive(Parser)]` — entry point, generates `Cli::parse()` and `Cli::try_parse_from()`
- `#[command(...)]` — metadata: name, version (from Cargo.toml), about text
- `#[arg(...)]` — configures individual arguments

### Subcommands

Use `#[derive(Subcommand)]` on an enum. Each variant becomes a subcommand:

```rust
#[derive(Subcommand)]
pub enum Command {
    Greet(GreetArgs),       // `demo-cli greet`
    Serve(ServeArgs),       // `demo-cli serve`
    Config(ConfigCmd),      // `demo-cli config` (nested)
}
```

### Nested Subcommands

Wrap another `#[derive(Subcommand)]` enum inside an `#[derive(Args)]` struct:

```rust
#[derive(Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Get { key: String },
    Set { key: String, value: String },
    List,
}
```

This gives you `demo-cli config get <key>`, `demo-cli config set <key> <value>`, etc.

### Flags and Arguments

```rust
#[derive(Args)]
pub struct GreetArgs {
    /// Positional argument (no `--` prefix)
    pub name: String,

    /// Named flag with short alias, default, and validation
    #[arg(short = 'n', long, default_value_t = 1,
          value_parser = clap::value_parser!(u32).range(1..=100))]
    pub count: u32,

    /// Boolean flag
    #[arg(short, long)]
    pub uppercase: bool,
}
```

Key patterns:
- Positional args: bare fields without `long`/`short`
- Optional values: use `Option<T>`
- Defaults: `default_value_t` for typed, `default_value` for strings
- Validation: `value_parser` with `.range()` for numeric bounds

### Env Var Binding

Clap can read values from environment variables as a fallback:

```rust
#[arg(long, env = "HOST")]
pub host: Option<String>,

#[arg(long, env = "PORT")]
pub port: Option<u16>,
```

Priority: explicit CLI flag > env var > `None`.

Note these args are `Option<T>` with **no** clap `default_value`. When a
CLI value also participates in layered config (next section), a clap default
would make the flag always-present, silently shadowing the config-file and
`APP_*` env layers. Keep the effective default in `AppConfig::default()` and
let clap report "not provided" as `None`.

### ValueEnum

For arguments with a fixed set of choices:

```rust
#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

// In Cli:
#[arg(long, global = true, default_value = "text")]
pub format: OutputFormat,
```

Clap auto-generates `--format text` / `--format json` with validation and help text.

### Global Arguments

`global = true` makes a flag available on all subcommands:

```rust
#[arg(short, long, global = true)]
pub verbose: bool,
```

This allows both `demo-cli --verbose greet Alice` and `demo-cli greet --verbose Alice`.

---

## Configuration Merging with Figment

Real-world CLIs need layered configuration: defaults → config file → env vars → CLI flags. The `figment` crate handles this cleanly.

### Layer Priority (highest wins)

1. CLI flags
2. Environment variables (`APP_` prefix)
3. TOML config file
4. Hardcoded defaults

### Config Struct

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub tls_cert: Option<PathBuf>,
    pub log_level: String,
    pub workers: usize,
}

impl Default for AppConfig { /* ... */ }
```

### CLI Overrides Pattern

Use `Option` fields with `skip_serializing_if` so unset CLI flags don't clobber lower layers:

```rust
#[derive(Serialize)]
pub struct CliOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}
```

### Loading

```rust
use figment::{Figment, providers::{Format, Toml, Env, Serialized}};

pub fn load_config(path: &Path, cli: CliOverrides) -> Result<AppConfig, figment::Error> {
    Figment::new()
        .merge(Serialized::defaults(AppConfig::default()))
        .merge(Toml::file(path))
        .merge(Env::prefixed("APP_"))
        .merge(Serialized::defaults(cli))
        .extract()
}
```

### Example TOML

```toml
host = "0.0.0.0"
port = 3000
log_level = "info"
workers = 8
```

See `crates/cli/config.example.toml` for the full example.

---

## Interactive Output

### Colored Output with owo-colors

`owo-colors` provides zero-allocation colored terminal output via extension traits:

```rust
use owo_colors::OwoColorize;

println!("{}", "Error: something went wrong".red().bold());
println!("{}", "Warning: check config".yellow());
println!("{}", "Success: done".green());
println!("{} {} {}", "bold".bold(), "italic".italic(), "underline".underline());
```

Why owo-colors over `colored`: smaller dependency, no global state, works with `no_std`.

### Progress Bars with indicatif

```rust
use indicatif::{ProgressBar, ProgressStyle};

let pb = ProgressBar::new(total_steps);
pb.set_style(
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .expect("valid template")
        .progress_chars("=>-"),
);

for i in 0..total_steps {
    pb.set_message(format!("step {}", i + 1));
    // do work...
    pb.inc(1);
}
pb.finish_with_message("done");
```

Tips:
- Use `ProgressBar::new_spinner()` for indeterminate tasks
- Use `MultiProgress` for concurrent progress bars
- Call `pb.finish_with_message()` to leave a clean final line

---

## Shell Completions

Generate completions for bash, zsh, fish, PowerShell, and elvish using `clap_complete`:

```rust
use clap::CommandFactory;
use clap_complete::{generate, Shell};

pub fn print_completions(shell: Shell) {
    generate(shell, &mut Cli::command(), "demo-cli", &mut io::stdout());
}
```

Expose it as a subcommand:

```rust
Completions {
    #[arg(value_enum)]
    shell: Shell,
},
```

### Installing Completions

```bash
# Bash
demo-cli completions bash > ~/.local/share/bash-completion/completions/demo-cli

# Zsh
demo-cli completions zsh > ~/.zfunc/_demo-cli

# Fish
demo-cli completions fish > ~/.config/fish/completions/demo-cli.fish
```

---

## Testing

### Unit Tests — Command Parsing

Use `Cli::parse_from()` and `Cli::try_parse_from()` to test parsing without running a process:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn greet_basic() {
        let cli = Cli::parse_from(["demo-cli", "greet", "Alice"]);
        match &cli.command {
            Command::Greet(a) => assert_eq!(a.name, "Alice"),
            _ => panic!("expected Greet"),
        }
    }

    #[test]
    fn rejects_invalid_input() {
        let result = Cli::try_parse_from(["demo-cli", "greet", "X", "-n", "0"]);
        assert!(result.is_err());
    }
}
```

Test the `run()` function directly for output verification:

```rust
#[test]
fn greet_output() {
    let cli = Cli::parse_from(["demo-cli", "greet", "Bob", "--uppercase"]);
    assert_eq!(run(&cli), "HELLO, BOB!");
}
```

### Integration Tests with assert_cmd

`assert_cmd` runs the compiled binary as a subprocess — tests the full CLI end-to-end:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("demo-cli").expect("binary exists")
}

#[test]
fn greet_outputs_hello() {
    cmd()
        .args(["greet", "World"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, World!"));
}

#[test]
fn missing_subcommand_fails() {
    cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}
```

Key `assert_cmd` patterns:
- `Command::cargo_bin("name")` — finds the binary built by cargo
- `.assert().success()` / `.failure()` — check exit code
- `.stdout(predicate)` / `.stderr(predicate)` — check output
- `.env("KEY", "value")` — set env vars for the subprocess

Add to `Cargo.toml`:

```toml
[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
```

---

## Best Practices

### Structure

- **Separate parsing from logic.** Define types in `lib.rs`, keep `main.rs` minimal (`parse` then `run`). This makes unit testing trivial.
- **Use a `run()` function** that takes `&Cli` and returns `String` or `Result`. Avoids `process::exit` in library code.
- **Group related args** with `#[derive(Args)]` structs rather than flat flag lists.

### Error Handling

- Return `anyhow::Result` or a custom error type from `run()` — let `main()` handle display and exit codes.
- Use `try_parse_from` in tests to verify error cases without panicking.
- Provide actionable error messages: say what went wrong and what the user should do.

### Configuration

- Support `--config <path>` for explicit config file location.
- Use layered merging (defaults → file → env → flags) so every setting has a sensible default but can be overridden at any level.
- Prefix env vars with your app name (`APP_HOST`, `APP_PORT`) to avoid collisions.

### UX

- Add `global = true` to flags like `--verbose` and `--format` so they work anywhere in the command.
- Use `value_parser` with `.range()` for numeric validation — gives clear error messages automatically.
- Provide shell completions — it's a small addition that significantly improves usability.
- Use color and progress bars for interactive use, but respect `NO_COLOR` env var and pipe detection.

### Testing

- Unit test parsing with `parse_from` / `try_parse_from` — fast, no subprocess overhead.
- Integration test with `assert_cmd` for end-to-end verification of the compiled binary.
- Test both success and failure paths (invalid input, missing args, validation errors).
- Use `tempfile` for tests that need config files on disk.

### Dependencies (as used in this template)

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | Argument parsing |
| `clap_complete` | Shell completion generation |
| `figment` | Layered configuration merging |
| `owo-colors` | Colored terminal output |
| `indicatif` | Progress bars and spinners |
| `serde` | Serialization for config structs |
| `assert_cmd` | Integration testing (dev) |
| `predicates` | Output assertions (dev) |
| `tempfile` | Temp files in tests (dev) |

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough including running the CLI
- [ARCHITECTURE.md](ARCHITECTURE.md) — workspace layout and crate structure
- [TOOLCHAIN.md](TOOLCHAIN.md) — required tools and editor setup
- [EXTENDING.md](EXTENDING.md) — adding new binary crates to the workspace
- [MEMORY_SAFETY_AND_CONCURRENCY.md](MEMORY_SAFETY_AND_CONCURRENCY.md) — safety patterns and concurrency guide
- [SECURITY_SCANNING.md](SECURITY_SCANNING.md) — security tools and CI integration
