//! A global allocator backed by a vendored, statically linked mimalloc v3.
//!
//! ```no_run
//! use mimalloc_rust::MiMalloc;
//!
//! #[global_allocator]
//! static GLOBAL: MiMalloc = MiMalloc;
//! ```
//!
//! Other mimalloc functionality is available as raw bindings in `mimalloc-rust-sys`.

#![no_std]

mod stats;

use core::alloc::{GlobalAlloc, Layout};

use mimalloc_rust_sys as ffi;

pub use stats::{StatsJson, stats_json};

/// The mimalloc allocator, intended for `#[global_allocator]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MiMalloc;

// SAFETY: the `mi_*_aligned` functions return null or a block of at least `size` bytes
// aligned to `align`, and `mi_free` releases any such block from any thread.
unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `Layout` guarantees a non-zero power-of-two alignment, the only
        // requirement mimalloc places on `alignment`.
        unsafe { ffi::mi_malloc_aligned(layout.size(), layout.align()).cast() }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as in `alloc`.
        unsafe { ffi::mi_zalloc_aligned(layout.size(), layout.align()).cast() }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: the caller guarantees `ptr` is a live block from this allocator;
        // `mi_free` needs no size or alignment.
        unsafe { ffi::mi_free(ptr.cast()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees `ptr` is a live block allocated with `layout` and
        // that `new_size` is valid for `layout.align()`. On failure mimalloc returns null
        // and leaves the original block allocated, as `GlobalAlloc::realloc` requires.
        unsafe { ffi::mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast() }
    }
}

/// Returns the version of the linked mimalloc, encoded as `major * 10000 + minor * 100 + patch`.
pub fn version() -> u32 {
    // SAFETY: `mi_version` has no preconditions.
    let version = unsafe { ffi::mi_version() };
    u32::try_from(version).expect("mimalloc reports a non-negative version")
}

/// Returns the usable size of a block, which can be larger than the size requested for it.
///
/// # Safety
///
/// `ptr` must point to a live block allocated by mimalloc, for example through [`MiMalloc`].
pub unsafe fn usable_size(ptr: *const u8) -> usize {
    // SAFETY: forwarded to the caller.
    unsafe { ffi::mi_usable_size(ptr.cast()) }
}
