//! With `override`, the C allocation functions of the executable are mimalloc's, so
//! allocations made by C code, inside libc and by `std::alloc::System` all come from it.
#![cfg(feature = "override")]

use core::ffi::{c_char, c_int, c_void};
use core::hint::black_box;
use std::alloc::{GlobalAlloc, Layout, System};

use mimalloc_rust_sys::mi_any_heap_contains;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn posix_memalign(out: *mut *mut c_void, alignment: usize, size: usize) -> c_int;
    fn free(ptr: *mut c_void);
    fn asprintf(out: *mut *mut c_char, format: *const c_char, ...) -> c_int;
}

// `black_box` keeps the optimizer from reasoning about known malloc/free pairs.
fn from_mimalloc<T>(ptr: *const T) -> bool {
    unsafe { mi_any_heap_contains(black_box(ptr.cast())) }
}

#[test]
fn c_allocation_functions_return_mimalloc_blocks() {
    let block = unsafe { malloc(100) };
    assert!(from_mimalloc(block));

    let block = unsafe { realloc(block, 100_000) };
    assert!(from_mimalloc(block));
    unsafe { free(block) };

    let zeroed = unsafe { calloc(10, 10) };
    assert!(from_mimalloc(zeroed));
    unsafe { free(zeroed) };

    let mut aligned = core::ptr::null_mut();
    assert_eq!(unsafe { posix_memalign(&raw mut aligned, 4096, 100) }, 0);
    assert!(from_mimalloc(aligned));
    assert_eq!(aligned.addr() % 4096, 0);
    unsafe { free(aligned) };
}

// mimalloc replaces `strdup` and friends directly, so this uses `asprintf`, which libc
// implements itself on top of `malloc`.
#[test]
fn allocations_inside_libc_come_from_mimalloc() {
    let mut text = core::ptr::null_mut();
    assert!(unsafe { asprintf(&raw mut text, c"%s %d".as_ptr(), c"libc".as_ptr(), 42) } > 0);
    assert!(from_mimalloc(text));
    unsafe { free(text.cast()) };
}

#[test]
fn system_allocator_uses_mimalloc() {
    for align in [8, 4096] {
        let layout = Layout::from_size_align(64, align).unwrap();
        let block = unsafe { System.alloc(layout) };
        assert!(from_mimalloc(block), "{layout:?}");
        unsafe { System.dealloc(block, layout) };
    }
}
