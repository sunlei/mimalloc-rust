//! Contract tests for `MiMalloc`, which is also this test binary's global allocator.

use std::alloc::{GlobalAlloc, Layout};
use std::collections::HashSet;
use std::{array, ptr, slice, thread};

use mimalloc_rust::MiMalloc;
use mimalloc_rust_sys::{MI_MALLOC_VERSION, MI_STAT_VERSION, mi_any_heap_contains};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn is_mimalloc_block<T: ?Sized>(value: &T) -> bool {
    // SAFETY: `mi_any_heap_contains` only inspects the address.
    unsafe { mi_any_heap_contains(ptr::from_ref(value).cast()) }
}

// Sizes around the 8/16-byte boundaries and the size classes that are not powers of two,
// where `MiMalloc` switches between the `mi_malloc` and the aligned functions.
const SIZES: [usize; 35] = [
    1, 2, 3, 4, 5, 7, 8, 9, 12, 15, 16, 17, 23, 24, 25, 31, 32, 33, 40, 48, 63, 64, 65, 96, 160,
    224, 320, 640, 1000, 1024, 1025, 2240, 4096, 4640, 70_000,
];
const ALIGNS: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 4096];

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
fn every_size_and_alignment_is_honored_by_alloc_and_alloc_zeroed() {
    for align in ALIGNS {
        for size in SIZES {
            let layout = layout(size, align);
            let block = unsafe { MiMalloc.alloc(layout) };
            assert!(!block.is_null(), "{layout:?}");
            assert_eq!(block.addr() % align, 0, "{layout:?}");
            unsafe { block.write_bytes(0xA5, size) };
            unsafe { MiMalloc.dealloc(block, layout) };

            let block = unsafe { MiMalloc.alloc_zeroed(layout) };
            assert!(!block.is_null(), "{layout:?}");
            assert_eq!(block.addr() % align, 0, "{layout:?}");
            let bytes = unsafe { slice::from_raw_parts(block, size) };
            assert!(bytes.iter().all(|&b| b == 0), "{layout:?}");
            unsafe { MiMalloc.dealloc(block, layout) };
        }
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

// mimalloc puts a freed block on its page's `local_free` list, so a single free followed by
// an allocation usually returns a different block. Freeing many dirty blocks first makes the
// zeroed allocations reuse some of them, which the test asserts.
#[test]
fn alloc_zeroed_clears_reused_dirty_blocks() {
    const LIVE: usize = 4096;
    for size in [8, 24, 224, 640, 4640] {
        let layout = layout(size, 8);
        let dirty: Vec<*mut u8> = (0..LIVE)
            .map(|_| unsafe { MiMalloc.alloc(layout) })
            .collect();
        for &block in &dirty {
            assert!(!block.is_null(), "{layout:?}");
            unsafe { block.write_bytes(0xFF, size) };
        }
        for &block in &dirty {
            unsafe { MiMalloc.dealloc(block, layout) };
        }

        let zeroed: Vec<*mut u8> = (0..LIVE)
            .map(|_| unsafe { MiMalloc.alloc_zeroed(layout) })
            .collect();
        for &block in &zeroed {
            assert!(!block.is_null(), "{layout:?}");
            let bytes = unsafe { slice::from_raw_parts(block, size) };
            assert!(bytes.iter().all(|&b| b == 0), "{layout:?}");
        }
        let dirty: HashSet<*mut u8> = dirty.into_iter().collect();
        assert!(
            zeroed.iter().any(|block| dirty.contains(block)),
            "no dirty block was reused for {layout:?}"
        );
        for &block in &zeroed {
            unsafe { MiMalloc.dealloc(block, layout) };
        }
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

// Layouts whose alignment `mi_malloc` already provides take the `mi_malloc` path, which
// relies on every block of a size class being aligned, not just the first one of a page.
#[test]
fn every_live_block_is_aligned() {
    const LIVE: usize = 512;
    for size in (1_usize..=1100).chain([2240, 4096, 4640, 70_000]) {
        let align = (1 << size.ilog2()).min(16);
        let layout = layout(size, align);
        let blocks: Vec<*mut u8> = (0..LIVE)
            .map(|_| unsafe { MiMalloc.alloc(layout) })
            .collect();
        for &block in &blocks {
            assert!(!block.is_null(), "{layout:?}");
            assert_eq!(block.addr() % align, 0, "{layout:?}");
        }
        for &block in &blocks {
            unsafe { MiMalloc.dealloc(block, layout) };
        }
    }
}

// An alignment above the size is met by over-allocation, which can place a small request in
// a medium or singleton page (upstream issue #1400), so freeing must not assume such a
// block lives in a small page. Many live blocks make that placement likely.
#[test]
fn over_aligned_small_blocks_can_be_freed() {
    const LIVE: usize = 200;
    for align in [1024, 16 * 1024, 128 * 1024] {
        let layout = layout(8, align);
        let blocks: Vec<*mut u8> = (0..LIVE)
            .map(|_| unsafe { MiMalloc.alloc(layout) })
            .collect();
        for &block in &blocks {
            assert!(!block.is_null(), "{layout:?}");
            assert_eq!(block.addr() % align, 0, "{layout:?}");
            unsafe { block.write_bytes(0xA5, layout.size()) };
        }
        for &block in &blocks {
            unsafe { MiMalloc.dealloc(block, layout) };
        }
    }
}

// Crosses the boundary between `mi_realloc` and `mi_realloc_aligned` in both directions.
#[test]
fn realloc_keeps_alignment_and_contents_across_entry_points() {
    let expected: Vec<u8> = (0..70_000).map(pattern).collect();
    for align in ALIGNS {
        for from in SIZES {
            for to in SIZES {
                let block = unsafe { MiMalloc.alloc(layout(from, align)) };
                assert!(!block.is_null());
                unsafe { block.copy_from_nonoverlapping(expected.as_ptr(), from) };

                let moved = unsafe { MiMalloc.realloc(block, layout(from, align), to) };
                assert!(!moved.is_null(), "{from} -> {to}, align {align}");
                assert_eq!(moved.addr() % align, 0, "{from} -> {to}, align {align}");
                let kept = from.min(to);
                assert_eq!(
                    unsafe { slice::from_raw_parts(moved, kept) },
                    &expected[..kept],
                    "{from} -> {to}, align {align}"
                );
                unsafe { MiMalloc.dealloc(moved, layout(to, align)) };
            }
        }
    }
}

// The aligned functions would over-allocate 48 bytes into a 64-byte block
// (see docs/known-issues.md). With `MI_GUARDED`, `MiMalloc` keeps using them.
#[test]
fn small_layouts_get_a_block_of_their_own_size_class() {
    if mimalloc_rust_sys::MI_GUARDED {
        return;
    }
    let layout = layout(48, 8);
    let block = unsafe { MiMalloc.alloc(layout) };
    assert!(!block.is_null());
    assert_eq!(unsafe { mimalloc_rust::usable_size(block) }, 48);
    unsafe { MiMalloc.dealloc(block, layout) };
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
