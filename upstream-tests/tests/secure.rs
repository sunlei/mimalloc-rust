//! Upstream's `test/test-secure.c`: protection of process metadata and guarded pages.

use mimalloc_rust_sys as _;
use std::ffi::c_int;
use std::process::ExitCode;

unsafe extern "C" {
    fn mi_test_secure_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments and checks the linked allocator's protections.
    let failed = unsafe { mi_test_secure_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
