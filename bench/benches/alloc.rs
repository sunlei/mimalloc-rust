//! Compares the entry points `MiMalloc` can use for common Rust layouts.
//!
//! `alloc_free_batch` allocates `BATCH` blocks and then frees them on one warm thread, so its
//! results are steady-state fast-path costs, not cold-start or cross-thread costs.
//! `cross_thread_free` frees the blocks on another thread instead.

use std::alloc::{GlobalAlloc, Layout};
use std::hint::black_box;
use std::sync::mpsc;
use std::{mem, ptr, thread};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mimalloc_rust::MiMalloc;
use mimalloc_rust_sys::{mi_free, mi_free_size_aligned, mi_malloc, mi_malloc_aligned};

const BATCH: usize = 64;
// 48, 224 and 640 have size classes that are not powers of two, where the aligned entry
// point falls back to over-allocation (see docs/known-issues.md); the other sizes have
// power-of-two size classes.
const SIZES: [usize; 9] = [8, 24, 48, 64, 224, 256, 640, 1024, 4096];
// `mi_malloc` already guarantees this alignment, so `MiMalloc` takes its `mi_malloc` path
// unless built with `debug`. The `black_box`ed layout keeps `MiMalloc`'s alignment check from
// being folded at compile time, so it runs a few instructions more per allocation than
// `mi_malloc` here, unlike with the constant layouts of most callers.
const ALIGN: usize = 8;

fn malloc(layout: Layout) -> *mut u8 {
    unsafe { mi_malloc(layout.size()) }.cast()
}

fn free(block: *mut u8, _layout: Layout) {
    unsafe { mi_free(block.cast()) }
}

// Without `debug` or `secure`, up to `MI_SMALL_SIZE_MAX` (1 KiB on 64-bit targets) this finds
// the page by masking the pointer instead of loading it; otherwise it calls `mi_free`.
fn free_sized(block: *mut u8, layout: Layout) {
    unsafe { mi_free_size_aligned(block.cast(), layout.size(), layout.align()) }
}

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
    let malloc_aligned =
        |layout: Layout| unsafe { mi_malloc_aligned(layout.size(), layout.align()) }.cast::<u8>();
    let global_alloc = |layout: Layout| unsafe { MiMalloc.alloc(layout) };
    let global_dealloc =
        |block: *mut u8, layout: Layout| unsafe { MiMalloc.dealloc(block, layout) };

    let mut group = c.benchmark_group("alloc_free_batch");
    group.throughput(Throughput::Elements(BATCH as u64));
    for size in SIZES {
        let layout = Layout::from_size_align(size, ALIGN).expect("valid layout");
        assert_valid_block(layout, malloc, free);
        assert_valid_block(layout, malloc, free_sized);
        assert_valid_block(layout, malloc_aligned, free);
        assert_valid_block(layout, global_alloc, global_dealloc);

        group.bench_function(BenchmarkId::new("mi_malloc", size), |b| {
            b.iter(|| alloc_free_batch(layout, malloc, free));
        });
        group.bench_function(BenchmarkId::new("mi_free_size_aligned", size), |b| {
            b.iter(|| alloc_free_batch(layout, malloc, free_sized));
        });
        group.bench_function(BenchmarkId::new("mi_malloc_aligned", size), |b| {
            b.iter(|| alloc_free_batch(layout, malloc_aligned, free));
        });
        group.bench_function(BenchmarkId::new("MiMalloc", size), |b| {
            b.iter(|| alloc_free_batch(layout, global_alloc, global_dealloc));
        });
    }
    group.finish();
}

// One block freed right after it is allocated, as a short-lived `Box` is. `alloc_free_batch`
// repeats each call 64 times in a row, which hides the cost of a free followed by an
// allocation from the same page (see the store-forwarding entry in docs/known-issues.md).
fn bench_alloc_then_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("alloc_then_free");
    for size in [8, 24, 48, 64] {
        let layout = Layout::from_size_align(size, ALIGN).expect("valid layout");
        group.bench_function(BenchmarkId::new("MiMalloc", size), |b| {
            b.iter(|| {
                let block = unsafe { MiMalloc.alloc(black_box(layout)) };
                unsafe { MiMalloc.dealloc(black_box(block), layout) };
            });
        });
    }
    group.finish();
}

// Large enough that the frees outweigh the two channel hand-offs of each iteration.
const CROSS_THREAD_BATCH: usize = 4096;

// Allocates a batch on this thread, then hands it to a worker thread that frees it and hands
// the empty buffer back. The time includes both hand-offs, so compare free functions with
// each other rather than with `alloc_free_batch`.
fn cross_thread_batches(
    b: &mut criterion::Bencher<'_>,
    layout: Layout,
    free: impl Fn(*mut u8, Layout) + Send,
) {
    let (to_worker, from_main) = mpsc::sync_channel::<Box<[usize]>>(1);
    let (to_main, from_worker) = mpsc::sync_channel::<Box<[usize]>>(1);
    thread::scope(|scope| {
        scope.spawn(move || {
            for blocks in from_main {
                for &addr in &blocks {
                    free(ptr::with_exposed_provenance_mut(addr), layout);
                }
                if to_main.send(blocks).is_err() {
                    break;
                }
            }
        });
        let mut blocks: Box<[usize]> = vec![0; CROSS_THREAD_BATCH].into_boxed_slice();
        b.iter(|| {
            for addr in &mut blocks {
                *addr = malloc(black_box(layout)).expose_provenance();
            }
            to_worker
                .send(mem::take(&mut blocks))
                .expect("worker is running");
            blocks = from_worker.recv().expect("worker is running");
        });
        drop(to_worker);
    });
}

fn bench_cross_thread_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_thread_free");
    group.throughput(Throughput::Elements(CROSS_THREAD_BATCH as u64));
    for size in [64, 640, 4096] {
        let layout = Layout::from_size_align(size, ALIGN).expect("valid layout");
        group.bench_function(BenchmarkId::new("mi_free", size), |b| {
            cross_thread_batches(b, layout, free);
        });
        group.bench_function(BenchmarkId::new("mi_free_size_aligned", size), |b| {
            cross_thread_batches(b, layout, free_sized);
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_alloc_free,
    bench_alloc_then_free,
    bench_cross_thread_free
);
criterion_main!(benches);
