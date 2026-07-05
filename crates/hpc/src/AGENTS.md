# AGENTS.md — crates/hpc/src

Read the workspace root `AGENTS.md` first. This file explains why each module
in the `hpc` crate exists and what an edit must never destroy.

## Why this crate exists

`hpc` teaches the four pillars of high-performance Rust, one per module:
data-parallel CPU work (`parallel`), async concurrency (`async_runtime`),
vectorization (`simd`), and compile-time abstraction (`zero_cost`). Each module
answers a *choice* a reader faces:

- **`parallel` vs `async_runtime`:** Rayon is for CPU-bound work that should
  saturate cores; tokio is for I/O-bound work that should overlap waiting.
  They are deliberately separate modules so the reader never sees the two
  mixed — mixing them (CPU work on the async runtime) is the classic mistake.
- **`simd` shows auto-vectorization AND manual SSE intrinsics** on purpose:
  the lesson is that you should write auto-vectorizable scalar code first
  (`dot_product`, `vec_add`) and reach for `unsafe` intrinsics only when
  measurement justifies it. The benchmark in `benches/` exists to make that
  comparison observable. Deleting either half destroys the comparison.
- **`zero_cost` exists to prove abstractions compile away.** Generics
  monomorphize, newtypes vanish, iterator chains fuse. Introducing `Box`,
  `dyn` dispatch, or intermediate `collect()` calls there defeats the entire
  point of the module even if behavior stays correct.

This is the only crate besides `systems` allowed to contain `unsafe`, and only
for the SIMD intrinsics.

## Files

### `lib.rs`
Module list and crate doc only. Keep it that way — no logic in `lib.rs`.

### `parallel.rs`
Rayon patterns: `par_iter` map/sum, unstable parallel sort, `filter_map`,
`fold`+`reduce` (the two-phase pattern that avoids a shared accumulator), and
`with_thread_pool` for scoping work to a custom pool. Each function is the
minimal honest form of one Rayon idiom; do not merge them into one "utility".
Known deviation: `with_thread_pool` uses `.expect()` on pool construction —
changing that means changing the public signature, so leave it unless asked.

### `async_runtime.rs`
Tokio patterns: fan-out/fan-in via `tokio::spawn` + join, producer-consumer
over a **bounded** mpsc channel, request-response via `oneshot`, and `race`
via `tokio::select!` returning the local `Either` enum. The bounded channel is
the backpressure lesson — do not switch to `unbounded_channel`. The
`.expect()` calls here propagate panics from spawned tasks; they are a known,
documented deviation from the no-expect rule, kept because returning
`Result<_, JoinError>` would complicate every signature the module teaches.

### `simd.rs`
`dot_product` and `vec_add` are written in the exact shape LLVM
auto-vectorizes at `opt-level = 3`; "cleaning them up" into different shapes
can silently disable vectorization. `dot_product_sse` is the manual 128-bit
SSE version: it must keep its `#[cfg(target_arch = "x86_64")]` gate, its
`# Safety` doc section, the in-body `// SAFETY:` comments, and the scalar
remainder loop for lengths not divisible by 4. SSE is baseline on x86_64, so
no runtime `is_x86_feature_detected!` check is needed — but if you ever add
AVX or newer instructions, a runtime feature check becomes mandatory.

### `zero_cost.rs`
`Accumulate` + `generic_sum` demonstrate monomorphized generic dispatch;
`Meters`/`Seconds`/`MetersPerSecond` demonstrate newtype unit safety;
`sum_positive_squares` demonstrates fused single-pass iterator chains. The
invariant is *zero runtime cost*: no boxing, no `dyn`, no allocation, no
intermediate collections may be introduced here.

## Editing rules

- Never run CPU-bound work on the tokio runtime; in `async_runtime` examples
  that need heavy computation, use `tokio::task::spawn_blocking` (or hand the
  work to Rayon), never a plain `async` block that spins the CPU.
- Keep every unsafe operation inside `simd.rs`, gated with
  `#[cfg(target_arch = "x86_64")]`, and give every unsafe block/call site a
  `// SAFETY:` comment stating the invariant. Gate the corresponding tests
  and benches with the same cfg.
- Prefer iterator chains over index loops except where the index loop *is*
  the point (`vec_add` is an index loop because that shape auto-vectorizes).
- Do not add needless `collect()`; return iterators or fold directly.
- Keep tests deterministic: no timing-based assertions, no reliance on task
  or thread scheduling order (sort results or use order-independent asserts).
- Float assertions use epsilon comparisons, never exact `==` on computed
  values.
- No `unwrap`/`expect` in new library paths; tests and benches may use them.

## Verification

```bash
cargo fmt
cargo clippy -p hpc --all-targets -- -D warnings
cargo test -p hpc
cargo bench -p hpc --no-run   # benches must still compile
```
