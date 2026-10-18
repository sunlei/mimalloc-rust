//! Calls a renamed upstream stress `main`, which takes `[THREADS] [SCALE] [ITER]`.
//!
//! The callers pass upstream's release defaults for these explicitly, because the tests pick
//! lighter ones whenever `MI_GUARDED` is defined, and the build script always defines it.
//! Without `NDEBUG` (see build.rs) the tests otherwise run their debug variant: a minimal
//! arena reserve and periodic arena dumps.

use std::ffi::{CStr, c_char, c_int};
use std::process::ExitCode;
use std::ptr;

pub(crate) type StressMain = unsafe extern "C" fn(c_int, *const *const c_char) -> c_int;

/// Runs `main` with `args` as its command line, the first being the program name.
pub(crate) fn run(main: StressMain, args: &[&CStr]) -> ExitCode {
    let argv: Vec<*const c_char> = args
        .iter()
        .map(|arg| arg.as_ptr())
        .chain([ptr::null()])
        .collect();
    let argc = c_int::try_from(args.len()).unwrap();
    // SAFETY: as for a C `main`, `argv` holds `argc` C strings followed by a null pointer.
    let status = unsafe { main(argc, argv.as_ptr()) };
    if status == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
