# AGENTS.md — `crates/api-server/src`

Read the root `AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75, no `unwrap`/`expect`/`panic` in library code).

## Why this crate exists

This crate teaches the shape of a production Axum REST service in the smallest
form that still shows the real layering: router construction, typed extractors,
shared state, a custom middleware, HTTP error mapping, and a typed `reqwest`
client for the other side of the wire. It deliberately uses an in-memory
`HashMap` instead of a real database so the HTTP concerns stay visible — the
`database` crate teaches persistence separately.

The load-bearing idea is **separation by concern, one file each**: handlers
never format HTTP errors, state never knows about routes, and error mapping
lives in exactly one place. `ApiError` exists as a newtype for two reasons:
the orphan rule forbids implementing the foreign trait `IntoResponse` on the
foreign type `common::AppError`, and having a single conversion point is what
lets us enforce error-text hygiene — 500-class variants (`Database`, `Io`,
`Internal`) are logged server-side and replaced with a generic message so
internal detail (paths, connection strings) never reaches a client.

## Files

### `lib.rs`
Assembles the router (`app()`) and the server entry point (`serve()`), and
holds the router-level tests (`tower::ServiceExt::oneshot`, no real sockets).
`app()` is a pure function returning `Router` precisely so tests can drive it
without binding a port — keep it that way. Layer order matters: `.layer()`
calls wrap the router **outside-in in reverse order**, so the last layer added
(`CorsLayer`) runs first on a request. Routes are registered before
`.with_state()` so the state type is fully resolved.

### `state.rs`
`AppState` is `Clone` because Axum clones the state for every request — that
is why the map lives behind `Arc<RwLock<…>>` (cloning is a cheap pointer copy,
not a data copy). Uses `tokio::sync::RwLock`, not `std::sync::RwLock`: a std
guard held across an `.await` blocks the executor thread. Keep `AppState`
cheap to clone; never put unshared owned data in it.

### `handlers.rs`
One handler per extractor pattern (`Query`, `Json`, `Path`, `State`). Handlers
must stay thin — validate, touch state, return. Any real business logic
belongs in a lower layer (see the `database` crate's repository pattern).
Handlers return `Result<_, ApiError>` so `?` plus `From<AppError>` does the
error conversion; do not construct HTTP status codes for errors inside
handlers.

### `error.rs`
The `ApiError` newtype and its `IntoResponse` impl — the only place domain
errors become HTTP. The module is intentionally **private** (`mod error;` in
`lib.rs` re-exports nothing): callers outside the crate should never build
HTTP error responses directly. The 500-sanitization behavior is a security
invariant with tests; any edit must keep `internal_error_detail_is_not_leaked`
passing.

### `middleware.rs`
A minimal `axum::middleware::from_fn` middleware: injects `x-request-id` into
request and response, preserving an incoming value from a gateway. It shows
the pattern (take `Request` and `Next`, call `next.run(req).await`, decorate
the response). The UUID→`HeaderValue` conversion uses a non-panicking fallback
on purpose — do not "simplify" it back to `unwrap()`.

### `client.rs`
The consumer side: a typed wrapper over `reqwest::Client`. The one lesson is
**build the client once and clone it** — `reqwest::Client` is an `Arc` around
a connection pool, and constructing one per request is the classic perf bug.
Errors funnel into `common::AppError` so callers see one error type.

### `Cargo.toml`
All versions come from `[workspace.dependencies]` — never pin here. `tower`
enables the `util` feature locally because the tests need
`ServiceExt::oneshot`; feature additions are fine, version pins are not.

## Editing rules

- Extractor ordering: in a handler signature, body-consuming extractors
  (`Json`, `Form`, `String`) must be the **last** argument; `State`, `Path`,
  `Query` come first. Getting this wrong is a confusing compile error.
- New routes go in `app()` and get a handler in `handlers.rs`, doc-commented
  with the `METHOD /path` convention used there, plus an `oneshot` test in
  `lib.rs`.
- New error variants in `common::AppError` need a status arm in `error.rs`;
  the match is exhaustive on purpose so the compiler flags the omission.
  Decide explicitly whether the new variant's text is client-safe.
- No `unwrap`/`expect`/`panic` in non-test code. The single `expect` in
  `shutdown_signal` is a tolerated application-edge case; do not add more.
- Doc comments (`///`) on every public item; tests live in `#[cfg(test)]`
  modules in the same file (where `unwrap` is fine).
- `serve()` installs a global tracing subscriber — never call it from tests,
  and do not add a second `.init()` anywhere in the crate.

## Verification

```bash
cargo test -p api-server
cargo clippy -p api-server --all-targets -- -D warnings
cargo fmt
```
