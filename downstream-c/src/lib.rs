//! A dependent of `mimalloc-rust-sys` with its own C code, checked by its tests only.
//!
//! It enables `override`, so it stays out of the workspace's default members: building it
//! together with them would unify the feature into their tests.

#![cfg(test)]

use core::ffi::{c_int, c_void};
use core::hint::black_box;

use mimalloc_rust_sys::{MI_MALLOC_VERSION, mi_any_heap_contains};

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn dep_malloc(size: usize) -> *mut c_void;
    fn dep_free(ptr: *mut c_void);
    fn dep_mimalloc_version() -> c_int;
}

// `black_box` keeps the optimizer from reasoning about known malloc/free pairs.
fn from_mimalloc(ptr: *mut c_void) -> bool {
    unsafe { mi_any_heap_contains(black_box(ptr).cast_const()) }
}

#[test]
fn c_code_compiles_against_the_bound_headers() {
    let version = unsafe { dep_mimalloc_version() };
    assert_eq!(u32::try_from(version), Ok(MI_MALLOC_VERSION));
}

// Checking that both sides resolve to the same `malloc` would also pass if neither used
// mimalloc, so this checks that the library's blocks come from mimalloc.
#[test]
fn blocks_cross_a_shared_library_boundary() {
    let block = unsafe { dep_malloc(100) };
    assert!(from_mimalloc(block));
    unsafe { free(block) };

    let block = unsafe { malloc(100) };
    assert!(from_mimalloc(block));
    unsafe { dep_free(block) };
}
