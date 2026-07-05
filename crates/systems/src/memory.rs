//! Memory management: arena allocator, RAII guards, manual layout control.

use std::alloc::{self, Layout};
use std::cell::Cell;
use std::marker::PhantomData;

// --- Arena allocator ---

/// A simple bump allocator that allocates from a contiguous block.
/// All allocations are freed at once when the arena is dropped.
pub struct Arena {
    buf: *mut u8,
    layout: Layout,
    offset: Cell<usize>,
}

impl Arena {
    /// Create an arena with `capacity` bytes.
    ///
    /// # Panics
    /// Panics if `capacity` is zero or the allocation fails.
    pub fn new(capacity: usize) -> Self {
        // `alloc` with a zero-size layout is undefined behavior, so reject it up front.
        assert!(capacity > 0, "arena capacity must be non-zero");
        let layout = Layout::from_size_align(capacity, 16).unwrap();
        // SAFETY: `layout` has non-zero size (asserted above).
        let buf = unsafe { alloc::alloc(layout) };
        assert!(!buf.is_null(), "arena allocation failed");
        Self {
            buf,
            layout,
            offset: Cell::new(0),
        }
    }

    /// Allocate a `T` in the arena and return a reference tied to the arena's lifetime.
    ///
    /// # Panics
    /// Panics if the arena is out of space or `T` requires alignment above 16.
    pub fn alloc<T>(&self, val: T) -> &T {
        let align = std::mem::align_of::<T>();
        let size = std::mem::size_of::<T>();
        // The buffer base is only 16-byte aligned, so aligning the *offset* can
        // only guarantee alignment up to 16. Reject stricter types (e.g. repr(align(32)))
        // rather than hand out a misaligned pointer.
        assert!(align <= 16, "arena supports alignment up to 16 bytes");
        let off = align_up(self.offset.get(), align);
        assert!(off + size <= self.layout.size(), "arena out of memory");
        self.offset.set(off + size);
        // SAFETY: `off` is aligned for `T` and `off + size` fits in the live buffer.
        // The bump offset only moves forward, so this range never overlaps a prior
        // allocation, and the returned reference borrows `self`, keeping the buffer alive.
        unsafe {
            let ptr = self.buf.add(off) as *mut T;
            ptr.write(val);
            &*ptr
        }
    }

    /// Bytes remaining in the arena.
    pub fn remaining(&self) -> usize {
        self.layout.size() - self.offset.get()
    }

    /// Reset the arena, reclaiming all memory. Existing references become invalid.
    ///
    /// # Safety
    /// Caller must ensure no references from prior allocations are still in use.
    /// Does NOT run destructors on allocated values.
    pub unsafe fn reset(&self) {
        self.offset.set(0);
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        // NOTE: does not run destructors on allocated values — arena is for POD-like types.
        // SAFETY: `buf` was allocated in `new` with exactly `self.layout` and is freed
        // only here, once.
        unsafe { alloc::dealloc(self.buf, self.layout) };
    }
}

fn align_up(offset: usize, align: usize) -> usize {
    (offset + align - 1) & !(align - 1)
}

// --- RAII guard pattern ---

/// A guard that runs a cleanup closure on drop. Useful for managing
/// resources that don't have a Rust wrapper (file descriptors, C handles, etc.).
pub struct Guard<F: FnOnce()> {
    cleanup: Option<F>,
}

impl<F: FnOnce()> Guard<F> {
    /// Create a guard that will run `cleanup` when dropped.
    pub fn new(cleanup: F) -> Self {
        Self {
            cleanup: Some(cleanup),
        }
    }

    /// Disarm the guard, preventing cleanup from running.
    pub fn disarm(&mut self) {
        self.cleanup = None;
    }
}

impl<F: FnOnce()> Drop for Guard<F> {
    fn drop(&mut self) {
        if let Some(f) = self.cleanup.take() {
            f();
        }
    }
}

// --- Typed heap allocation with manual layout ---

/// A single heap-allocated value with explicit layout control.
/// Like `Box<T>` but demonstrates manual alloc/dealloc.
pub struct HeapVal<T> {
    ptr: *mut T,
    _marker: PhantomData<T>,
}

impl<T> HeapVal<T> {
    /// Allocate `val` on the heap with explicit layout control.
    ///
    /// # Panics
    /// Panics if `T` is zero-sized or the allocation fails.
    pub fn new(val: T) -> Self {
        let layout = Layout::new::<T>();
        // `alloc` with a zero-size layout is undefined behavior; a real Box uses a
        // dangling pointer for ZSTs, but that machinery would obscure this example.
        assert!(layout.size() > 0, "zero-sized types are not supported");
        // SAFETY: `layout` has non-zero size (asserted above).
        let ptr = unsafe { alloc::alloc(layout) as *mut T };
        assert!(!ptr.is_null());
        // SAFETY: `ptr` is non-null, aligned for `T`, and points to uninitialized
        // memory owned by us.
        unsafe { ptr.write(val) };
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Returns a shared reference to the contained value.
    pub fn get(&self) -> &T {
        // SAFETY: `ptr` was initialized in `new` and stays valid until drop; the
        // returned reference borrows `self`, so it cannot outlive the value.
        unsafe { &*self.ptr }
    }

    /// Returns an exclusive reference to the contained value.
    pub fn get_mut(&mut self) -> &mut T {
        // SAFETY: as in `get`, plus `&mut self` guarantees the reference is unique.
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for HeapVal<T> {
    fn drop(&mut self) {
        // SAFETY: `ptr` was allocated and initialized in `new` with this exact
        // layout; the value is dropped and the memory freed exactly once, here.
        unsafe {
            self.ptr.drop_in_place();
            alloc::dealloc(self.ptr as *mut u8, Layout::new::<T>());
        }
    }
}

// SAFETY: HeapVal owns its data exclusively.
unsafe impl<T: Send> Send for HeapVal<T> {}
unsafe impl<T: Sync> Sync for HeapVal<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc() {
        let arena = Arena::new(1024);
        let a = arena.alloc(42u64);
        let b = arena.alloc(3.14f64);
        let c = arena.alloc([1u8, 2, 3, 4]);
        assert_eq!(*a, 42);
        assert_eq!(*b, 3.14);
        assert_eq!(*c, [1, 2, 3, 4]);
        assert!(arena.remaining() < 1024);
    }

    #[test]
    #[should_panic(expected = "arena out of memory")]
    fn test_arena_oom() {
        let arena = Arena::new(16);
        let _ = arena.alloc([0u8; 32]); // too big
    }

    #[test]
    #[should_panic(expected = "arena capacity must be non-zero")]
    fn test_arena_zero_capacity_rejected() {
        let _ = Arena::new(0);
    }

    #[test]
    #[should_panic(expected = "arena supports alignment up to 16")]
    fn test_arena_overaligned_type_rejected() {
        #[repr(align(32))]
        struct Overaligned(#[allow(dead_code)] u8);
        let arena = Arena::new(1024);
        let _ = arena.alloc(Overaligned(1));
    }

    #[test]
    fn test_arena_reset() {
        let arena = Arena::new(64);
        let _ = arena.alloc(1u64);
        assert!(arena.remaining() < 64);
        // SAFETY: no references from prior allocations are used after this point.
        unsafe { arena.reset() };
        assert_eq!(arena.remaining(), 64);
        assert_eq!(*arena.alloc(2u64), 2);
    }

    #[test]
    fn test_guard_runs_cleanup() {
        let cleaned = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = cleaned.clone();
        {
            let _g = Guard::new(move || c.store(true, std::sync::atomic::Ordering::SeqCst));
        }
        assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_guard_disarm() {
        let cleaned = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = cleaned.clone();
        {
            let mut g = Guard::new(move || c.store(true, std::sync::atomic::Ordering::SeqCst));
            g.disarm();
        }
        assert!(!cleaned.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_heap_val() {
        let mut h = HeapVal::new(String::from("hello"));
        assert_eq!(h.get(), "hello");
        h.get_mut().push_str(" world");
        assert_eq!(h.get(), "hello world");
    }
}
