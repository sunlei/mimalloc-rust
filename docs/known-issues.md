# Known issues

## `override` on musl depends on rustc's link order

**Status:** mitigated; a residual risk remains (see below).

**Targets:** `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, with the `override` feature.

**Symptom:** the link fails with errors such as:

```
alloc-override.c:352: multiple definition of `posix_memalign'; .../self-contained/libc.a(posix_memalign.lo): first defined here
free.c:289: multiple definition of `free'; .../self-contained/libc.a(free.lo): first defined here
alloc.c:338: multiple definition of `__libc_malloc'; .../self-contained/libc.a(lite_malloc.lo): first defined here
```

often together with `undefined reference to '__stack_chk_guard'` from mimalloc's sources.

**Cause:** musl targets link the static `libc.a`, which defines `malloc`, `free` and friends just like mimalloc does with `override`. GNU ld resolves archives in a single pass, so the link only works when the mimalloc archive (linked with `+whole-archive`, see `mimalloc-rust-sys/UPSTREAM.md`) comes before `libc.a`: mimalloc's definitions are then already in place and the allocator members of `libc.a` are never pulled in. When `libc.a` comes first, its allocator members are pulled in to satisfy std, and the forced inclusion of mimalloc then defines the same symbols a second time.

rustc places every crate before the crates it depends on, but leaves unrelated crates in the order they were loaded. `mimalloc-rust-sys` is `#![no_std]`, so nothing ordered it relative to the libc that std links. In the test binary of a `no_std` crate, where std only enters through the test harness, `libc.a` came first. This was observed for the `mimalloc-rust` lib test and for a separate `no_std` library that only contains `use mimalloc_rust as _;`.

A build script can only add link arguments for its own package, so there is no way to place mimalloc's object first on a downstream link line, as upstream's static override instructions recommend.

**Mitigation:** with `override`, `mimalloc-rust-sys` declares `extern crate std`. The dependency makes rustc place the sys crate, and with it the mimalloc archive, before std and its `libc.a`. On both musl targets, in dev and release, the previously failing test binaries now link and pass without changes to the depending crates; removing the declaration brings the failure back. The `mimalloc-rust` lib test stays enabled as the regression check.

**Residual risk:** the fix relies on rustc ordering crates before their dependencies on the link line. Single-pass linkers need that order, but rustc does not document it as a guarantee. A link-order problem has only been observed as the link error above; this does not prove that every possible link configuration fails loudly.

**If you hit it:** build without `override` on that target, and report the crate layout that produced the failing link line (`cargo rustc ... -- --print link-args` shows the order).

## Before mimalloc v3.6.0, the aligned functions took a slow path for sizes whose size class is not a power of two

**Status:** fixed in mimalloc v3.6.0. `MiMalloc` still calls `mi_malloc` where it can, because the aligned functions remain slower: on x86_64 at every measured size, on aarch64 mainly at 1 KiB and 4 KiB (see Fix).

**Targets:** all, through the `mi_*_aligned` functions of mimalloc before v3.6.0. `MiMalloc::alloc`, `alloc_zeroed` and `realloc` used them for every alignment before this crate added its `mi_malloc` path, and still use them with the `debug` feature.

**Symptom:** allocations whose mimalloc size class is not a power of two (for example 48, 224 or 640 bytes with alignment 8) are slower and use more memory than the same requests made through `mi_malloc`. `cargo bench --bench alloc` reproduces it. On an Apple M1 Max, before the fix, one allocate-then-free pair cost about 3.5 ns more (about 68%) at 48, 224 and 640 bytes, and `mi_usable_size` reported 64, 256 and 768 bytes for these blocks instead of 48, 224 and 640. On Linux, `mi_malloc_aligned` against `mi_malloc` at those sizes cost 108–121% more on Intel i5-13400 P-cores, 114–132% on its E-cores and 40–60% on Neoverse-V3. Sizes whose size class is a power of two (8, 24, 64, 256, 1024) showed no consistent difference on Apple M1 Max, nor from 8 to 256 bytes on Neoverse-V3; on the i5-13400 the aligned functions were 17–33% (P-cores) and 12–56% (E-cores) slower at 8 to 256 bytes as well.

**Cause:** in `alloc-aligned.c` of v3.5.3, the inline fast path of the `mi_*_aligned` functions only takes a block from the `free` list of the page for the requested size class. `mi_free` pushes blocks freed by the owning thread onto the page's `local_free` list, and only the regular (non-aligned) generic allocation path moves `local_free` back to `free`. When the fast path finds no block, `mi_theap_malloc_zero_aligned_at_generic` uses the regular path only if `mi_malloc_is_naturally_aligned` holds, which requires the size class to be a power of two up to 4 KiB or a multiple of 4 KiB. Otherwise it over-allocates `size + alignment - 1` bytes, which lands in a larger size class. Judging from the code (not observed directly), the page for the requested size class is then never refilled, so every later request of that size goes through two non-inlined functions and the over-allocation.

**Upstream status:** the over-allocation is a regression in mimalloc v3, not part of the design. In v2 (still the case on the `dev2` branch), `mi_malloc_is_naturally_aligned` returned true for any `alignment <= MI_MAX_ALIGN_SIZE`, so `mi_malloc_aligned(48, 8)` took the regular path. The v3 rewrite removed that check in upstream commit `9ebe941c` (2024-11-30, first released in v3.0.0-alpha) and only accepted power-of-two size classes. Upstream commit `a7875866` on the `dev3` branch (2026-10-14, "relax the naturally aligned condition and check in debug builds") adds `mi_natural_alignment_of_size`, which accepts any alignment that every block of the size class has (at least the largest power of two dividing the block size, up to 16), and asserts that guarantee when pages are created. v3.6.0 (2026-10-23) is the first release that contains it. These findings come from the submodule's git history.

**Fix:** `MiMalloc` calls `mi_malloc`, `mi_zalloc` and `mi_realloc` when `align <= 16 && align <= size`, the same condition std's `System` uses on Unix, and the aligned functions otherwise. In the default configuration (`MI_ALIGN2W` in `page-queue.c`), size classes above 8 bytes are multiples of 16 bytes and pages start their blocks at a multiple of 16, so every such block is 16-byte aligned. `every_live_block_is_aligned` in `mimalloc-rust/tests/global.rs` checks this with many blocks of each size alive. With v3.6.0 the path is no longer needed to avoid the over-allocation, but it is kept for speed: on Neoverse-V3 (Graviton5), over five code layouts each, `mi_malloc_aligned(size, 8)` in `alloc_free_batch` no longer had the cliff at 48, 224 and 640 bytes (48–59% slower than `mi_malloc` with v3.5.3, at most 5% with v3.6.0), but was still 4–12% slower at 256, 640, 1024 and 4096 bytes with Clang 19.1.7 and 3–12% slower at 256, 1024 and 4096 bytes with GCC 14.2. With v3.6.1, it was 13% slower at 4096 bytes on Neoverse-V3 with both compilers, and with GCC also 11% at 1024 bytes and 2–6% at 8, 64, 224, 256 and 640 bytes; Clang showed no difference beyond layout noise at the other sizes. On a performance core of an Intel Core i5-13400, it was 18–27% slower from 8 to 640 bytes and 11–14% slower at 1024 and 4096 bytes with both compilers, as with v3.5.3 at the power-of-two size classes (see Symptom). Step 6 of Upgrading in `mimalloc-rust-sys/UPSTREAM.md` re-measures this.

**Why `debug` uses the aligned functions:** the `debug` feature builds with `MI_DEBUG=2`, without `NDEBUG` and with `MI_GUARDED=2`, as upstream's CMake Debug build does. With guarded sampling and `MIMALLOC_UNSAFE_GUARDED_PRECISE=1` (`MIMALLOC_GUARDED_PRECISE=1` before v3.6.0), mimalloc places a sampled object right before its guard page without rounding its size, so `mi_malloc(17)` returned 64 of 64 blocks that were not 16-byte aligned in a probe. `MiMalloc` therefore compares the alignment against `mimalloc_rust_sys::MI_MALLOC_ALIGN` instead of 16. It is 1 when the sys crate's build script enables `MI_GUARDED`, however the `debug` feature was enabled, so `MiMalloc` then uses the aligned functions for every alignment above 1. In that configuration a test binary using `MiMalloc` also aborted on mimalloc's own `_mi_memcpy_aligned` assertion during start-up, whether or not `MiMalloc` used the `mi_malloc` path.

**If you hit it:** this crate's mimalloc (v3.6.1) contains the fix. With an older mimalloc, call `mi_malloc` for alignments it already guarantees, as `MiMalloc` does.

## On aarch64, a free followed by an allocation stalls on store-to-load forwarding

**Status:** fixed in mimalloc v3.6.0. With v3.5.3, `build.rs` worked around it with compiler flags.

**Targets:** all aarch64 targets, with mimalloc v3.5.3.

**Symptom:** freeing a small block and then allocating one of the same size class, as a short-lived `Box` does, costs a few nanoseconds more than with mimalloc v3.3.2. `cargo bench --bench alloc` reproduces it in `alloc_then_free`; `alloc_free_batch` hides it. With v3.5.3 and the compilers' default flags, one allocate-then-free pair took 6.7–6.9 ns on Neoverse-V3 (Graviton5) with both Clang 19.1.7 and GCC 14.2, and 6.7–7.0 ns on an Apple M1 Max; with the pairing disabled it took 4.1–4.5 ns and 5.1–5.4 ns. In a downstream application on Neoverse-V3, constructing and dropping one small boxed object went from 17 ns with v3.3.2 to 22–23 ns with v3.5.3, and back to 17 ns with the pairing disabled.

**Cause:** in v3.5.3, `free` (offset 0x10), `xused` (0x18, 64 bits wide) and `local_free` (0x20) are adjacent fields of `mi_page_t`. The inline fast path of `mi_malloc` reads and writes `free` and `xused`, and `mi_free` reads and writes `xused` and `local_free`. Clang and GCC merge each pair into one 16-byte `ldp`/`stp`, so a call loads 16 bytes that the previous call stored only half of. The likely mechanism is that store-to-load forwarding cannot serve such a load, so it waits for the store to complete; this is inferred from the disassembly and from the effect of disabling the pairing, not observed with hardware counters. In v3.3.2 the used count was 16 bits wide and accessed with `ldrh`/`strh` at its own address, so each load matched an earlier store exactly. On Neoverse-V3, `MI_FREE_USE_PAGEMAP=1` (page lookup through the page map instead of the aligned page meta data) kept the pairing and did not help, and neither did `MI_PROFILE=0` or `MI_STATS=0`.

**Fix:** upstream commit `97178171` ("change layout of mi_page_t to avoid ldp/stp on arm64"), released in v3.6.0, moves the fields apart: `mi_malloc` accesses `free` (0x10) and `xused` (0x28), `mi_free` accesses `local_free` (0x18) and `xused`. Clang 19.1.7 and GCC 14.2 then emit single `ldr`/`str` instructions for them without any flags. With v3.5.3, `build.rs` passed `-mllvm -aarch64-enable-ldst-opt=false` to Clang and `--param=aarch64-ldp-policy=never --param=aarch64-stp-policy=never` to GCC 14 and later; it no longer does. On Neoverse-V3, over five code layouts each, `alloc_then_free` and `alloc_free_batch` with v3.6.0 showed no difference with and without these flags (at most 1.9%) with either compiler, and `alloc_then_free` with v3.6.0 matched v3.5.3 with the flags.

**Upstream status:** reported as [microsoft/mimalloc#1433](https://github.com/microsoft/mimalloc/issues/1433) and fixed in v3.6.0. With the C reproduction from the issue on Neoverse-V3, one `mi_malloc(32)` + `mi_free` took 3.98 ns with v3.3.2, 4.91 ns with v3.5.3 and 4.01 ns with v3.6.0 (Clang 19.1.7, median over three code layouts). With GCC 14.2, v3.6.0 took 3.89 ns against 3.71 ns for v3.5.3 with the pairing disabled, and 4.41 ns against 3.89 ns for `mi_malloc(256)`; `alloc_then_free`, which only measures up to 64 bytes, showed no such difference.

**If you hit it:** use mimalloc v3.6.0 or later. To check a build, disassemble `mi_malloc` and `mi_free`: the accesses to `free`, `local_free` and `xused` of the page should be single `ldr`/`str` instructions, not `ldp`/`stp`.

## Since mimalloc v3.6.0, batches of 1 KiB and 4 KiB allocations are slower

**Status:** mostly fixed in mimalloc v3.6.1, which raised the default merge threshold to 4 MiB. Batches of these sizes remain up to 7% slower than with v3.5.3; turning the merging off removes at most 2 percentage points of that, and the rest has no single cause (see Cause).

**Targets:** all. Measured on Linux aarch64 and x86_64 and on macOS aarch64.

**Symptom:** `alloc_free_batch` of `cargo bench --bench alloc`, which allocates 64 blocks of one size and then frees them on one thread, is slower at 1024 and 4096 bytes than with v3.5.3, through `MiMalloc` and `mi_malloc` alike. With v3.6.0, on Neoverse-V3 (Graviton5), over five code layouts each, it was 7% and 11% slower with Clang 19.1.7 and 12% and 17% slower with GCC 14.2; on an Apple M1 Max (one code layout), 9% and 11%. From 8 to 640 bytes the changes ranged from 10% faster to 6% slower. With v3.6.1, over five code layouts each, it was 4–6% slower at 1024 bytes and unchanged at 4096 bytes with Clang and 4–7% slower at both sizes with GCC on Neoverse-V3; on a performance core of an Intel Core i5-13400, it was 3% slower at 4096 bytes with Clang and 3–4% slower at both sizes with GCC. On aarch64, the v3.5.3 builds in these comparisons used the pairing flags of the store-forwarding entry above.

**Cause:** v3.6.0 merges a thread's allocation statistics into those of its heap whenever the thread has allocated and freed `stats_merge_threshold` bytes since the last merge, by default 512 KiB. The option is new in v3.6.0, for [microsoft/mimalloc#1410](https://github.com/microsoft/mimalloc/issues/1410): it came with a default of 0 from upstream PR #1412, and upstream commit `0d2fd910` set it to 512 KiB. v3.5.3 merged only when a thread collected or exited. Each merge costs about the same, so the slowdown grows with the bytes moved per allocation: a batch of 64 blocks of 4096 bytes moves 512 KiB and merges once per batch. Bisected between v3.5.3 and v3.6.0 with a C program running the same loop on Neoverse-V3: it took 589 ns per batch of 1024-byte blocks with v3.5.3 and 642 ns with v3.6.0, and 1880 and 2095 ns with 4096-byte blocks (Clang 19.1.7, median over three code layouts). With `MIMALLOC_STATS_MERGE_THRESHOLD=0`, v3.6.0 took 572–576 and 1901–1911 ns. On a performance core of an Intel Core i5-13400, the program took 534 and 597 ns at 1024 bytes and 1886 and 2128 ns at 4096 bytes with Clang 19.1.7, and 550 and 644 ns and 2010 and 2338 ns with GCC 14.2 (v3.5.3 and v3.6.0, median over three code layouts); with merging off, v3.6.0 still executed 2–8% more instructions per batch than v3.5.3 there. Upstream commit `f0032d4c`, released in v3.6.1, raised the default to 4 MiB, so a batch of 4096-byte blocks merges once every eight batches. Run again in one session, the C program took 572 and 1906 ns with v3.6.1 on Neoverse-V3 with Clang, no different from v3.5.3 (575 and 1883 ns), but 590 and 1985 ns with GCC, 4% and 7% more than v3.5.3 (567 and 1862 ns); on the i5-13400, v3.6.1 was 3–5% slower than v3.5.3 with either compiler. Turning the merging off changed the v3.6.1 times by at most 2%. On Neoverse-V3 with GCC, a bisection with merging off found no single cause for the rest: upstream commit `2579b098`, which reads the threshold option on every page statistics update even when merging is off, added 44 ns per batch of 4096-byte blocks, and later commits added the remainder. A profile shows that at this size most of the time goes to page-level work (finding, extending, retiring and freeing pages, and the arena bitmaps), and that v3.6.1 executes more instructions than v3.5.3 across that path (page queues, page retirement, arena frees, statistics): 11% more per batch in total with GCC and 3% more with Clang. Writing to the blocks did not shrink the GCC difference: with the default threshold it was 140 ns per batch of 4096-byte blocks without writing, and 140 and 135 ns (7% and 3%) when the first 64 bytes or all of each block were written.

**Mitigation:** the threshold can be changed with `MIMALLOC_STATS_MERGE_THRESHOLD` before the process starts, or with `mi_option_set(mi_option_stats_merge_threshold, ...)`; the value is in bytes. With v3.6.1 there is little left to gain, since even 0 changed the C program by at most 2%. A larger threshold makes process-wide statistics such as `stats_json()` lag by up to that many bytes per thread, and 0 turns the merging off, which brings back the problem of #1410: a thread that frees memory but rarely allocates keeps its frees out of those statistics until it collects or exits.

**Upstream status:** reported as [microsoft/mimalloc#1458](https://github.com/microsoft/mimalloc/issues/1458) and closed after `f0032d4c`. The maintainer prefers to keep the merging on in release builds so that statistics stay useful, and noted that the benchmark does not touch the memory it allocates, which may make it more sensitive to this than real workloads. The remainder is not reported upstream, since no single commit explains it.

**If you hit it:** this crate's mimalloc (v3.6.1) contains the larger default. The remainder does not depend on the threshold; if it matters for your workload, measure the workload itself against v3.5.3.

## GCC warns about a write to address zero in `_mi_theap_malloc_guarded`

**Status:** a false positive in upstream code; nothing to change.

**Targets:** Linux targets, whose C compiler is GCC, in optimized (release) builds. Clang on macOS does not report it.

**Symptom:** the build of `mimalloc-rust-sys` prints:

```
include/mimalloc/atomic.h:54:35: warning: '__atomic_fetch_or_8' writing 8 bytes into a region of size 0 overflows the destination [-Wstringop-overflow=]
...
In function '_mi_theap_malloc_guarded':
cc1: note: destination object is likely at address zero
```

**Cause:** `mi_block_ptr_set_guarded` (`src/sample-guarded.c`) looks up the page of a block with `_mi_ptr_page` and sets a flag on it with an atomic `or`. After inlining, GCC sees that the page-map lookup can return NULL for an address mimalloc does not manage, and warns about the write through that NULL page. The block passed in was just allocated by `_mi_malloc_generic_no_sample` and checked for NULL, so it lies in a mimalloc page and the lookup finds that page.

Without the `debug` feature the function is compiled but never called: its only caller, in `src/sample-profile.c`, is inside `#if MI_GUARDED` (the other one, in `src/alloc-aligned.c`, is inside `#if 0`).

**If you hit it:** ignore it. Report it if the warning names a different function.
