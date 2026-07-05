# AGENTS.md — crates/cli/tests

`cli_integration.rs` drives the **compiled binary** through `assert_cmd` and
`predicates`, on purpose. Unit tests in `src/lib.rs` already cover parsing
and `run()` output; these tests cover the part unit tests cannot: the real
user contract — exit codes, what lands on stdout vs stderr, and how flags,
env vars, and config files behave when the process is invoked the way a user
invokes it. If a regression would only be visible to someone at a shell
prompt, the test for it belongs here.

Rules for edits:

- Always start from `Command::cargo_bin("demo-cli")` (the `cmd()` helper).
- Assert the contract, not internals: `.success()`/`.failure()` for exit
  codes, `.stdout(...)`/`.stderr(...)` predicates for output placement.
  Diagnostics must never be asserted on stdout.
- Stay hermetic. No network. Config files go in `tempfile::tempdir()` dirs,
  never the repo. Any test touching `serve` must go through `serve_cmd()`,
  which strips `HOST`/`PORT`/`APP_*` so the developer's shell environment
  cannot change results; extend that list if you add new env bindings.
- Keep tests deterministic — no timing assertions, no sleeps.
- Every new subcommand or flag added in `src/lib.rs` needs at least one
  success-path and one failure-path test here.

Verification: `cargo test -p cli` and
`cargo clippy -p cli --all-targets -- -D warnings`.
