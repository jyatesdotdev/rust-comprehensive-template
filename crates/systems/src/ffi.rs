//! FFI bindings: calling C from Rust, callback patterns, CString handling.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// --- Calling libc functions from Rust ---

/// Get the current process ID via libc.
pub fn getpid() -> i32 {
    // SAFETY: getpid takes no arguments, has no preconditions, and cannot fail.
    unsafe { libc::getpid() }
}

/// Get an environment variable via libc `getenv`. Returns `None` if unset.
pub fn getenv(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    // SAFETY: `c_name` is a valid NUL-terminated string. When getenv returns
    // non-null it points to a valid NUL-terminated string; we copy it to an owned
    // String before returning because later env mutations may invalidate it.
    unsafe {
        let ptr = libc::getenv(c_name.as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
        }
    }
}

/// Get system page size via libc `sysconf`.
pub fn page_size() -> usize {
    // SAFETY: sysconf has no memory-safety preconditions; _SC_PAGESIZE is a valid name.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

// --- Exposing Rust functions to C ---

/// A function with C ABI that can be called from C code.
/// Computes the length of a null-terminated C string.
///
/// # Safety
/// `s` must be a valid, null-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn rust_strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    CStr::from_ptr(s).to_bytes().len()
}

// --- Callback pattern: passing Rust closures to C-style APIs ---

/// Simulates a C-style API that takes a callback function pointer.
/// Calls `cb` with each element of `data`.
pub fn c_style_foreach(data: &[i32], cb: extern "C" fn(i32)) {
    for &item in data {
        cb(item);
    }
}

/// Wrapper that accepts a Rust closure and invokes it through a C-style
/// trampoline using a `*mut` context pointer.
pub fn foreach_with_closure<F: FnMut(i32)>(data: &[i32], mut f: F) {
    // Trampoline: extern "C" function that casts context back to closure
    extern "C" fn trampoline<F: FnMut(i32)>(val: i32, ctx: *mut std::ffi::c_void) {
        // SAFETY: `ctx` was created below from `&mut f`, which outlives every call
        // made in the loop, and no other reference to `f` exists while the
        // trampoline runs — so the reborrow is unique and valid.
        let f = unsafe { &mut *(ctx as *mut F) };
        f(val);
    }

    let ctx = &mut f as *mut F as *mut std::ffi::c_void;
    for &item in data {
        trampoline::<F>(item, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_getpid() {
        let pid = getpid();
        assert!(pid > 0);
    }

    #[test]
    fn test_getenv() {
        // PATH should exist on any Unix system
        assert!(getenv("PATH").is_some());
        assert!(getenv("__NONEXISTENT_VAR_12345__").is_none());
    }

    #[test]
    fn test_page_size() {
        let ps = page_size();
        assert!(ps >= 4096);
        assert!(ps.is_power_of_two());
    }

    #[test]
    fn test_rust_strlen() {
        let s = CString::new("hello").unwrap();
        let len = unsafe { rust_strlen(s.as_ptr()) };
        assert_eq!(len, 5);
        assert_eq!(unsafe { rust_strlen(std::ptr::null()) }, 0);
    }

    #[test]
    fn test_foreach_with_closure() {
        let data = [1, 2, 3, 4, 5];
        let mut sum = 0i32;
        foreach_with_closure(&data, |v| sum += v);
        assert_eq!(sum, 15);
    }
}
