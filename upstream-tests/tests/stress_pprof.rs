//! Upstream's `test/test-stress-pprof.c`: the stress test with the pprof heap profiler
//! attached, taking a snapshot after each iteration. Upstream only runs it on macOS.

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    use std::ffi::{c_char, c_int};
    use std::path::Path;
    use std::{env, fs};

    use mimalloc_rust_sys as _;

    unsafe extern "C" {
        fn mi_test_stress_pprof_main(argc: c_int, argv: *const *const c_char) -> c_int;
    }

    // The test writes its snapshots to `profile/` in the working directory, which upstream's
    // CMake creates in its build directory.
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stress-pprof");
    fs::create_dir_all(dir.join("profile")).unwrap();
    env::set_current_dir(&dir).unwrap();
    // Threads, load per thread in percent, iterations.
    common::run(
        mi_test_stress_pprof_main,
        &[c"test-stress-pprof", c"32", c"50", c"50"],
    )
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("skipped: upstream only runs it on macOS");
}
