# AGENTS.md — `crates/database/migrations`

SQL schema migrations, embedded into the crate at compile time by
`src/migrate.rs` (`include_str!` + the `MIGRATIONS` array).

## Rules

- **Migrations are append-only.** Once a migration file exists, treat it as
  applied to somebody's database. Editing it would make already-migrated
  databases diverge from freshly-migrated ones with no record of the
  difference — schema history must be reproducible from the files alone.
  To change the schema, add a **new** file; never rewrite or delete an old one.
- **Naming:** zero-padded ascending prefix plus a snake_case description,
  e.g. `002_add_entities_status.sql`. The number defines apply order.
- **Registration:** after adding a file, append it to the `MIGRATIONS` array
  in `src/migrate.rs` in numeric order — the runner does not scan this
  directory at runtime.
- **Idempotency:** this template's runner has no version-tracking table and
  re-runs every file on startup, so every statement must be safe to run twice
  (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, guarded
  `ALTER`s where SQLite allows).
- **Dialect:** SQLite. Columns are `TEXT` (UUIDs hyphenated, timestamps
  RFC 3339 — see `src/AGENTS.md`). Note `DEFAULT (datetime('now'))` yields a
  non-RFC 3339 string; Rust code always supplies `created_at` explicitly.

## Verification

```bash
cargo test -p database   # repo tests run the migrations against sqlite::memory:
```
