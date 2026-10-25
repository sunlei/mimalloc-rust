//! Upstream's `test/test-fork.c`: forks while other threads allocate, start and exit; each
//! child must neither deadlock nor fail to free blocks of threads it does not have.

use std::ffi::c_int;
use std::process::ExitCode;

use mimalloc_rust_sys as _;

unsafe extern "C" {
    fn mi_test_fork_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments, only calls the mimalloc API and libc, and its
    // children leave with `_exit`, so they never return into Rust.
    let failed = unsafe { mi_test_fork_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
