# AGENTS.md — crates/systems/src

Read the root `AGENTS.md` first for workspace-wide rules. The companion human doc
is `docs/MEMORY_SAFETY_AND_CONCURRENCY.md` (see its "Unsafe Rust Guidelines"
section); keep it in sync if you change behavior described there.

## Why this crate exists

This is the **one crate in the workspace where `unsafe` is the point**. It shows
what safe Rust automates — allocation, deallocation, drop order, aliasing rules —
by doing those jobs manually, and it shows the professional discipline for it:
every unsafe operation is wrapped in a small **safe API whose invariants make the
unsafe code sound**. A reader should come away understanding that unsafe Rust is
not "Rust without rules"; it is Rust where *you* uphold the rules the compiler
normally enforces.

- `unsafe_rust.rs` teaches raw pointers and manual container management: what
  `Vec` really does under the hood, and how a safe wrapper (RawStack) confines
  the danger.
- `memory.rs` teaches manual allocation strategies: a bump arena (allocate fast,
  free all at once), an RAII guard for foreign resources, and a hand-rolled `Box`.
- `ffi.rs` teaches the C boundary: calling libc, exporting a C-ABI function, and
  the **trampoline pattern** — the standard trick for passing a Rust closure
  through a C-style `fn pointer + void* context` API, since closures capture
  state and cannot themselves be function pointers.

## Files

### unsafe_rust.rs

`raw_swap` is the minimal `unsafe fn` example: the safety contract lives in the
`# Safety` doc section, not in code. `RawStack<T>` is the centerpiece — a
fixed-capacity stack over a raw allocation. Its load-bearing invariants:

- **`len <= cap` always**, and slots `0..len` hold initialized values; slots
  `len..cap` are uninitialized garbage that must never be read or dropped.
- `push` writes only when `len < cap`; `pop` uses `ptr::read` (a *move* out),
  so the slot it vacates must not be dropped again — `Drop` only drops `0..len`.
- `new` rejects `cap == 0` and zero-sized `T` because `alloc` with a zero-size
  layout is undefined behavior.
- `Drop` drops the live elements *then* frees with the **same layout** used to
  allocate. The `unsafe impl Send` is sound only because RawStack exclusively
  owns its buffer.

`DeepSizeOf` demonstrates an `unsafe trait`: the *implementor* promises
correctness, the caller trusts it. Note its Vec impl is shallow — it does not
recurse into element-owned heap memory.

### memory.rs

`Arena` is a bump allocator. Invariants that must survive any edit: the offset
only moves forward, so **no two allocations ever overlap**; returned references
borrow the arena, so the buffer outlives them; `alloc` rejects alignment > 16
because the buffer base is only 16-aligned (aligning the offset cannot fix a
misaligned base); capacity must be non-zero (zero-size `alloc` is UB). `reset`
is `unsafe` because it invalidates outstanding references — the caller promises
none exist. Neither `reset` nor `Drop` runs destructors: the arena is for
POD-like types, and that limitation is documented, not a bug.

`Guard` is the RAII cleanup pattern for resources with no Rust wrapper. `disarm`
exists for the commit/rollback idiom: arm the guard, do fallible work, disarm on
success so cleanup only runs on the error path. The `Option::take` in `Drop`
guarantees the closure runs at most once.

`HeapVal<T>` is a teaching `Box`: alloc → write → use → `drop_in_place` →
dealloc, always with the same `Layout::new::<T>()`. It rejects zero-sized types
(real `Box` uses a dangling pointer for ZSTs; that machinery would obscure the
example). Its `Send`/`Sync` impls mirror `Box`'s bounds — do not loosen them.

### ffi.rs

The libc wrappers (`getpid`, `getenv`, `page_size`) show the shape of a safe
wrapper over C: validate inputs (`CString::new` guards interior NULs), check for
null returns, and **copy foreign data before returning** (`getenv`'s result is
copied because later env mutations may invalidate the C pointer).
`rust_strlen` shows the export direction: `#[no_mangle] extern "C"`, marked
`unsafe fn` because the pointer contract cannot be checked — keep the null check
and the `# Safety` docs. `foreach_with_closure` is the trampoline: a monomorphized
`extern "C" fn` casts the `void*` context back to `&mut F`. The soundness hinges
on the context pointer outliving every callback invocation and being the only
live reference to the closure during the call.

## Editing rules

- Every `unsafe` block gets a `// SAFETY:` comment stating the invariant that
  makes it sound. Every `unsafe fn`/`unsafe trait` gets a `# Safety` doc section
  stating the caller's/implementor's obligations. No exceptions.
- **Never widen the unsafe surface.** Keep unsafe encapsulated behind safe
  wrappers; do not add public APIs that hand out raw pointers or take unchecked
  indices. Prefer making a safe method panic over making it unsafe.
- Think like Miri: no aliasing `&mut`, no use-after-free, no reads of
  uninitialized memory, no zero-size or mismatched-layout alloc/dealloc, respect
  alignment. If you touch pointer code, re-derive the invariant from scratch —
  do not assume the surrounding code was right.
- Allocation and deallocation must use the *same* layout; values written with
  `ptr::write` are dropped exactly once (either `read` out or `drop_in_place`).
- `assert!` on API misuse is acceptable here (this crate is exempt from the
  no-panic convention), but never replace a safety check with a debug-only one.
- Do not add dependencies; `libc` is deliberately the only FFI dep. Keep
  everything Unix-portable (no Linux-only or macOS-only calls).

## Verification

```bash
cargo test -p systems
cargo clippy -p systems --all-targets -- -D warnings
cargo fmt
```

If you have Miri available (nightly), `cargo +nightly miri test -p systems`
is the strongest check for this crate — the FFI tests won't run under Miri,
but the pointer code in `unsafe_rust.rs` and `memory.rs` will.
