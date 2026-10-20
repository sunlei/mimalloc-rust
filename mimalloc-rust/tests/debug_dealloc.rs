//! With `MI_DEBUG`, `MiMalloc::dealloc` hands the layout to mimalloc, which reports a size
//! larger than the block. This is a binary of its own because the error callback is
//! process-wide, and it sets no `#[global_allocator]` so that only these calls can trigger it.

use std::alloc::{GlobalAlloc, Layout};
use std::env;
use std::ffi::{c_int, c_void};
use std::io;
use std::process::Command;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use mimalloc_rust::MiMalloc;
use mimalloc_rust_sys::{MI_DEBUG, mi_register_error};

const CHILD_MARKER: &str = "MIMALLOC_RUST_DEBUG_DEALLOC_CHILD";
const TEST_NAME: &str = "dealloc_reports_a_layout_larger_than_the_block_only_with_mi_debug";

static INVALID_ARGUMENT_ERRORS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn count_error(err: c_int, _arg: *mut c_void) {
    // mimalloc reports a size larger than the block as `EINVAL`.
    if io::Error::from_raw_os_error(err).kind() == io::ErrorKind::InvalidInput {
        INVALID_ARGUMENT_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn dealloc_reports_a_layout_larger_than_the_block_only_with_mi_debug() {
    // Guarded blocks skip the size check, and mimalloc reads the sample rate when a heap is
    // initialized, so the body runs in a child that starts with sampling off.
    if env::var_os(CHILD_MARKER).is_none() {
        let output = Command::new(env::current_exe().expect("test binary path"))
            .args(["--exact", TEST_NAME])
            .env(CHILD_MARKER, "1")
            .env("MIMALLOC_GUARDED_SAMPLE_RATE", "0")
            .output()
            .expect("the test binary runs");
        assert!(output.status.success(), "{output:?}");
        // A test name that no longer matches would run nothing and still succeed.
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("1 passed"), "{stdout}");
        return;
    }

    // SAFETY: registered once, before this test allocates; `count_error` ignores `arg` and
    // can run on any thread.
    unsafe { mi_register_error(Some(count_error), ptr::null_mut()) };
    let layout = Layout::from_size_align(16, 8).expect("valid layout");

    // SAFETY: `layout` has a non-zero size.
    let block = unsafe { MiMalloc.alloc(layout) };
    assert!(!block.is_null());
    // SAFETY: `block` is live and was allocated with `layout`.
    unsafe { MiMalloc.dealloc(block, layout) };
    assert_eq!(INVALID_ARGUMENT_ERRORS.load(Ordering::Relaxed), 0);

    // SAFETY: `layout` has a non-zero size.
    let block = unsafe { MiMalloc.alloc(layout) };
    assert!(!block.is_null());
    let too_large = Layout::from_size_align(4096, 8).expect("valid layout");
    // SAFETY: not met on purpose: `block` was allocated with `layout`, not `too_large`. With
    // `MI_DEBUG`, mimalloc reports the size and then frees the block as `mi_free` would;
    // without it, `mi_free` ignores the layout.
    unsafe { MiMalloc.dealloc(block, too_large) };
    assert_eq!(
        INVALID_ARGUMENT_ERRORS.load(Ordering::Relaxed),
        usize::from(MI_DEBUG)
    );
}
