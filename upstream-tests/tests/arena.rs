//! Upstream's `test/test-arena.c`: theap cleanup, `mi_thread_id_of`, reclaim on free and
//! arena iteration, partly through the library's internal functions; the exit status counts
//! failures.

use std::ffi::c_int;
use std::process::ExitCode;

use mimalloc_rust_sys as _;

unsafe extern "C" {
    fn mi_test_arena_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments and only calls the mimalloc API.
    let failed = unsafe { mi_test_arena_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
