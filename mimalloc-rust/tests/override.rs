//! With `override`, the C allocation functions of the executable are mimalloc's, so
//! allocations made by C code, by libc itself and by `std::alloc::System` all come from it.
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
    fn strdup(s: *const c_char) -> *mut c_char;
}

// `black_box` keeps the optimizer from reasoning about known malloc/free pairs.
fn from_mimalloc<T>(ptr: *const T) -> bool {
    unsafe { mi_any_heap_contains(black_box(ptr.cast())) }
}

#[test]
fn c_allocation_functions_return_mimalloc_blocks() {
    unsafe {
        let block = malloc(100);
        assert!(from_mimalloc(block));

        let block = realloc(block, 100_000);
        assert!(from_mimalloc(block));
        free(block);

        let zeroed = calloc(10, 10);
        assert!(from_mimalloc(zeroed));
        free(zeroed);

        let mut aligned = core::ptr::null_mut();
        assert_eq!(posix_memalign(&mut aligned, 4096, 100), 0);
        assert!(from_mimalloc(aligned));
        assert_eq!(aligned.addr() % 4096, 0);
        free(aligned);
    }
}

#[test]
fn allocations_inside_libc_come_from_mimalloc() {
    unsafe {
        let copy = strdup(c"allocated by libc".as_ptr());
        assert!(from_mimalloc(copy));
        free(copy.cast());
    }
}

#[test]
fn system_allocator_uses_mimalloc() {
    for layout in [Layout::from_size_align(64, 8), Layout::from_size_align(64, 4096)] {
        let layout = layout.unwrap();
        unsafe {
            let block = System.alloc(layout);
            assert!(from_mimalloc(block), "{layout:?}");
            System.dealloc(block, layout);
        }
    }
}
