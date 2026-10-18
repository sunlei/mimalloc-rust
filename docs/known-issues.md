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

## `MiMalloc` takes a slow path for sizes whose size class is not a power of two

**Status:** open; a fix is planned but deferred (see below).

**Targets:** all, through `MiMalloc::alloc`, `alloc_zeroed` and `realloc`.

**Symptom:** with `MiMalloc` as the global allocator, allocations whose mimalloc size class is not a power of two (for example 48, 224 or 640 bytes with alignment 8) are slower and use more memory than the same requests made through `mi_malloc`. `cargo bench --bench alloc` reproduces it. On an Apple M1 Max, one allocate-then-free pair cost about 3.5 ns more (about 68%) at 48, 224 and 640 bytes, and `mi_usable_size` reported 64, 256 and 768 bytes for these blocks instead of 48, 224 and 640. Sizes whose size class is a power of two (8, 24, 64, 256, 1024) showed no consistent difference.

**Cause:** `MiMalloc` calls the `mi_*_aligned` functions for every layout. In `alloc-aligned.c`, their inline fast path only takes a block from the `free` list of the page for the requested size class. `mi_free` pushes blocks freed by the owning thread onto the page's `local_free` list, and only the regular (non-aligned) generic allocation path moves `local_free` back to `free`. When the fast path finds no block, `mi_theap_malloc_zero_aligned_at_generic` uses the regular path only if `mi_malloc_is_naturally_aligned` holds, which requires the size class to be a power of two up to 4 KiB or a multiple of 4 KiB. Otherwise it over-allocates `size + alignment - 1` bytes, which lands in a larger size class. Judging from the code (not observed directly), the page for the requested size class is then never refilled, so every later request of that size goes through two non-inlined functions and the over-allocation.

**Planned fix:** call `mi_malloc`, `mi_zalloc` and `mi_realloc` when `align <= 16 && align <= size`, the same condition std's `System` uses on Unix, and keep the aligned functions otherwise. Two preconditions must be verified first:

- In the default configuration (`MI_ALIGN2W` in `page-queue.c`), every block of a size class above 8 bytes must be 16-byte aligned, not just the first block of a page. A test that keeps many blocks of each size alive and checks each one should guard this.
- The `debug` feature builds with `MI_DEBUG=2` and without `NDEBUG`, which enables `MI_GUARDED` (`types.h`). It is unverified whether guarded sampling weakens the alignment `mi_malloc` returns; until it is checked, the fast path should stay off with `debug`.

**If you hit it:** no workaround inside this crate yet. Pick allocation sizes with power-of-two size classes, or call `mimalloc_rust_sys::mi_malloc` directly for hot paths whose alignment you control.
