//! Builds `src/hog.c` into a shared library that takes static TLS, for `tests/dlopen.rs`.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/hog.c");
    // The test only runs on glibc; elsewhere there is nothing to build.
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("set by Cargo");
    let target_env = env::var("CARGO_CFG_TARGET_ENV").expect("set by Cargo");
    if (target_os.as_str(), target_env.as_str()) != ("linux", "gnu") {
        return;
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("set by Cargo"));

    // `get_compiler` already adds `-fPIC` on these targets. `-shared` is passed by hand:
    // cc deprecates `shared_flag` because it only builds static libraries.
    let status = cc::Build::new()
        .get_compiler()
        .to_command()
        .arg("-shared")
        .arg("src/hog.c")
        .arg("-o")
        .arg(out_dir.join("libhog.so"))
        .status()
        .expect("running the C compiler");
    assert!(status.success(), "compiling src/hog.c failed");
}
