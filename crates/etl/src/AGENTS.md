# AGENTS.md — crates/etl/src

Read the workspace root `AGENTS.md` first. This file explains why each module
in the `etl` crate exists and what an edit must never destroy.

## Why this crate exists

`etl` teaches how to pick a data-processing architecture in Rust. The four
modules are four answers to "how should I process this data?", ordered by the
decision a reader must make:

- **`iterators`** — the default answer. Data fits in memory, single thread is
  fast enough: use fused iterator combinators, zero extra allocation.
- **`pipeline`** — same setting, but the *shape* of processing must be
  composable and reusable: trait-based stages chained at compile time, so the
  type system proves stage outputs match stage inputs with no dynamic
  dispatch.
- **`parallel`** — data fits in memory but one core is not enough: Rayon
  map-reduce and fold/reduce over slices. CPU-bound, synchronous, no runtime.
- **`streaming`** — data does not fit in memory or arrives over time: async
  stages connected by **bounded** channels, where backpressure (a full
  channel suspending the producer) is the core lesson.

Keeping these separate is the point. A reader should be able to diff the four
files and see exactly what each step up in complexity buys and costs.

## Files

### `lib.rs`
Crate doc with the module map. No logic; keep it a table of contents.

### `iterators.rs`
One function per iterator idiom: `parse_csv` (map→filter, tolerating bad
rows), `group_sum` (fold into a `HashMap`), `running_average` (`scan` for
stateful streaming), `flatten_transform` (`flat_map`), `top_n`
(`select_nth_unstable` partial sort — deliberately *not* a full sort, that is
the lesson). `top_n` uses `f64::total_cmp` so NaN cannot panic; do not revert
to `partial_cmp(..).unwrap()`. Each function stays a single fused chain — do
not split into multiple passes or intermediate `collect()`s.

### `pipeline.rs`
The `Stage` trait (associated `Input`/`Output` types), `Chain` for
composition, `StageExt::then` for ergonomic chaining, and closure-backed
`map_stage`/`filter_stage`. The design decision is *static* composition:
`Chain<A, B>` nests concrete types, so the whole pipeline monomorphizes with
zero dynamic dispatch. Replacing this with `Box<dyn Stage>` or trait objects
destroys the lesson. `process` returning `Option` is the built-in filtering
mechanism — keep it.

### `parallel.rs`
Rayon batch ETL: `par_map_reduce` (identity must be provided because reduce
needs a neutral element per split), `par_group_sum` (the fold-then-reduce
pattern: per-thread `HashMap`s merged at the end, avoiding a locked shared
map — that avoidance is the lesson), `par_filter_transform`, and
`par_batch_process` (chunk first, then parallelize over chunks). Everything
here is synchronous and CPU-bound by design; do not introduce tokio.

### `streaming.rs`
`streaming_pipeline` (source task → transform task → sink, linked by bounded
`tokio::sync::mpsc` channels) and `fan_out_pipeline` (N workers sharing one
`async_channel` receiver — `async_channel` is used precisely because tokio's
mpsc receiver cannot be cloned for multi-consumer). Invariants: channels stay
**bounded** (unbounded channels remove the backpressure lesson; buffer sizes
are deliberate), senders are dropped when producing ends so receivers
terminate (the explicit `drop(tx_out)` in `fan_out_pipeline` is load-bearing),
and every `send` checks for a closed channel instead of unwrapping.

## Editing rules

- CPU-bound work belongs in `parallel` (Rayon); never do heavy computation
  directly on the tokio runtime. If an async example must compute, use
  `tokio::task::spawn_blocking`.
- Preserve iterator fusion: one pass, no intermediate `collect()`, no
  index-based loops where a combinator reads better.
- No `unwrap`/`expect`/`panic` in library paths. For float ordering use
  `total_cmp`, for channel sends handle the `Err` (receiver-gone) case.
- Do not clone data to satisfy the borrow checker in parallel code; Rayon
  works on `&[T]` — restructure instead.
- Keep tests deterministic: `fan_out_pipeline` results arrive in arbitrary
  order, so tests must sort before asserting (see `fan_out_processes_all`).
  Never assert on timing or worker scheduling.
- No `unsafe` in this crate, ever.
- Public API is generic-heavy; changing a bound (e.g. adding `Clone`) is a
  breaking change — do not do it casually.

## Verification

```bash
cargo fmt
cargo clippy -p etl --all-targets -- -D warnings
cargo test -p etl
```
