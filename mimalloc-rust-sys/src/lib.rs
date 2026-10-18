//! Raw FFI bindings to mimalloc, compiled from the vendored sources in `c_src/mimalloc`.
//!
//! The bindings cover the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.
//! Macros and `static inline` helpers (such as `mi_stats_init` and the `*_csize`
//! variants) are not bound; see `UPSTREAM.md`.
//!
//! Build scripts of crates that depend on this crate directly can read the directory of
//! the matching headers from `DEP_MIMALLOC_INCLUDE_DIR`, for example to compile C code that
//! includes `mimalloc.h`. Cargo does not pass it to crates further down the graph.
//!
//! # Using the bindings
//!
//! The authoritative contracts are the comments in `c_src/mimalloc/include/mimalloc.h`
//! at the vendored tag. The rules below are the ones that are easy to get wrong from Rust:
//!
//! - Release blocks from `mi_*` functions with `mi_free` or another `mi_*` function, not
//!   with the C library's `free`, unless the `override` feature has replaced `free`.
//! - `mi_free_small` and `mi_free_small_nonnull` only accept blocks from `mi_*alloc_small`.
//! - A `mi_heap_t` can be used and its blocks freed from any thread. `mi_heap_delete`
//!   moves live blocks to the main heap, while `mi_heap_destroy` frees them, leaving every
//!   outstanding pointer into that heap dangling.
//! - A `mi_theap_t` can only allocate on the thread that created it.
//! - Callbacks and their `arg` registered with `mi_register_deferred_free`,
//!   `mi_register_output` and `mi_register_error` are process-global and run on whichever
//!   thread triggers them, so `arg` must be safe to use from every thread. Each callback
//!   and its `arg` are stored separately, so register each one once, from a single thread,
//!   during initialization: replacing one while it can run may pair a callback with the
//!   wrong `arg`. Registering again does not wait for calls in progress, so whatever `arg`
//!   points to must outlive every call that can still reach it.
//! - The deferred free callback runs from allocation and collection, and is not re-entered
//!   on the same thread.
//! - A registered error callback runs after the message is printed and replaces the
//!   default handling, which sets `errno` and, with the `debug` or `secure` feature, aborts
//!   on corrupted metadata (`EFAULT`). If the callback returns, the failing call continues
//!   on its own error path without that handling.
//! - Unwinding out of an `extern "C"` callback aborts the process, so a panic in one is
//!   fatal.
//! - The `*_stats_get` functions return `false` unless the `size` and `version` fields of the
//!   destination already match, which the unbound `mi_stats_init` would arrange: zero a
//!   `mi_stats_t`, then set `size` to its size and `version` to [`MI_STAT_VERSION`].

#![no_std]
#![allow(non_camel_case_types, non_upper_case_globals)]

// rustc places a crate before its dependencies on the link line but leaves unrelated
// crates in load order. Depending on std puts the override archive before the static
// `libc.a` on musl, which otherwise can come first (for example in the test binary of a
// `no_std` crate) and fail the link with duplicate `malloc` definitions.
#[cfg(feature = "override")]
extern crate std;

/// Whether the library was built with `MI_GUARDED`, which the `debug` feature enables.
///
/// With guarded sampling and the `guarded_precise` option enabled at run time, `mi_malloc`
/// can return blocks that are not 16-byte aligned. Features are unified across the dependency
/// graph, so dependents should read this instead of their own `debug` feature.
pub const MI_GUARDED: bool = cfg!(feature = "debug");

/// C `wchar_t`, used by `mi_wcsdup` and `mi_wdupenv_s`.
///
/// Generated bindings would pin the signedness of whichever target ran bindgen.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub type wchar_t = core::ffi::c_uint;
/// C `wchar_t`, used by `mi_wcsdup` and `mi_wdupenv_s`.
///
/// Generated bindings would pin the signedness of whichever target ran bindgen.
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
pub type wchar_t = core::ffi::c_int;

include!("bindings.rs");
