//! Allocation through `#[global_allocator]`, the way dependents use `MiMalloc`: `Box` and
//! `Vec` reach it through std's allocator shim, which the optimizer inlines into this crate's
//! call sites even without LTO, so `const_*` see a constant layout and `dyn_*` a runtime one.
//!
//! On Linux, `BENCH_WORKER_CPU` pins the freeing thread of `cross_thread_free` to that CPU;
//! pin the process itself with `taskset`.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::mem::MaybeUninit;
use std::sync::mpsc;
use std::{array, mem, thread};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mimalloc_rust::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const BATCH: usize = 64;

// `Block<W>` is a `W * 8`-byte, 8-byte aligned layout. 1032 bytes is just above
// `MI_SMALL_SIZE_MAX` (1 KiB on 64-bit targets).
type Block<const W: usize> = Box<MaybeUninit<[u64; W]>>;

fn box_batch<const W: usize>() {
    let blocks: [Block<W>; BATCH] = array::from_fn(|_| Box::new_uninit());
    black_box(&blocks);
}

fn box_pair<const W: usize>() {
    drop(black_box(Box::<[u64; W]>::new_uninit()));
}

fn vec_batch(words: usize) {
    let blocks: [Vec<u64>; BATCH] = array::from_fn(|_| Vec::with_capacity(words));
    black_box(&blocks);
}

fn bench_batches(c: &mut Criterion) {
    let mut group = c.benchmark_group("const_batch");
    group.throughput(Throughput::Elements(BATCH as u64));
    macro_rules! const_cases {
        ($($words:literal),*) => {$(
            group.bench_function(BenchmarkId::from_parameter($words * 8), |b| {
                b.iter(box_batch::<$words>);
            });
        )*};
    }
    const_cases!(1, 6, 8, 28, 80, 128, 129, 512);
    group.finish();

    let mut group = c.benchmark_group("dyn_batch");
    group.throughput(Throughput::Elements(BATCH as u64));
    for words in [1, 6, 8, 28, 80, 128, 129, 512] {
        group.bench_function(BenchmarkId::from_parameter(words * 8), |b| {
            b.iter(|| vec_batch(black_box(words)));
        });
    }
    group.finish();
}

// One block freed right after it is allocated, as a short-lived `Box` is.
fn bench_pairs(c: &mut Criterion) {
    let mut group = c.benchmark_group("const_pair");
    group.bench_function(BenchmarkId::from_parameter(8), |b| b.iter(box_pair::<1>));
    group.bench_function(BenchmarkId::from_parameter(48), |b| b.iter(box_pair::<6>));
    group.bench_function(BenchmarkId::from_parameter(1024), |b| {
        b.iter(box_pair::<128>);
    });
    group.finish();
}

const CROSS_THREAD_BATCH: usize = 4096;

#[cfg(target_os = "linux")]
fn pin_current_thread(cpu: usize) {
    unsafe extern "C" {
        fn sched_setaffinity(pid: i32, size: usize, mask: *const u64) -> i32;
    }
    let mut mask = [0_u64; 16];
    mask[cpu / 64] |= 1 << (cpu % 64);
    // SAFETY: `mask` is a valid 1024-bit `cpu_set_t`; pid 0 is the calling thread.
    let result = unsafe { sched_setaffinity(0, size_of_val(&mask), mask.as_ptr()) };
    assert_eq!(result, 0, "sched_setaffinity({cpu}) failed");
}

#[cfg(not(target_os = "linux"))]
fn pin_current_thread(_cpu: usize) {
    panic!("BENCH_WORKER_CPU is only supported on Linux");
}

// Allocates a batch on this thread, then hands it to a worker thread that frees it and hands
// the empty buffer back. The time includes both hand-offs.
fn cross_thread_batches<const W: usize>(b: &mut criterion::Bencher<'_>, worker_cpu: Option<usize>) {
    let (to_worker, from_main) = mpsc::sync_channel::<Vec<Block<W>>>(1);
    let (to_main, from_worker) = mpsc::sync_channel::<Vec<Block<W>>>(1);
    thread::scope(|scope| {
        scope.spawn(move || {
            if let Some(cpu) = worker_cpu {
                pin_current_thread(cpu);
            }
            for mut blocks in from_main {
                blocks.clear();
                if to_main.send(blocks).is_err() {
                    break;
                }
            }
        });
        let mut blocks = Vec::with_capacity(CROSS_THREAD_BATCH);
        b.iter(|| {
            blocks.extend((0..CROSS_THREAD_BATCH).map(|_| Box::<[u64; W]>::new_uninit()));
            to_worker
                .send(mem::take(&mut blocks))
                .expect("worker is running");
            blocks = from_worker.recv().expect("worker is running");
        });
        drop(to_worker);
    });
}

fn bench_cross_thread_free(c: &mut Criterion) {
    let worker_cpu = std::env::var("BENCH_WORKER_CPU")
        .ok()
        .map(|cpu| cpu.parse().expect("BENCH_WORKER_CPU is a CPU number"));
    let mut group = c.benchmark_group("cross_thread_free");
    group.throughput(Throughput::Elements(CROSS_THREAD_BATCH as u64));
    group.bench_function(BenchmarkId::from_parameter(64), |b| {
        cross_thread_batches::<8>(b, worker_cpu);
    });
    group.bench_function(BenchmarkId::from_parameter(640), |b| {
        cross_thread_batches::<80>(b, worker_cpu);
    });
    group.bench_function(BenchmarkId::from_parameter(4096), |b| {
        cross_thread_batches::<512>(b, worker_cpu);
    });
    group.finish();
}

// Mixed sizes from runtime values, as a guard against regressions outside the cases above.
fn bench_mixed(c: &mut Criterion) {
    c.bench_function("mixed_btree", |b| {
        b.iter(|| {
            let map: BTreeMap<u64, Box<[u8]>> = (0..1000_u64)
                .map(|i| {
                    (
                        i.wrapping_mul(0x9E37_79B9),
                        vec![0; (i as usize * 7) % 300 + 1].into(),
                    )
                })
                .collect();
            black_box(map)
        });
    });
}

criterion_group!(
    benches,
    bench_batches,
    bench_pairs,
    bench_cross_thread_free,
    bench_mixed
);
criterion_main!(benches);
