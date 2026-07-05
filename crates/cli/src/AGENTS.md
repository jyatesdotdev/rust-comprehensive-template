# AGENTS.md — crates/cli/src

Read the root `AGENTS.md` first for workspace-wide rules. The companion human
doc is `docs/cli.md` — keep it in sync with any behavior change here.

## Why this crate exists

This crate teaches how to build a production-shaped command-line tool. It
builds the workspace's only binary, `demo-cli`. It deliberately depends on
nothing internal so a reader can lift it out of the workspace whole.

The lessons it exists to demonstrate:

- **lib.rs + main.rs split.** All parsing types and the `run()` dispatcher
  live in the library so unit tests can exercise them with
  `Cli::parse_from(...)` — no subprocess, no `process::exit`, no global state.
  `main.rs` stays a thin shim (parse, run, print) precisely because a shim
  that thin never needs its own tests.
- **Layered configuration with figment.** Precedence is defaults < config
  file < env vars < CLI flags. That order is the point: each layer is more
  specific and more intentional than the one below it — a flag typed at the
  prompt should beat an env var exported last week, which should beat a file
  edited last month. The `CliOverrides` struct with `Option` +
  `skip_serializing_if` exists so *unset* flags do not clobber lower layers.
- **Generated, not handwritten, shell completions.** `clap_complete` derives
  completions from the same `Cli` type that parses arguments, so completions
  can never drift from the real CLI surface. Handwritten completion scripts
  rot the first time someone adds a flag.
- **Exit codes and stream discipline.** stdout is for program output (what a
  user would pipe); stderr is for diagnostics (`--verbose` debug dump, usage
  errors from clap). Scripts depend on this contract — the integration tests
  assert it, and `docs/cli.md` documents it.

## Files

### lib.rs

The whole CLI surface: `Cli` (root parser), `Command`, all `Args` structs and
sub-enums, plus `run()`, which maps a parsed `Cli` to an output `String`.
Each subcommand exists to teach one clap feature (positional args, env
binding, numeric `value_parser` ranges, nested subcommands, `ValueEnum`,
`global = true` flags) — do not collapse them into fewer, cleverer examples.
`run()` returning `String` instead of printing is what keeps the unit tests
subprocess-free; preserve that shape.

### main.rs

Thin binary shim: parse, optionally dump `Cli` to **stderr** when verbose,
print `run()`'s output to stdout (skipping empty output so `completions`
does not get a stray blank line). Keep main.rs logic-free — any behavior you
are tempted to add here belongs in `run()` where it can be unit tested.

### config.rs

The figment lesson. `AppConfig` (with a `Default` impl as the bottom layer),
`CliOverrides` (the "only merge what the user actually set" pattern), and
`load_config()` whose four `.merge()` calls encode the precedence order.
**The provider order is the lesson** — reordering the merges silently changes
user-facing behavior and invalidates `docs/cli.md`. Related invariant in
lib.rs: `serve`'s `--host`/`--port` are `Option<T>` **without** clap
`default_value`s, precisely so unset flags stay `None` and don't shadow the
file/env layers (effective defaults live in `AppConfig::default()` instead).
Giving those args clap defaults again would silently break layering for host
and port — the integration test `serve_config_file_sets_host_and_port`
guards this.

### completions.rs

Three lines on purpose: `clap_complete::generate` against `Cli::command()`.
Completions must stay generated from the parser type; never check in or
hand-edit a completion script. Output goes straight to stdout because users
redirect it into their shell's completion directory.

### interactive.rs

Terminal-UX demos: `demo_colors` (owo-colors) returns a `String` so it is
testable; `demo_progress` (indicatif) draws to the terminal and sleeps, so
tests only call it with `steps = 0`. Note the progress-bar template error is
handled with a fallback, not `expect` — library code paths must not panic
(workspace convention), even when the failure "cannot happen".

## Editing rules

- Follow the existing clap derive conventions: doc comments become help
  text, so write them for end users; group a subcommand's flags in a
  dedicated `#[derive(Args)]` struct; validate numerics with
  `value_parser!(T).range(...)` rather than manual checks.
- Adding a subcommand is an end-to-end change: enum variant in `Command`,
  args struct, a `run()` match arm, unit tests for parsing and output, an
  integration test in `tests/cli_integration.rs`, and a section in
  `docs/cli.md`. A variant without all of these is half-finished.
- Never print errors or diagnostics to stdout. Errors go to stderr with a
  non-zero exit; stdout is reserved for pipeable program output.
- Do not add `unwrap`/`expect`/`panic` in these modules (tests excepted).
- Keep `config.example.toml` (crate root) in lockstep with `AppConfig` —
  every field present, same names, same types.
- New env-var integration must use the `APP_` prefix for figment or an
  explicit `env = "..."` clap binding; document either in `docs/cli.md`.

## Verification

```bash
cargo fmt
cargo clippy -p cli --all-targets -- -D warnings
cargo test -p cli
```

Coverage gate: CI enforces ≥80% workspace line coverage, so new code needs
tests in the same change.
