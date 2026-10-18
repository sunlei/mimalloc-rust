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

/// Whether `mi_malloc(size)` already returns blocks aligned to `align`.
///
/// Size classes above 8 bytes are multiples of 16 bytes and pages start their blocks at a
/// multiple of 16, so every such block is 16-byte aligned; the 8-byte class only serves
/// requests of at most 8 bytes. This is the condition std's `System` uses on Unix to pick
/// `malloc`. The `mi_*_aligned` functions would also work, but on a fast-path miss they
/// over-allocate any size class that is not a power of two, which costs time and memory
/// (see `docs/known-issues.md`).
#[inline]
const fn mi_malloc_aligns(size: usize, align: usize) -> bool {
    align <= ffi::MI_MALLOC_ALIGN && align <= size
}

// SAFETY: `mi_malloc`, `mi_zalloc` and `mi_realloc` are only used when `mi_malloc_aligns`
// holds, and otherwise the `mi_*_aligned` functions. Both return null or a block of at least
// `size` bytes aligned to `align`, and `mi_free` releases either from any thread.
unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let block = if mi_malloc_aligns(layout.size(), layout.align()) {
            // SAFETY: `mi_malloc` has no preconditions.
            unsafe { ffi::mi_malloc(layout.size()) }
        } else {
            // SAFETY: `Layout` guarantees a non-zero power-of-two alignment, the only
            // requirement mimalloc places on `alignment`.
            unsafe { ffi::mi_malloc_aligned(layout.size(), layout.align()) }
        };
        block.cast()
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let block = if mi_malloc_aligns(layout.size(), layout.align()) {
            // SAFETY: as in `alloc`.
            unsafe { ffi::mi_zalloc(layout.size()) }
        } else {
            // SAFETY: as in `alloc`.
            unsafe { ffi::mi_zalloc_aligned(layout.size(), layout.align()) }
        };
        block.cast()
    }

    // Not `mi_free_size_aligned`: it measured no faster in the bench crate (same thread and
    // cross-thread, Apple M1 Max), and in release builds a wrong `layout` would then corrupt
    // the heap silently instead of being ignored.
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: the caller guarantees `ptr` is a live block from this allocator;
        // `mi_free` needs no size or alignment.
        unsafe { ffi::mi_free(ptr.cast()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // In both branches, the caller guarantees `ptr` is a live block allocated with
        // `layout` and that `new_size` is valid for `layout.align()`. On failure mimalloc
        // returns null and leaves the original block allocated, as `GlobalAlloc::realloc`
        // requires.
        let block = if mi_malloc_aligns(new_size, layout.align()) {
            // SAFETY: see above. `mi_realloc` either keeps `ptr`, which is aligned to
            // `layout.align()`, or moves the contents to a block from `mi_malloc`, which
            // `mi_malloc_aligns` covers. It accepts blocks from the aligned functions too.
            unsafe { ffi::mi_realloc(ptr.cast(), new_size) }
        } else {
            // SAFETY: see above.
            unsafe { ffi::mi_realloc_aligned(ptr.cast(), new_size, layout.align()) }
        };
        block.cast()
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
