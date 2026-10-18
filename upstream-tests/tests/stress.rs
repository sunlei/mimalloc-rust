//! Upstream's `test/test-stress.c`: threads allocate, transfer and free objects across
//! threads, and the test aborts on memory corruption.

use mimalloc_rust_sys as _;
use std::ffi::{CStr, c_char, c_int};
use std::process::ExitCode;
use std::ptr;

unsafe extern "C" {
    fn mi_test_stress_main(argc: c_int, argv: *const *const c_char) -> c_int;
}

// Upstream's release defaults (threads, load per thread in percent, iterations). Passed
// explicitly because the test picks lighter ones whenever `MI_GUARDED` is defined, and
// the build script always defines it. Without `NDEBUG` (see build.rs) the test otherwise
// runs its debug variant: a minimal arena reserve and periodic arena dumps.
const ARGS: [&CStr; 4] = [c"test-stress", c"32", c"50", c"50"];

fn main() -> ExitCode {
    let argv: Vec<*const c_char> = ARGS
        .iter()
        .map(|arg| arg.as_ptr())
        .chain([ptr::null()])
        .collect();
    let argc = c_int::try_from(ARGS.len()).unwrap();
    // SAFETY: as for a C `main`, `argv` holds `argc` C strings followed by a null pointer.
    let status = unsafe { mi_test_stress_main(argc, argv.as_ptr()) };
    if status == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
