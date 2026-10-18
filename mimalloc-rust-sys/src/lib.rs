//! Raw FFI bindings to mimalloc, compiled from the vendored sources in `c_src/mimalloc`.
//!
//! The bindings cover the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.
//! Macros and `static inline` helpers (such as `mi_stats_init` and the `*_csize`
//! variants) are not bound; see `UPSTREAM.md`.

#![no_std]
