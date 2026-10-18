//! Compiles upstream's C tests with the configuration of the library under test.
//!
//! Each test's `main` is renamed so that a Rust test target can call it and link the
//! library through `mimalloc-rust-sys`, exactly as an application would.

use std::env;
use std::path::PathBuf;

// The tests upstream's CMakeLists.txt registers, except `profile`, which it only runs with
// `MI_PROFILE` builds.
const TESTS: [&str; 5] = [
    "api",
    "api-fill",
    "stress",
    "stress-heaps",
    "stress-subprocs",
];

fn main() {
    let include_dir =
        PathBuf::from(env::var_os("DEP_MIMALLOC_INCLUDE_DIR").expect("set by mimalloc-rust-sys"));
    let test_dir = include_dir
        .parent()
        .expect("include/ has a parent")
        .join("test");
    // The directory also covers the shared `testhelper.h`.
    println!("cargo:rerun-if-changed={}", test_dir.display());
    let defines = env::var("DEP_MIMALLOC_DEFINES").expect("set by mimalloc-rust-sys");
    for test in TESTS {
        let id = test.replace('-', "_");
        let mut build = cc::Build::new();
        build
            .file(test_dir.join(format!("test-{test}.c")))
            .include(&include_dir)
            .define("main", format!("mi_test_{id}_main").as_str());
        for define in defines.split_whitespace() {
            let (name, value) = define.split_once('=').expect("NAME=VALUE");
            // `NDEBUG` would turn the tests' `assert` checks into no-ops. The library keeps
            // it, and the headers follow the explicit `MI_DEBUG` and `MI_GUARDED` either way.
            if name == "NDEBUG" {
                continue;
            }
            build.define(name, value);
        }
        build.compile(&format!("upstream_test_{id}"));
    }
}
