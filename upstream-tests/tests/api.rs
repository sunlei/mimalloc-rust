//! Upstream's `test/test-api.c`: API behavior checks; the exit status counts failures.

use mimalloc_rust_sys as _;
use std::ffi::c_int;
use std::process::ExitCode;

unsafe extern "C" {
    fn mi_test_api_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments and only calls the mimalloc API.
    let failed = unsafe { mi_test_api_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
