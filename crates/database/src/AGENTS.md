# AGENTS.md — `crates/database/src`

Read the root `AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75, no `unwrap`/`expect`/`panic` in library code).
One rule from there bears repeating: **`sqlx` must stay ≥ 0.8** — 0.7 pulled
in a vulnerable `rsa` crate, and the accepted suppression in
`.cargo/audit.toml` exists only for the residual 0.8 advisory. Never downgrade.

## Why this crate exists

This crate teaches the three things every persistent Rust service needs:
a configured connection pool, a migration story, and a **repository** that
hides SQL behind a typed API. The repository exists so that callers deal in
`common::Entity` and `common::AppError`, never in rows, SQL strings, or
`sqlx::Error` — that boundary is what makes the storage engine swappable and
the call sites testable.

SQLite is used in the examples because `sqlite::memory:` gives every test a
free, isolated, zero-setup database that works in CI. The workspace `sqlx`
dependency still enables the `postgres` feature on purpose: versions and
features are declared once at the workspace root, and readers copying this
crate into a real service are expected to swap `SqlitePool` for `PgPool`
without touching dependency declarations.

The queries are deliberately the **runtime-checked** kind (`sqlx::query`,
`sqlx::query_as`), not the compile-time `sqlx::query!` macros: the macros
require a live `DATABASE_URL` or offline metadata at build time, and this
template must build with nothing installed. Do not "upgrade" to the macros.

## Files

### `lib.rs`
Module wiring plus a doc-test quick start showing the intended call order:
pool → migrate → repo. Keep that example compiling; it is the crate's
front-page documentation.

### `pool.rs`
`PoolConfig` exists so every tunable is named, documented, and defaulted in
one struct instead of scattered call-site literals. The defaults are
deliberately conservative — 5 connections and a 5-second acquire timeout —
because SQLite serializes writers, so a large pool buys nothing, and a
bounded acquire timeout converts pool exhaustion into a visible error instead
of a hang. `create_if_missing` defaults to `true` for frictionless examples;
a real deployment would likely flip it. The `after_connect` hook is there to
show where per-connection setup (PRAGMAs, session settings) belongs.

### `migrate.rs`
Runs the SQL files under `migrations/`, embedded at compile time with
`include_str!` so the runner can never drift from the files on disk — the SQL
must never be duplicated into Rust string literals. `sqlx::raw_sql` is used
because migration files contain multiple statements and `sqlx::query` cannot
execute more than one. There is no version-tracking table; idempotency comes
from `IF NOT EXISTS`, which is why `migrations/AGENTS.md` requires it. New
migration files must be appended to the `MIGRATIONS` array **in order**. The
doc comment pointing at `sqlx::migrate!()` for production is load-bearing —
keep it.

### `repo.rs`
The repository. `EntityRow` is a private mirror of the table (everything is
`TEXT` — SQLite has no native UUID or timestamp types), and
`TryFrom<EntityRow> for Entity` is the single fallible boundary where strings
become `Uuid`/`DateTime<Utc>`. Keep row types private; leaking them defeats
the pattern. `create_batch` exists to demonstrate transactions: work runs
against `&mut *tx`, and **dropping a `Transaction` without `.commit()` rolls
it back** — that drop-based rollback on the early-`?`-return path is the
lesson, so never restructure it in a way that commits partially. Every
`sqlx::Error` is converted to `AppError::database(e.to_string())` at this
layer; nothing above sees sqlx types.

### `Cargo.toml`
All versions come from `[workspace.dependencies]`; never pin a version here.

## Editing rules

- **Never build SQL by string interpolation or `format!`.** Every dynamic
  value goes through `.bind()` — this is the injection-safety invariant of
  the whole crate.
- Timestamps are stored as RFC 3339 text (`to_rfc3339()` on write,
  `parse::<DateTime<Utc>>()` on read). If you write with a different format,
  reads will fail at the `TryFrom` boundary. Note that the schema's SQL-side
  `DEFAULT (datetime('now'))` produces a *non*-RFC 3339 string — always
  supply `created_at` explicitly from Rust.
- UUIDs are stored as hyphenated text via `.to_string()`; compare and bind
  them in that form only.
- New repo methods: return `Result<_, common::AppError>`, convert errors with
  `AppError::database(e.to_string())`, add `///` docs, and add a
  `#[tokio::test]` using the existing `setup()` helper (fresh in-memory DB
  per test — tests must not share state).
- Inside a transaction, execute against `&mut *tx`, not `&self.pool`;
  querying the pool mid-transaction silently escapes the transaction.
- No `unwrap`/`expect`/`panic` outside `#[cfg(test)]`.

## Verification

```bash
cargo test -p database
cargo clippy -p database --all-targets -- -D warnings
cargo fmt
```
