//! Raw FFI bindings to mimalloc, compiled from the vendored sources in `c_src/mimalloc`.
//!
//! The bindings cover the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.
//! Macros and `static inline` helpers (such as `mi_stats_init` and the `*_csize`
//! variants) are not bound; see `UPSTREAM.md`.
//!
//! Build scripts of crates that depend on this crate directly can read the directory of
//! the matching headers from `DEP_MIMALLOC_INCLUDE_DIR`, for example to compile C code that
//! includes `mimalloc.h`. C code that includes the internal headers under `mimalloc/` also
//! needs the defines the library was compiled with, which `DEP_MIMALLOC_DEFINES` lists as
//! space-separated `NAME=VALUE` pairs. Cargo does not pass either variable to crates further
//! down the graph.
//!
//! # Using the bindings
//!
//! The authoritative contracts are the comments in `c_src/mimalloc/include/mimalloc.h`
//! at the vendored tag. The rules below are the ones that are easy to get wrong from Rust:
//!
//! - Release blocks from `mi_*` functions with `mi_free` or another `mi_*` function, not
//!   with the C library's `free`, unless the `override` feature has replaced `free`.
//! - `mi_free_small` and `mi_free_small_nonnull` only accept blocks from `mi_*alloc_small`.
//! - The size outputs of `mi_u*` are usable byte counts, including those of `mi_urealloc`,
//!   `mi_urealloc_aligned`, and `mi_ufree`. For internal block accounting, use
//!   `mi_allocated_size` while the allocation is live. Padding, alignment and guarded
//!   sampling can make the two sizes differ.
//! - A `mi_heap_t` can be used and its blocks freed from any thread. `mi_heap_delete`
//!   leaves live blocks available to free. Other threads must stop using the heap before
//!   deletion, or call `mi_thread_heap_release` if they still need to free blocks concurrently.
//!   After releasing a heap, a thread can no longer allocate in it. `mi_heap_destroy` requires
//!   exclusive access and frees all live blocks, leaving outstanding pointers dangling.
//!   Pass `mi_heap_main()` when targeting the main heap, since public heap APIs no longer
//!   interpret a null heap as the main heap.
//! - Keep other threads from allocating in a visited heap or freeing its blocks during
//!   `mi_heap_visit_blocks` or `mi_heap_visit_abandoned_blocks`.
//! - A `mi_theap_t` can only allocate on the thread that created it.
//! - Callbacks and their `arg` registered with `mi_register_deferred_free`,
//!   `mi_register_output` and `mi_register_error` are process-global and run on whichever
//!   thread triggers them, so `arg` must be safe to use from every thread. A callback always
//!   receives its registered `arg`. Re-registration does not wait for readers to finish, so
//!   the previous callback and argument must remain valid while any thread can still use them.
//! - The deferred free callback runs from allocation and collection, and is not re-entered
//!   on the same thread.
//! - A registered error callback runs after the diagnostic and replaces the default error
//!   handler. An allocator operation sets `errno` on its own failure path when required by
//!   its contract. Secure builds still abort on corrupted metadata (`EFAULT`) if the callback
//!   returns. Unrecoverable allocator errors abort in every build.
//! - Unwinding out of an `extern "C"` callback aborts the process, so a panic in one is
//!   fatal.
//! - Option setters synchronize with concurrent readers. Options consumed at process or
//!   heap initialization must be set before that initialization to affect it. Set
//!   `MIMALLOC_*` variables before process start. Secure builds only accept `show_stats`,
//!   `verbose`, `show_errors` and `allow_thp` from the environment. Elevated processes
//!   ignore all `MIMALLOC_*` options.
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

/// Whether the library was built with mimalloc's internal checks (`MI_DEBUG`), which the
/// `debug` feature enables. Features are unified across the dependency graph, so dependents
/// should read this instead of their own `debug` feature.
pub const MI_DEBUG: bool = cfg!(mi_debug);

/// Whether the library was built with `MI_GUARDED`, which the `debug` feature enables.
///
/// Features are unified across the dependency graph, so dependents should read this instead
/// of their own `debug` feature.
pub const MI_GUARDED: bool = cfg!(mi_guarded);

/// The alignment `mi_malloc` gives every block of at least this many bytes.
///
/// This is the default `MI_MAX_ALIGN_SIZE` from `config.h` (16), or a conservative bound
/// of 1 with [`MI_GUARDED`]. Since v3.7.0, guarded sampling rounds up object sizes by
/// default. Precise placement requires the C build macro `MI_UNSAFE_GUARDED_PRECISE`,
/// which this crate does not enable.
pub const MI_MALLOC_ALIGN: usize = if MI_GUARDED { 1 } else { 16 };

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
