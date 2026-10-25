//! Upstream's `test/test-pprof.c`: the pprof heap profiler writes text and protobuf
//! snapshots, also from several threads and, on Linux, while allocation fails; the exit
//! status counts failures.
//!
//! Skipped on musl, where the library records no backtraces (see build.rs).

#[cfg(not(target_env = "musl"))]
fn main() -> std::process::ExitCode {
    use std::ffi::c_int;
    use std::path::Path;
    use std::process::ExitCode;
    use std::{env, fs};

    use mimalloc_rust_sys as _;

    unsafe extern "C" {
        fn mi_test_pprof_main() -> c_int;
    }

    // The test writes its snapshots to `profile/` in the working directory, which upstream's
    // CMake creates in its build directory.
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pprof");
    fs::create_dir_all(dir.join("profile")).unwrap();
    env::set_current_dir(&dir).unwrap();
    // SAFETY: the C test takes no arguments and only calls the mimalloc API and libc.
    let failed = unsafe { mi_test_pprof_main() };
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(target_env = "musl")]
fn main() {
    println!("skipped: no backtraces on musl");
}
