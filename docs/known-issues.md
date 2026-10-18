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
