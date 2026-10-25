//! Upstream's `test/test-qsbr.c`: delayed page reuse for runtimes with quiescent-state-based
//! reclamation (`MI_CPYTHON`), such as CPython; the exit status counts failures.
//!
//! The C test compiles its own copy of the allocator with that layout instead of using the
//! library, so this target does not link `mimalloc-rust-sys` (see build.rs).

use std::ffi::c_int;
use std::process::ExitCode;

#[link(name = "upstream_test_qsbr", kind = "static")]
unsafe extern "C" {
    fn mi_test_qsbr_main() -> c_int;
}

fn main() -> ExitCode {
    // SAFETY: the C test takes no arguments and only calls its own allocator and libc.
    let failed = unsafe { mi_test_qsbr_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
