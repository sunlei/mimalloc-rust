//! Upstream's `test/test-stress.c`: threads allocate, transfer and free objects across
//! threads, and the test aborts on memory corruption.

mod common;

use std::ffi::{c_char, c_int};
use std::process::ExitCode;

use mimalloc_rust_sys as _;

unsafe extern "C" {
    fn mi_test_stress_main(argc: c_int, argv: *const *const c_char) -> c_int;
}

fn main() -> ExitCode {
    // Threads, load per thread in percent, iterations.
    common::run(mi_test_stress_main, &[c"test-stress", c"32", c"50", c"50"])
}
