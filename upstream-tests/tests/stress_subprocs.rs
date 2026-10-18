//! Upstream's `test/test-stress-subprocs.c`: the stress test running in two mimalloc
//! sub-processes (`mi_subproc_*`) at once.

mod common;

use std::ffi::{c_char, c_int};
use std::process::ExitCode;

use mimalloc_rust_sys as _;

unsafe extern "C" {
    fn mi_test_stress_subprocs_main(argc: c_int, argv: *const *const c_char) -> c_int;
}

fn main() -> ExitCode {
    // Threads per sub-process (this test's `NTHREADS`), load per thread in percent, iterations.
    common::run(
        mi_test_stress_subprocs_main,
        &[c"test-stress-subprocs", c"16", c"50", c"50"],
    )
}
