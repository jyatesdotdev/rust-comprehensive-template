# Memory Safety & Concurrency Guide

This guide covers Rust's ownership model, lifetime system, concurrency primitives, and unsafe code
guidelines — with references to working examples in this workspace.

---

## Table of Contents

1. [Ownership & Borrowing](#ownership--borrowing)
2. [Lifetimes](#lifetimes)
3. [Error Handling Patterns](#error-handling-patterns)
4. [Concurrency: Async/Await](#concurrency-asyncawait)
5. [Concurrency: Channels & Message Passing](#concurrency-channels--message-passing)
6. [Concurrency: Shared State](#concurrency-shared-state)
7. [Parallel Processing](#parallel-processing)
8. [Unsafe Rust Guidelines](#unsafe-rust-guidelines)
9. [Common Pitfalls](#common-pitfalls)

---

## Ownership & Borrowing

Rust enforces memory safety at compile time through three rules:

1. Every value has exactly one owner.
2. You can have either one `&mut T` or any number of `&T` — never both.
3. References must always be valid (no dangling pointers).

### Move Semantics

```rust
let s1 = String::from("hello");
let s2 = s1;          // s1 is MOVED into s2
// println!("{s1}");   // compile error: s1 is no longer valid
```

Use `.clone()` when you need an independent copy. Prefer borrowing (`&`) when you don't need
ownership.

### Copy vs Clone

- `Copy`: bitwise copy, implicit (integers, floats, `bool`, `char`, tuples of `Copy` types)
- `Clone`: explicit `.clone()`, can be expensive (heap allocations)

```rust
// Copy — no move, both valid
let x: i32 = 5;
let y = x;  // x is still valid

// Clone — explicit deep copy
let a = vec![1, 2, 3];
let b = a.clone();  // a is still valid
```

### Borrowing Patterns

```rust
// Shared reference: read-only, multiple allowed
fn print_len(s: &str) {
    println!("{}", s.len());
}

// Mutable reference: read-write, exclusive
fn push_item(v: &mut Vec<i32>, item: i32) {
    v.push(item);
}

// Reborrowing: &mut T can be temporarily borrowed as &T
fn process(data: &mut Vec<i32>) {
    let len = data.len();  // implicit reborrow as &Vec<i32>
    data.push(len as i32); // mutable access resumes
}
```

### Ownership Transfer Patterns

When designing APIs, choose the right ownership model:

```rust
// Take ownership when you need to store or transform the value
fn consume(s: String) -> String { s.to_uppercase() }

// Borrow when you only need to read
fn inspect(s: &str) -> usize { s.len() }

// Borrow mutably when you need to modify in place
fn normalize(s: &mut String) { s.make_ascii_lowercase(); }

// Return owned data from constructors
fn create() -> Vec<i32> { vec![1, 2, 3] }
```

**See:** `crates/patterns/src/builder.rs` — type-safe builder that consumes `self` at each step,
enforcing required fields at compile time.

---

## Lifetimes

Lifetimes ensure references don't outlive the data they point to. The compiler infers most
lifetimes, but sometimes you must annotate them.

### Lifetime Elision Rules

The compiler applies three rules to infer lifetimes on `fn` signatures:

1. Each reference parameter gets its own lifetime: `fn f(a: &str, b: &str)` → `fn f<'a, 'b>(a: &'a str, b: &'b str)`
2. If there's exactly one input lifetime, it's assigned to all outputs: `fn f(s: &str) -> &str` → `fn f<'a>(s: &'a str) -> &'a str`
3. If one parameter is `&self` or `&mut self`, its lifetime is assigned to all outputs.

When elision doesn't apply, annotate explicitly:

```rust
// Return could come from either input — must annotate
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}
```

### Lifetime in Structs

A struct holding a reference must declare the lifetime:

```rust
struct Parser<'input> {
    source: &'input str,
    pos: usize,
}

impl<'input> Parser<'input> {
    fn new(source: &'input str) -> Self {
        Self { source, pos: 0 }
    }

    fn remaining(&self) -> &'input str {
        &self.source[self.pos..]
    }
}
```

### Common Lifetime Patterns

```rust
// 'static — lives for the entire program
let s: &'static str = "hello";  // string literals are 'static
static CONFIG: &str = "default";

// Bounded lifetimes — output tied to input
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

// Multiple lifetimes — when inputs have different scopes
fn select<'a, 'b>(a: &'a str, _b: &'b str) -> &'a str { a }
```

**See:** `crates/systems/src/memory.rs` — `Arena::alloc()` returns `&T` tied to the arena's
lifetime, preventing use-after-free at compile time.

---

## Error Handling Patterns

Rust uses `Result<T, E>` for recoverable errors and `panic!` for unrecoverable bugs.

### The Error Type Hierarchy

```
Application code
  └─ anyhow::Error  (ad-hoc, context-rich)
Library code
  └─ thiserror      (structured, typed enums)
```

### Idiomatic Error Handling

```rust
use common::{AppError, Result, ResultExt};

// Use ? for propagation — errors convert automatically via From
fn load_and_parse(path: &str) -> Result<serde_json::Value> {
    let data = std::fs::read_to_string(path)?;  // io::Error → AppError::Io
    let val = serde_json::from_str(&data)?;      // serde::Error → AppError::Serialization
    Ok(val)
}

// Add context with the ResultExt trait
fn init_config() -> Result<Config> {
    load_config().context_app("initializing application config")
}

// Use domain-specific constructors for business logic errors
fn validate_age(age: i32) -> Result<i32> {
    if age < 0 || age > 150 {
        return Err(AppError::validation(format!("invalid age: {age}")));
    }
    Ok(age)
}
```

**See:** `crates/common/src/error.rs` — full `AppError` enum with `thiserror`, `anyhow` bridge,
`ResultExt` context trait, and `is_client_error()` classification.

---

## Concurrency: Async/Await

Rust's async model is zero-cost: futures do nothing until polled. The runtime (tokio) drives
execution.

### Core Concepts

```rust
// async fn returns impl Future<Output = T>
async fn fetch(url: &str) -> String {
    reqwest::get(url).await.unwrap().text().await.unwrap()
}

// Spawn independent tasks
let handle = tokio::spawn(async {
    expensive_computation().await
});
let result = handle.await.unwrap();
```

### Structured Concurrency Patterns

**Fan-out/fan-in** — spawn N tasks, collect all results:

```rust
// See crates/hpc/src/async_runtime.rs — fan_out()
let results = fan_out(10, |i| async move { i * 2 }).await;
```

**Select/race** — first future to complete wins:

```rust
// See crates/hpc/src/async_runtime.rs — race()
let result = race(
    async { fetch_from_primary().await },
    async { fetch_from_fallback().await },
).await;
```

### Async Gotchas

1. **Don't hold locks across `.await`** — use `tokio::sync::Mutex` if you must, or restructure:

   ```rust
   // BAD: std::sync::Mutex held across await point
   let guard = mutex.lock().unwrap();
   do_async_work().await;  // other tasks can't acquire the lock
   drop(guard);

   // GOOD: release lock before awaiting
   let data = {
       let guard = mutex.lock().unwrap();
       guard.clone()
   };
   do_async_work_with(data).await;
   ```

2. **`Send` bounds** — spawned futures must be `Send`. Avoid `Rc`, `Cell`, non-Send types across
   await points.

3. **Blocking in async** — never call blocking I/O in async context. Use `tokio::task::spawn_blocking`:

   ```rust
   let result = tokio::task::spawn_blocking(|| {
       std::fs::read_to_string("large_file.txt")
   }).await.unwrap();
   ```

**See:** `crates/hpc/src/async_runtime.rs` — fan-out, producer-consumer, request-response, and
select patterns.

---

## Concurrency: Channels & Message Passing

Channels transfer ownership between tasks/threads — no shared mutable state needed.

### Tokio Channels

```rust
use tokio::sync::{mpsc, oneshot, broadcast, watch};

// mpsc — multiple producers, single consumer (bounded)
let (tx, mut rx) = mpsc::channel::<String>(100);
tokio::spawn(async move {
    tx.send("hello".into()).await.unwrap();
});
let msg = rx.recv().await;  // Some("hello")

// oneshot — single value, single use (request-response)
let (tx, rx) = oneshot::channel::<i32>();
tx.send(42).unwrap();
let val = rx.await.unwrap();  // 42

// broadcast — multiple consumers, each gets every message
let (tx, _) = broadcast::channel::<String>(16);
let mut rx1 = tx.subscribe();
let mut rx2 = tx.subscribe();
tx.send("event".into()).unwrap();

// watch — single latest value, multiple observers
let (tx, rx) = watch::channel(Config::default());
tx.send(new_config).unwrap();
let current = rx.borrow().clone();
```

### Choosing the Right Channel

| Pattern | Channel | Use Case |
|---------|---------|----------|
| Work queue | `mpsc` | Distribute tasks to a worker |
| Request-response | `oneshot` | Await a single reply |
| Event bus | `broadcast` | Notify all subscribers |
| Config/state | `watch` | Share latest value |

**See:** `crates/hpc/src/async_runtime.rs` — `producer_consumer()` (mpsc) and
`request_response()` (oneshot).

**See:** `crates/etl/src/streaming.rs` — `streaming_pipeline()` and `fan_out_pipeline()` using
bounded channels for backpressure.

---

## Concurrency: Shared State

When message passing isn't practical, use shared-state primitives.

### Arc + Mutex

```rust
use std::sync::{Arc, Mutex};

let counter = Arc::new(Mutex::new(0));
let handles: Vec<_> = (0..10).map(|_| {
    let counter = counter.clone();
    std::thread::spawn(move || {
        let mut num = counter.lock().unwrap();
        *num += 1;
    })
}).collect();

for h in handles { h.join().unwrap(); }
assert_eq!(*counter.lock().unwrap(), 10);
```

### RwLock — Multiple Readers, Single Writer

```rust
use std::sync::{Arc, RwLock};

let data = Arc::new(RwLock::new(vec![1, 2, 3]));

// Multiple readers concurrently
let reader = data.read().unwrap();
println!("{:?}", *reader);
drop(reader);

// Exclusive writer
let mut writer = data.write().unwrap();
writer.push(4);
```

### Atomics — Lock-Free Primitives

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

static REQUEST_COUNT: AtomicUsize = AtomicUsize::new(0);

fn handle_request() {
    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn get_count() -> usize {
    REQUEST_COUNT.load(Ordering::Relaxed)
}
```

### Choosing the Right Primitive

| Need | Primitive | Notes |
|------|-----------|-------|
| Simple counter/flag | `AtomicUsize`, `AtomicBool` | Lock-free, fastest |
| Shared mutable data | `Arc<Mutex<T>>` | Simple, blocks on contention |
| Read-heavy workloads | `Arc<RwLock<T>>` | Multiple concurrent readers |
| Async contexts | `tokio::sync::Mutex` | Won't block the runtime |
| Single-writer broadcast | `arc-swap` or `watch` | Lock-free reads |

**See:** `crates/systems/src/memory.rs` — `Guard` uses `Arc<AtomicBool>` in tests to verify
cleanup runs on drop.

---

## Parallel Processing

For CPU-bound work, use Rayon's work-stealing thread pool instead of manual thread management.

### Rayon Parallel Iterators

```rust
use rayon::prelude::*;

// Drop-in replacement for iterator chains
let sum: f64 = data.par_iter().map(|x| x * x).sum();

// Parallel sort
data.par_sort_unstable();

// Parallel fold + reduce for custom aggregation
let result = data.par_iter()
    .fold(|| HashMap::new(), |mut acc, item| {
        *acc.entry(item.key()).or_insert(0) += 1;
        acc
    })
    .reduce(|| HashMap::new(), |mut a, b| {
        for (k, v) in b { *a.entry(k).or_insert(0) += v; }
        a
    });
```

### When to Use Rayon vs Tokio

| Workload | Use | Why |
|----------|-----|-----|
| CPU-bound computation | Rayon | Work-stealing, no async overhead |
| I/O-bound (network, disk) | Tokio | Non-blocking, scales to thousands of tasks |
| Mixed | Both | `spawn_blocking` bridges async → sync |

**See:** `crates/hpc/src/parallel.rs` — parallel sum, sort, filter-map, fold/reduce, custom
thread pools.

**See:** `crates/etl/src/parallel.rs` — `par_map_reduce`, `par_group_sum`, `par_batch_process`.

---

## Unsafe Rust Guidelines

`unsafe` doesn't disable the borrow checker — it lets you do five specific things the compiler
can't verify:

1. Dereference raw pointers
2. Call unsafe functions
3. Access mutable statics
4. Implement unsafe traits
5. Access fields of `union`s

### Rules for Writing Unsafe Code

1. **Minimize unsafe surface area.** Wrap unsafe operations in safe abstractions:

   ```rust
   // Public API is safe — unsafe is an implementation detail
   pub fn swap<T>(a: &mut T, b: &mut T) {
       unsafe { std::ptr::swap(a, b) }
   }
   ```

2. **Document safety invariants.** Every `unsafe` block needs a `// SAFETY:` comment:

   ```rust
   // SAFETY: len < cap, so ptr.add(len) is within the allocation
   unsafe { self.ptr.add(self.len).write(val) };
   ```

3. **Uphold all invariants.** Unsafe code must maintain:
   - No aliased `&mut` references
   - All references are valid and aligned
   - No data races
   - No use-after-free or double-free
   - Initialized memory for reads

4. **Implement `Drop` correctly.** If you allocate, you must deallocate. If you write, you must
   drop-in-place before deallocating:

   ```rust
   impl<T> Drop for RawStack<T> {
       fn drop(&mut self) {
           for i in 0..self.len {
               unsafe { self.ptr.add(i).drop_in_place() };
           }
           unsafe { std::alloc::dealloc(self.ptr as *mut u8, layout) };
       }
   }
   ```

5. **Send/Sync must be justified.** Only implement these unsafe traits when your type truly
   guarantees thread safety:

   ```rust
   // SAFETY: HeapVal owns its data exclusively — no shared mutable access.
   unsafe impl<T: Send> Send for HeapVal<T> {}
   unsafe impl<T: Sync> Sync for HeapVal<T> {}
   ```

### FFI Safety Checklist

- Validate all pointers from C before dereferencing
- Handle null pointers explicitly
- Ensure correct calling convention (`extern "C"`)
- Match C ABI types exactly (`c_int`, `c_char`, etc.)
- Free memory on the same side that allocated it

**See:** `crates/systems/src/unsafe_rust.rs` — `RawStack`, `raw_swap`, `DeepSizeOf` unsafe trait.

**See:** `crates/systems/src/ffi.rs` — libc wrappers, `extern "C"` exports, closure trampolines.

**See:** `crates/systems/src/memory.rs` — `Arena` bump allocator, `HeapVal` manual Box, RAII
`Guard`.

---

## Common Pitfalls

### 1. Borrowing from a Temporary

```rust
// BAD: temporary String dropped at end of statement
// let s: &str = String::from("hello").as_str();

// GOOD: bind the String first
let owned = String::from("hello");
let s: &str = owned.as_str();
```

### 2. Self-Referential Structs

Rust's ownership model prevents structs from holding references to their own fields. Solutions:

- Use indices instead of references
- Use `Pin` + `unsafe` (advanced)
- Use crates like `ouroboros` or `self_cell`

### 3. Iterator Invalidation

```rust
// BAD: can't mutate while iterating
// for item in &vec { vec.push(item.clone()); }

// GOOD: collect first, then extend
let new_items: Vec<_> = vec.iter().cloned().collect();
vec.extend(new_items);
```

### 4. Deadlocks with Multiple Locks

```rust
// BAD: inconsistent lock ordering → deadlock risk
// Thread 1: lock(a) then lock(b)
// Thread 2: lock(b) then lock(a)

// GOOD: always acquire locks in the same order, or use a single coarser lock
```

### 5. Accidental Cloning in Hot Paths

```rust
// BAD: cloning in a tight loop
// for item in &data { process(item.clone()); }

// GOOD: borrow instead
for item in &data { process(item); }
```

---

## Further Reading

- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — unsafe Rust reference
- [Rust Async Book](https://rust-lang.github.io/async-book/) — async/await patterns
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) — async runtime guide
- [Rayon FAQ](https://github.com/rayon-rs/rayon/blob/main/FAQ.md) — parallel processing
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — idiomatic API design

## See Also

- [TUTORIAL.md](TUTORIAL.md) — new developer walkthrough
- [ARCHITECTURE.md](ARCHITECTURE.md) — workspace layout and crate structure
- [TOOLCHAIN.md](TOOLCHAIN.md) — required tools and editor setup
- [EXTENDING.md](EXTENDING.md) — adding crates, dependencies, feature flags
- [SECURITY_SCANNING.md](SECURITY_SCANNING.md) — security tools and CI integration
- [cli.md](cli.md) — CLI development patterns
