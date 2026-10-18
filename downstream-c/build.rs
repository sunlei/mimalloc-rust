//! Builds `src/dep.c` into a shared library, like a C library that a Rust program loads.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/dep.c");
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    assert!(
        !target_features
            .split(',')
            .any(|feature| feature == "crt-static"),
        "a statically linked executable cannot load the shared library under test"
    );
    let include_dir = env::var("DEP_MIMALLOC_INCLUDE_DIR").expect("set by mimalloc-rust-sys");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("set by Cargo");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("set by Cargo"));
    let extension = if target_os == "macos" { "dylib" } else { "so" };
    let library = out_dir.join(format!("libdep.{extension}"));

    let status = cc::Build::new()
        .get_compiler()
        .to_command()
        .args(["-shared", "-fPIC"])
        .arg(format!("-I{include_dir}"))
        .arg("src/dep.c")
        .arg("-o")
        .arg(&library)
        .status()
        .expect("running the C compiler");
    assert!(status.success(), "compiling src/dep.c failed");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=dylib=dep");
}
