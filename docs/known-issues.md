# Known issues

## `override` on musl depends on rustc's link order

**Targets:** `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, with the `override` feature.

**Symptom:** the link fails with errors such as:

```
alloc-override.c:352: multiple definition of `posix_memalign'; .../self-contained/libc.a(posix_memalign.lo): first defined here
free.c:289: multiple definition of `free'; .../self-contained/libc.a(free.lo): first defined here
alloc.c:338: multiple definition of `__libc_malloc'; .../self-contained/libc.a(lite_malloc.lo): first defined here
```

often together with `undefined reference to '__stack_chk_guard'` from mimalloc's sources.

**Cause:** musl targets link the static `libc.a`, which defines `malloc`, `free` and friends just like mimalloc does with `override`. GNU ld resolves archives in a single pass, so the link only works when the mimalloc archive (linked with `+whole-archive`, see `mimalloc-rust-sys/UPSTREAM.md`) comes before `libc.a`: mimalloc's definitions are then already in place and the allocator members of `libc.a` are never pulled in. When `libc.a` comes first, its allocator members are pulled in to satisfy std, and the forced inclusion of mimalloc then defines the same symbols a second time.

The order is chosen by rustc and cannot be set from this crate. A build script can only add link arguments for its own package, so there is no way to place mimalloc's object first on a downstream link line, as upstream's static override instructions recommend.

**Where it was observed:** only in the unit-test binary of the `mimalloc-rust` library itself (`cargo test --lib --features override`), where rustc puts the std rlibs and `-lc` before `libmimalloc_rust_sys`. That binary contains no tests and is disabled with `[lib] test = false`. Why rustc orders this binary differently has not been determined; one guess is that both crates are `#![no_std]`, so std only enters the graph through the test harness.

**Where it works** (both musl targets, dev and release): integration tests, doctests, the `mimalloc-rust-sys` tests, and a separate executable crate depending on `mimalloc-rust` with `override`, both with `#[global_allocator]` and with only `use mimalloc_rust as _;`. In those binaries `strdup` returns mimalloc blocks and `MIMALLOC_SHOW_STATS=1` prints mimalloc's statistics.

**Impact:** a failure is always a link error; the build never succeeds while silently keeping musl's allocator.

**If you hit it:** build without `override` on that target, and report the crate layout that produced the failing link line (`cargo rustc ... -- --print link-args` shows the order).
