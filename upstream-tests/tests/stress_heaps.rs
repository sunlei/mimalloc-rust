//! Upstream's `test/test-stress-heaps.c`: the stress test allocating from rotating
//! `mi_heap_t` heaps, each deleted a few iterations later, and allowing large objects.

mod common;

use std::ffi::{c_char, c_int};
use std::process::ExitCode;

use mimalloc_rust_sys::{
    mi_option_arena_eager_commit, mi_option_page_commit_on_demand, mi_option_set,
};

unsafe extern "C" {
    fn mi_test_stress_heaps_main(argc: c_int, argv: *const *const c_char) -> c_int;
}

fn main() -> ExitCode {
    // Upstream runs this test with `MIMALLOC_ARENA_EAGER_COMMIT=0` and
    // `MIMALLOC_PAGE_COMMIT_ON_DEMAND=0`. mimalloc reads both when it reserves an arena or
    // allocates a page, so setting them before the test starts has the same effect.
    // SAFETY: no other thread exists yet.
    unsafe { mi_option_set(mi_option_arena_eager_commit, 0) };
    // SAFETY: as above.
    unsafe { mi_option_set(mi_option_page_commit_on_demand, 0) };
    // Threads, load per thread in percent, iterations.
    common::run(
        mi_test_stress_heaps_main,
        &[c"test-stress-heaps", c"32", c"50", c"50"],
    )
}
