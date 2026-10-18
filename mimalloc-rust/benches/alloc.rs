//! Compares the entry points `MiMalloc` can use for common Rust layouts.
//!
//! Each iteration allocates `BATCH` blocks and then frees them on one warm thread, so the
//! results are steady-state fast-path costs, not cold-start or cross-thread costs.

use std::alloc::{GlobalAlloc, Layout};
use std::hint::black_box;
use std::ptr;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mimalloc_rust::MiMalloc;
use mimalloc_rust_sys::{mi_free, mi_malloc, mi_malloc_aligned};

const BATCH: usize = 64;
const SIZES: [usize; 6] = [8, 24, 64, 256, 1024, 4096];
// `mi_malloc` already guarantees this alignment, so it is the case where `MiMalloc` could
// skip the aligned entry point.
const ALIGN: usize = 8;

fn alloc_free_batch(
    layout: Layout,
    alloc: impl Fn(Layout) -> *mut u8,
    free: impl Fn(*mut u8, Layout),
) {
    let mut blocks = [ptr::null_mut(); BATCH];
    for block in &mut blocks {
        *block = alloc(black_box(layout));
    }
    for block in blocks {
        free(black_box(block), layout);
    }
}

fn assert_valid_block(
    layout: Layout,
    alloc: impl Fn(Layout) -> *mut u8,
    free: impl Fn(*mut u8, Layout),
) {
    let block = alloc(layout);
    assert!(!block.is_null(), "{layout:?}");
    assert_eq!(block.addr() % layout.align(), 0, "{layout:?}");
    free(block, layout);
}

fn bench_alloc_free(c: &mut Criterion) {
    let malloc = |layout: Layout| unsafe { mi_malloc(layout.size()) }.cast::<u8>();
    let malloc_aligned =
        |layout: Layout| unsafe { mi_malloc_aligned(layout.size(), layout.align()) }.cast::<u8>();
    let free = |block: *mut u8, _layout: Layout| unsafe { mi_free(block.cast()) };
    let global_alloc = |layout: Layout| unsafe { MiMalloc.alloc(layout) };
    let global_dealloc =
        |block: *mut u8, layout: Layout| unsafe { MiMalloc.dealloc(block, layout) };

    let mut group = c.benchmark_group("alloc_free_batch");
    group.throughput(Throughput::Elements(BATCH as u64));
    for size in SIZES {
        let layout = Layout::from_size_align(size, ALIGN).expect("valid layout");
        assert_valid_block(layout, malloc, free);
        assert_valid_block(layout, malloc_aligned, free);
        assert_valid_block(layout, global_alloc, global_dealloc);

        group.bench_with_input(
            BenchmarkId::new("mi_malloc", size),
            &layout,
            |b, &layout| {
                b.iter(|| alloc_free_batch(layout, malloc, free));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mi_malloc_aligned", size),
            &layout,
            |b, &layout| b.iter(|| alloc_free_batch(layout, malloc_aligned, free)),
        );
        group.bench_with_input(BenchmarkId::new("MiMalloc", size), &layout, |b, &layout| {
            b.iter(|| alloc_free_batch(layout, global_alloc, global_dealloc));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_alloc_free);
criterion_main!(benches);
