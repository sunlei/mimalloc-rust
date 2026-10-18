//! Contract tests for `MiMalloc`, which is also this test binary's global allocator.

use std::alloc::{GlobalAlloc, Layout};
use std::{array, ptr, slice, thread};

use mimalloc_rust::MiMalloc;
use mimalloc_rust_sys::{MI_MALLOC_VERSION, MI_STAT_VERSION, mi_any_heap_contains};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn is_mimalloc_block<T: ?Sized>(value: &T) -> bool {
    // SAFETY: `mi_any_heap_contains` only inspects the address.
    unsafe { mi_any_heap_contains(ptr::from_ref(value).cast()) }
}

fn pattern(index: usize) -> u8 {
    (index * 31 % 251) as u8
}

fn layout(size: usize, align: usize) -> Layout {
    Layout::from_size_align(size, align).expect("valid layout")
}

#[test]
fn version_matches_vendored_sources() {
    assert_eq!(mimalloc_rust::version(), MI_MALLOC_VERSION);
}

#[test]
fn alloc_honors_size_and_alignment() {
    // Covers size < align, the default alignment, page alignment and a 1 MiB alignment.
    for layout in [
        layout(1, 16),
        layout(24, 8),
        layout(8, 4096),
        layout(1 << 20, 1 << 20),
    ] {
        let block = unsafe { MiMalloc.alloc(layout) };
        assert!(!block.is_null(), "{layout:?}");
        assert_eq!(block.addr() % layout.align(), 0, "{layout:?}");
        assert!(
            unsafe { mimalloc_rust::usable_size(block) } >= layout.size(),
            "{layout:?}"
        );
        unsafe { block.write_bytes(0xA5, layout.size()) };
        unsafe { MiMalloc.dealloc(block, layout) };
    }
}

#[test]
fn alloc_zeroed_clears_the_requested_range() {
    for layout in [layout(64, 8), layout(4096 + 7, 64), layout(1 << 20, 4096)] {
        // Dirty a block first so a reused block cannot pass by accident.
        let dirty = unsafe { MiMalloc.alloc(layout) };
        unsafe { dirty.write_bytes(0xFF, layout.size()) };
        unsafe { MiMalloc.dealloc(dirty, layout) };

        let block = unsafe { MiMalloc.alloc_zeroed(layout) };
        assert!(!block.is_null(), "{layout:?}");
        assert_eq!(block.addr() % layout.align(), 0, "{layout:?}");
        let bytes = unsafe { slice::from_raw_parts(block, layout.size()) };
        assert!(bytes.iter().all(|&b| b == 0));
        unsafe { MiMalloc.dealloc(block, layout) };
    }
}

#[test]
fn realloc_preserves_contents_when_growing_and_shrinking() {
    const SMALL: usize = 256;
    const LARGE: usize = 64 * 1024;
    const SHRUNK: usize = 100;
    let expected: [u8; SMALL] = array::from_fn(pattern);
    for align in [8, 64, 4096] {
        let block = unsafe { MiMalloc.alloc(layout(SMALL, align)) };
        assert!(!block.is_null());
        unsafe { block.copy_from_nonoverlapping(expected.as_ptr(), SMALL) };

        let grown = unsafe { MiMalloc.realloc(block, layout(SMALL, align), LARGE) };
        assert!(!grown.is_null());
        assert_eq!(grown.addr() % align, 0);
        assert_eq!(unsafe { slice::from_raw_parts(grown, SMALL) }, expected);

        let shrunk = unsafe { MiMalloc.realloc(grown, layout(LARGE, align), SHRUNK) };
        assert!(!shrunk.is_null());
        assert_eq!(shrunk.addr() % align, 0);
        assert_eq!(
            unsafe { slice::from_raw_parts(shrunk, SHRUNK) },
            &expected[..SHRUNK]
        );

        unsafe { MiMalloc.dealloc(shrunk, layout(SHRUNK, align)) };
    }
}

fn filled_blocks() -> Vec<Box<[u8]>> {
    (0..1000)
        .map(|i| (0..16 + i % 2048).map(pattern).collect())
        .collect()
}

fn assert_filled(blocks: &[Box<[u8]>]) {
    for block in blocks {
        assert!(block.iter().enumerate().all(|(i, &b)| b == pattern(i)));
    }
}

#[test]
fn blocks_outlive_the_thread_that_allocated_them() {
    let blocks = thread::spawn(filled_blocks)
        .join()
        .expect("allocating thread");
    // The allocating thread has exited; its blocks are now freed from this thread.
    assert_filled(&blocks);
    drop(blocks);
}

#[test]
fn blocks_can_be_freed_by_another_thread() {
    let blocks = filled_blocks();
    thread::spawn(move || {
        assert_filled(&blocks);
        drop(blocks);
    })
    .join()
    .expect("freeing thread");
}

#[test]
fn std_collections_allocate_from_mimalloc() {
    let boxed = Box::new(42_u64);
    let vec = vec![0_u8; 1 << 16];
    let string = String::from("mimalloc");
    assert!(is_mimalloc_block(&*boxed));
    assert!(is_mimalloc_block(vec.as_slice()));
    assert!(is_mimalloc_block(string.as_str()));
}

#[test]
fn stats_json_reports_the_linked_versions() {
    let json = mimalloc_rust::stats_json().expect("mimalloc produced statistics");
    let text = json.as_ref().to_str().expect("statistics are UTF-8");
    let stats: serde_json::Value = serde_json::from_str(text).expect("statistics are valid JSON");
    assert_eq!(stats["stat_version"], MI_STAT_VERSION);
    assert_eq!(stats["mimalloc_version"], MI_MALLOC_VERSION);
}
