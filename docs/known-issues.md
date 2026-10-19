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

## With `debug`, `MiMalloc` takes a slow path for sizes whose size class is not a power of two

**Status:** fixed except with the `debug` feature, where it is kept on purpose (see below).

**Targets:** all with the `debug` feature, through `MiMalloc::alloc`, `alloc_zeroed` and `realloc`. Before the fix, all builds.

**Symptom:** allocations whose mimalloc size class is not a power of two (for example 48, 224 or 640 bytes with alignment 8) are slower and use more memory than the same requests made through `mi_malloc`. `cargo bench --bench alloc` reproduces it. On an Apple M1 Max, before the fix, one allocate-then-free pair cost about 3.5 ns more (about 68%) at 48, 224 and 640 bytes, and `mi_usable_size` reported 64, 256 and 768 bytes for these blocks instead of 48, 224 and 640. On Linux, `mi_malloc_aligned` against `mi_malloc` at those sizes cost 108–121% more on Intel i5-13400 P-cores, 114–132% on its E-cores and 40–60% on Neoverse-V3. Sizes whose size class is a power of two (8, 24, 64, 256, 1024) showed no consistent difference on Apple M1 Max, nor from 8 to 256 bytes on Neoverse-V3; on the i5-13400 the aligned functions were 17–33% (P-cores) and 12–56% (E-cores) slower at 8 to 256 bytes as well.

**Cause:** in `alloc-aligned.c`, the inline fast path of the `mi_*_aligned` functions only takes a block from the `free` list of the page for the requested size class. `mi_free` pushes blocks freed by the owning thread onto the page's `local_free` list, and only the regular (non-aligned) generic allocation path moves `local_free` back to `free`. When the fast path finds no block, `mi_theap_malloc_zero_aligned_at_generic` uses the regular path only if `mi_malloc_is_naturally_aligned` holds, which requires the size class to be a power of two up to 4 KiB or a multiple of 4 KiB. Otherwise it over-allocates `size + alignment - 1` bytes, which lands in a larger size class. Judging from the code (not observed directly), the page for the requested size class is then never refilled, so every later request of that size goes through two non-inlined functions and the over-allocation.

**Upstream status:** the over-allocation is a regression in mimalloc v3, not part of the design. In v2 (still the case on the `dev2` branch), `mi_malloc_is_naturally_aligned` returned true for any `alignment <= MI_MAX_ALIGN_SIZE`, so `mi_malloc_aligned(48, 8)` took the regular path. The v3 rewrite removed that check in upstream commit `9ebe941c` (2024-11-30, first released in v3.0.0-alpha) and only accepted power-of-two size classes. Upstream commit `a7875866` on the `dev3` branch (2026-10-14, "relax the naturally aligned condition and check in debug builds") adds `mi_natural_alignment_of_size`, which accepts any alignment that every block of the size class has (at least the largest power of two dividing the block size, up to 16), and asserts that guarantee when pages are created. No release contains it yet: v3.5.4 (2026-09-30) does not. These findings come from the submodule's git history.

**Fix:** `MiMalloc` calls `mi_malloc`, `mi_zalloc` and `mi_realloc` when `align <= 16 && align <= size`, the same condition std's `System` uses on Unix, and the aligned functions otherwise. In the default configuration (`MI_ALIGN2W` in `page-queue.c`), size classes above 8 bytes are multiples of 16 bytes and pages start their blocks at a multiple of 16, so every such block is 16-byte aligned. `every_live_block_is_aligned` in `mimalloc-rust/tests/global.rs` checks this with many blocks of each size alive. The path is a workaround: once a release contains `a7875866`, it is re-measured and dropped if it no longer pays off (step 6 of Upgrading in `mimalloc-rust-sys/UPSTREAM.md`).

**Why `debug` keeps the slow path:** the `debug` feature builds with `MI_DEBUG=2` and without `NDEBUG`, which enables `MI_GUARDED` (`types.h`). With guarded sampling and `MIMALLOC_GUARDED_PRECISE=1`, mimalloc places a sampled object right before its guard page without rounding its size, so `mi_malloc(17)` returned 64 of 64 blocks that were not 16-byte aligned in a probe. `MiMalloc` therefore compares the alignment against `mimalloc_rust_sys::MI_MALLOC_ALIGN` instead of 16. It is 1 when the sys crate's build script enables `MI_GUARDED`, however the `debug` feature was enabled, so `MiMalloc` then uses the aligned functions for every alignment above 1. In that configuration a test binary using `MiMalloc` also aborted on mimalloc's own `_mi_memcpy_aligned` assertion during start-up, whether or not `MiMalloc` used the `mi_malloc` path.

**If you hit it:** with `debug`, pick allocation sizes with power-of-two size classes when measuring performance, or measure without `debug`.

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
