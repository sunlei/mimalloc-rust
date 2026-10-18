//! Compiles upstream's C tests with the configuration of the library under test.
//!
//! Each test's `main` is renamed so that a Rust test target can call it and link the
//! library through `mimalloc-rust-sys`, exactly as an application would.

use std::env;
use std::path::PathBuf;

const TESTS: [&str; 2] = ["api", "stress"];

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
        let mut build = cc::Build::new();
        build
            .file(test_dir.join(format!("test-{test}.c")))
            .include(&include_dir)
            .define("main", format!("mi_test_{test}_main").as_str());
        for define in defines.split_whitespace() {
            let (name, value) = define.split_once('=').expect("NAME=VALUE");
            build.define(name, value);
        }
        build.compile(&format!("upstream_test_{test}"));
    }
}
