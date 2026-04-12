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
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 16).unwrap();
        let buf = unsafe { alloc::alloc(layout) };
        assert!(!buf.is_null(), "arena allocation failed");
        Self {
            buf,
            layout,
            offset: Cell::new(0),
        }
    }

    /// Allocate a `T` in the arena and return a reference tied to the arena's lifetime.
    pub fn alloc<T>(&self, val: T) -> &T {
        let align = std::mem::align_of::<T>();
        let size = std::mem::size_of::<T>();
        let off = align_up(self.offset.get(), align);
        assert!(off + size <= self.layout.size(), "arena out of memory");
        self.offset.set(off + size);
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
    pub fn new(val: T) -> Self {
        let layout = Layout::new::<T>();
        let ptr = unsafe { alloc::alloc(layout) as *mut T };
        assert!(!ptr.is_null());
        unsafe { ptr.write(val) };
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Returns a shared reference to the contained value.
    pub fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }

    /// Returns an exclusive reference to the contained value.
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for HeapVal<T> {
    fn drop(&mut self) {
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
