//! Unsafe Rust patterns: raw pointers, unsafe blocks, unsafe traits.

use std::ptr;

/// Swap two values via raw pointers. Demonstrates `unsafe` pointer dereference.
///
/// # Safety
/// Both pointers must be valid, aligned, and non-overlapping.
pub unsafe fn raw_swap<T>(a: *mut T, b: *mut T) {
    ptr::swap(a, b);
}

/// A fixed-capacity stack using raw pointer arithmetic.
pub struct RawStack<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> RawStack<T> {
    /// Create a stack with the given fixed capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0);
        let layout = std::alloc::Layout::array::<T>(cap).unwrap();
        // SAFETY: layout has non-zero size (cap > 0, T is sized)
        let ptr = unsafe { std::alloc::alloc(layout) as *mut T };
        assert!(!ptr.is_null(), "allocation failed");
        Self { ptr, len: 0, cap }
    }

    /// Push a value onto the stack. Returns `Err(val)` if full.
    pub fn push(&mut self, val: T) -> Result<(), T> {
        if self.len == self.cap {
            return Err(val);
        }
        // SAFETY: len < cap, so ptr.add(len) is within allocation
        unsafe { self.ptr.add(self.len).write(val) };
        self.len += 1;
        Ok(())
    }

    /// Pop the top value off the stack, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        // SAFETY: element at len was previously written
        Some(unsafe { self.ptr.add(self.len).read() })
    }

    /// Returns the number of elements currently on the stack.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the stack contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Drop for RawStack<T> {
    fn drop(&mut self) {
        // Drop remaining elements
        for i in 0..self.len {
            unsafe { self.ptr.add(i).drop_in_place() };
        }
        let layout = std::alloc::Layout::array::<T>(self.cap).unwrap();
        unsafe { std::alloc::dealloc(self.ptr as *mut u8, layout) };
    }
}

// SAFETY: RawStack owns its data exclusively, safe to send across threads.
unsafe impl<T: Send> Send for RawStack<T> {}

/// Trait for types that can report their in-memory size including heap allocations.
///
/// # Safety
/// Implementors must return an accurate byte count.
pub unsafe trait DeepSizeOf {
    fn deep_size(&self) -> usize;
}

unsafe impl DeepSizeOf for String {
    fn deep_size(&self) -> usize {
        std::mem::size_of::<String>() + self.capacity()
    }
}

unsafe impl<T> DeepSizeOf for Vec<T> {
    fn deep_size(&self) -> usize {
        std::mem::size_of::<Vec<T>>() + self.capacity() * std::mem::size_of::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_swap() {
        let mut a = 10u32;
        let mut b = 20u32;
        unsafe { raw_swap(&mut a, &mut b) };
        assert_eq!(a, 20);
        assert_eq!(b, 10);
    }

    #[test]
    fn test_raw_stack() {
        let mut s = RawStack::new(3);
        assert!(s.push(1).is_ok());
        assert!(s.push(2).is_ok());
        assert!(s.push(3).is_ok());
        assert!(s.push(4).is_err()); // full
        assert_eq!(s.pop(), Some(3));
        assert_eq!(s.pop(), Some(2));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_raw_stack_drop_with_heap_types() {
        let mut s = RawStack::new(2);
        s.push(String::from("hello")).unwrap();
        s.push(String::from("world")).unwrap();
        // Drop should clean up both strings and the allocation
        drop(s);
    }

    #[test]
    fn test_deep_size_of() {
        let s = String::with_capacity(100);
        let size = unsafe { DeepSizeOf::deep_size(&s) };
        assert!(size >= std::mem::size_of::<String>() + 100);

        let v: Vec<u64> = Vec::with_capacity(50);
        let size = unsafe { DeepSizeOf::deep_size(&v) };
        assert!(size >= std::mem::size_of::<Vec<u64>>() + 50 * 8);
    }
}
