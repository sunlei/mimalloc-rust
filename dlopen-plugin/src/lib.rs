//! A plugin that allocates through `MiMalloc`, loaded with `dlopen` by `tests/dlopen.rs`.
//!
//! It enables `local_dynamic_tls`, so it stays out of the workspace's default members:
//! building it together with them would unify the feature into their tests.

use std::thread;

use mimalloc_rust::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Allocates on the calling thread and on a new one, and returns the bytes allocated.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_run() -> usize {
    let local = vec![1_u8; 1000];
    let remote = thread::spawn(|| vec![2_u8; 100])
        .join()
        .expect("allocating thread");
    local.len() + remote.len()
}
