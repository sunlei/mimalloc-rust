//! Raw FFI bindings to mimalloc, compiled from the vendored sources in `c_src/mimalloc`.
//!
//! The bindings cover the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.
//! Macros and `static inline` helpers (such as `mi_stats_init` and the `*_csize`
//! variants) are not bound; see `UPSTREAM.md`.

#![no_std]
#![allow(non_camel_case_types, non_upper_case_globals)]

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
