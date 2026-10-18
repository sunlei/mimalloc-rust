//! Upstream's `test/test-api-fill.c`: zero-initialization of the `*zalloc`, `calloc` and
//! `rezalloc` families and, with `MI_DEBUG`, the debug fill of fresh and freed blocks. The
//! exit status counts failures.

use std::ffi::c_int;
use std::process::ExitCode;

use mimalloc_rust_sys as _;

unsafe extern "C" {
    fn mi_test_api_fill_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments and only calls the mimalloc API.
    let failed = unsafe { mi_test_api_fill_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
