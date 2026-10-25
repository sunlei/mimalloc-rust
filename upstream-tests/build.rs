//! Compiles upstream's C tests with the configuration of the library under test.
//!
//! Each test's `main` is renamed so that a Rust test target can call it and link the
//! library through `mimalloc-rust-sys`, exactly as an application would.

use std::env;
use std::path::{Path, PathBuf};

// The tests upstream's CMakeLists.txt registers for a static build on our targets, except
// the target-dependent ones in `main`, `qsbr` (see below), and `test-rust`, which tests
// upstream's own Rust crate.
const TESTS: [&str; 9] = [
    "api",
    "api-fill",
    "arena",
    "bitmap",
    "fork",
    "profile",
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
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("set by Cargo");
    let target_env = env::var("CARGO_CFG_TARGET_ENV").expect("set by Cargo");
    let conditional = [
        // `pprof` checks the backtraces of its samples, which the library only records
        // with `execinfo.h` (see mimalloc-rust-sys/build.rs), so musl fails it.
        ("pprof", target_env != "musl"),
        // Upstream only registers it on macOS.
        ("stress-pprof", target_os == "macos"),
    ];
    let selected = conditional
        .into_iter()
        .filter_map(|(test, enabled)| enabled.then_some(test));
    for test in TESTS.into_iter().chain(selected) {
        build(test, &test_dir, &include_dir, &defines)
            .compile(&format!("upstream_test_{}", test.replace('-', "_")));
    }

    // `test-qsbr.c` includes the allocator's sources itself, so its archive defines every
    // `mi_*` symbol. Only its own test links it (`#[link]` in tests/qsbr.rs): on the other
    // tests' link lines it would come before the library and capture their `mi_*` calls.
    build("qsbr", &test_dir, &include_dir, &defines)
        .cargo_metadata(false)
        .compile("upstream_test_qsbr");
    println!(
        "cargo:rustc-link-search=native={}",
        env::var("OUT_DIR").expect("set by Cargo")
    );
}

fn build(test: &str, test_dir: &Path, include_dir: &Path, defines: &str) -> cc::Build {
    let mut build = cc::Build::new();
    build
        .file(test_dir.join(format!("test-{test}.c")))
        .include(include_dir)
        .define(
            "main",
            format!("mi_test_{}_main", test.replace('-', "_")).as_str(),
        );
    for define in defines.split_whitespace() {
        let (name, value) = define.split_once('=').expect("NAME=VALUE");
        // `NDEBUG` would turn the tests' `assert` checks into no-ops. The library keeps
        // it, and the headers follow the explicit `MI_DEBUG` and `MI_GUARDED` either way.
        if name == "NDEBUG" {
            continue;
        }
        build.define(name, value);
    }
    build
}
