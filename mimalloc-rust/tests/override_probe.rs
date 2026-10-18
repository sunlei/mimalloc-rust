//! Linking the crate alone must install mimalloc: this binary calls nothing in it and
//! sets no `#[global_allocator]`.
#![cfg(feature = "override")]

use mimalloc_rust as _;

use std::env;
use std::process::Command;

// Only the release profile is conclusive: dev builds share generic instances across
// crates, which can pull mimalloc into the link even without `+whole-archive`.
#[test]
fn linking_the_crate_installs_mimalloc() {
    // `--list` makes the child exit without running tests; mimalloc prints its
    // statistics to stderr at process exit.
    let output = Command::new(env::current_exe().expect("test binary path"))
        .arg("--list")
        .env("MIMALLOC_SHOW_STATS", "1")
        .output()
        .expect("the test binary runs");
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("  elapsed     :"), "{stderr}");
}
