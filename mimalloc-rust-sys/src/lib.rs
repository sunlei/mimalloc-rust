//! Raw FFI bindings to mimalloc, compiled from the vendored sources in `c_src/mimalloc`.
//!
//! The bindings cover the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.
//! Macros and `static inline` helpers (such as `mi_stats_init` and the `*_csize`
//! variants) are not bound; see `UPSTREAM.md`.

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
