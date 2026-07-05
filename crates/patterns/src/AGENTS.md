# AGENTS.md — crates/patterns/src

Read the root `/AGENTS.md` first for workspace-wide rules (coverage ≥80%,
clippy `-D warnings`, MSRV 1.75).

## Why this crate exists

`patterns` teaches four Rust-idiomatic design patterns — type-state builder,
newtype, typestate protocol, and strategy/trait objects — each in its smallest
readable form. It deliberately has **zero dependencies** (not even `common`):
the point is that these patterns are pure language features (generics,
`PhantomData`, traits), so a reader can copy any single file into any project
unchanged. Adding a dependency here would falsify that lesson; do not add one.

The domain objects (`Request`, `Connection`, `Compressor`) are toys on
purpose. Their realism does not matter; the compile-time guarantees do. Every
file's real payload is "the compiler rejects the wrong usage", so the test of
any edit is: do the illegal states still fail to compile?

## Files

### lib.rs

Declares the four pattern modules, one per pattern. Keep it that way — one
file per pattern, no cross-module imports between them, so each file stays
independently liftable.

### builder.rs

Builder with compile-time required-field enforcement. The invariant: `build()`
is only implemented on `RequestBuilder<Set, Set>`, and the only way to turn
`Missing` into `Set` is through `method()` / `url()`, which store `Some`. If
an edit lets `RequestBuilder::new().build()` compile, the file's purpose is
defeated. The two `unwrap()` calls in `build()` are the crate's one sanctioned
unwrap — the type-state proves they cannot fire; keep the INVARIANT comment
next to them. Note the state-transition impls are generic over the *other*
parameter (`impl<U> RequestBuilder<Missing, U>`), which is what makes
`method()`/`url()` order-independent — preserve that shape.

### newtype.rs

Newtypes for domain safety at three levels: validated construction (`Email`
can only be created through `new`, so an `Email` in hand is always valid —
never add a public constructor or `pub` field that bypasses validation),
phantom-tagged IDs (`Id<UserTag>` vs `Id<OrderTag>` share code but do not
unify, so a `UserId` cannot be passed where an `OrderId` is expected), and
unit wrappers (`Meters`/`Kilometers` with explicit `From` conversions). The
tag types are intentionally empty and never constructed.

### typestate.rs

State machine encoded in the type parameter: `Connection<Disconnected>` →
`Connected` → `Authenticated`, where `query()` exists only on
`Authenticated` and every transition consumes `self` (moves, not `&mut`),
making stale-state reuse a compile error. Both invariants — method
availability per state and by-value transitions — must survive any edit.
`token` is `Option<String>` because the struct is shared across states; only
`authenticate()` fills it, which is why `query()`'s fallback branch is
unreachable.

### strategy.rs

Strategy pattern shown both ways on the same `Compressor` trait: dynamic
dispatch (`Pipeline` holds `Box<dyn Compressor>`, swappable at runtime) and
enum dispatch (`CompressorKind`, no heap allocation). The point of the file
is the *contrast*, so keep both and keep them implementing the same trait.
The trait needs `Send + Sync` so boxed strategies can cross threads. RLE
detail with a test guarding it: the run counter is a `u8`, so runs longer
than 255 must split into multiple (count, byte) pairs.

## Editing rules

- Zero dependencies. Do not add anything to `[dependencies]`, including
  `common`. Errors here are plain `Result<T, &'static str>` for that reason —
  do not "upgrade" them to `thiserror`/`AppError`.
- Never weaken a compile-time guarantee to make code shorter. If you touch a
  type-state file, manually confirm the illegal calls still fail to compile
  (e.g. `RequestBuilder::new().build()`, `Connection::new("x").query("q")`).
- No `unwrap`/`expect`/`panic` in library paths, except the documented
  invariant unwraps in `builder.rs`. Fine in `#[cfg(test)]`.
- Public items need `///` doc comments — including trait methods, enum
  variants, and the marker/tag types.
- Tests are co-located in `#[cfg(test)] mod tests`; new behavior needs a test
  (CI enforces ≥80% line coverage).
- Footgun: when a state-transition method builds the next-state struct, it
  must rebuild it field by field (you cannot `..self` across a change of type
  parameter). Forgetting to carry a field over (e.g. dropping `headers` in
  `method()`) compiles fine and silently loses data — check every field.
- Footgun: `PhantomData<(M, U)>` markers cost nothing at runtime; do not
  replace them with real fields or runtime flags — runtime checks are exactly
  what these files exist to avoid.

## Verification

```bash
cargo test -p patterns
cargo clippy -p patterns --all-targets -- -D warnings
cargo fmt
```
