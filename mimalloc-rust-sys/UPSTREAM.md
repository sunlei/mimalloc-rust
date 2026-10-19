# Upstream mimalloc

| | |
|---|---|
| Repository | https://github.com/microsoft/mimalloc |
| Tag | `v3.5.3` (upstream readme: "Latest release: v3.5.3 recommended") |
| Commit | `d4881d338125e1cb7c47ba4cfb398d6f7c0c8d45` |
| Location | `c_src/mimalloc` (git submodule) |
| Local patches | None |

Only `include/` and `src/static.c` (which `#include`s every other source file) take part in the build. The `v3.5.4` tag was skipped on purpose: it has no GitHub release, its readme still recommends v3.5.3, and it adds work-in-progress profiler commits.

## Build configuration

`build.rs` reproduces the defines and compiler flags that upstream's `CMakeLists.txt` produces for a release static library on the supported targets. Warning flags are not mirrored.

| Setting | When | Upstream reference |
|---|---|---|
| `MI_DEBUG=0`, `NDEBUG=1` | default | CMake release build; `types.h` would otherwise default `MI_DEBUG` to 2 because cc never defines `NDEBUG` |
| `MI_DEBUG=2` | `debug` feature | CMake Debug build (`MI_DEBUG=INTERNAL`) |
| `MI_GUARDED=0`, or `MI_GUARDED=1` with the `debug` feature | always | The value `types.h` derives from `MI_DEBUG` and `NDEBUG`, set explicitly so that `build.rs` also reports it to Rust as `cfg(mi_guarded)`. A CMake Debug build uses `MI_GUARDED=2` (fine-grained sample rates) instead |
| `MI_PROFILE` not set (`types.h` default 1) | always | CMake's default when `MI_DEBUG` is off. A CMake Debug build uses `MI_PROFILE=2`, which allows finer profiling sample rates (below 64 KiB) and selects `MI_SAMPLE=2` |
| `MI_SECURE=4` | `secure` feature | `MI_SECURE=ON` |
| `MI_LIBC_MUSL=1` | musl targets | `MI_LIBC_MUSL=ON`, which upstream expects the builder to set; it changes thread-pointer selection in `prim-tls.h` |
| `MI_DEFAULT_ALLOW_THP=0` | `no_thp` feature on Linux/Android | What `MI_NO_THP=ON` turns into. Defining the C macro `MI_NO_THP` instead would only skip `madvise` and keep the runtime option enabled |
| `-ftls-model=initial-exec` | default | CMake for Clang/GNU |
| `-ftls-model=local-dynamic` | musl targets, or `local_dynamic_tls` feature | CMake static musl build (issue #644), or `MI_TLS_MODEL=LOCAL_DYNAMIC` |
| `-fvisibility=hidden` | always | CMake for Clang/GNU. In a static build `mi_decl_export` is empty, so this hides the `mi_*` API from dynamic export too; static linking still resolves it |
| `-mno-outline` | Clang | CMake for Clang |
| `-march=armv8.3-a`, `-Xclang -fno-split-cold-code` | `aarch64-apple-darwin` | CMake `MI_OPT_ARCH` default on Apple arm64; AppleClang on arm64 |
| `MI_MALLOC_OVERRIDE`, `-fno-builtin-malloc` | `override` feature | `MI_OVERRIDE=ON` |
| `MI_OSX_ZONE=1`, `MI_OSX_INTERPOSE=1` | `override` feature on macOS | `MI_OVERRIDE=ON` defaults. Interpose only takes effect in a shared library (`MI_SHARED_LIB_EXPORT`); in this static build the `constructor(101)` in `src/prim/osx/alloc-override-zone.c` installs the mimalloc zone. `static.c` includes that file under `MI_OSX_ZONE` |

### Intentional differences

- **Linux aarch64 CPU baseline.** Upstream's `MI_OPT_ARCH` default adds `-march=armv8.1-a` on arm64 Linux for LSE atomics. That raises the C code above the ARMv8.0 baseline of Rust's `aarch64-unknown-linux-{gnu,musl}` targets and traps (SIGILL) on cores such as Cortex-A53/A72. We keep the compiler's ARMv8.0 default and pass `-moutline-atomics`, which selects LSE at runtime when available.
  - Verified: no inline LSE instructions in the archive (all atomics call `__aarch64_*` helpers), and the test suite passes under `qemu-aarch64 -cpu cortex-a72`, where a `-march=armv8.1-a` control program traps.

- **No load/store pairing on aarch64.** Upstream compiles with the compiler's default, which merges the adjacent `mi_page_t` field accesses of `mi_malloc` and `mi_free` into `ldp`/`stp` pairs that defeat store-to-load forwarding (see [`docs/known-issues.md`](../docs/known-issues.md)). On every aarch64 target we pass `-mllvm -aarch64-enable-ldst-opt=false` to Clang, and `--param=aarch64-ldp-policy=never --param=aarch64-stp-policy=never` to GCC when it accepts them (GCC 14 and later). This applies to the whole library, not just these two functions.
  - Verified: in archives built by `build.rs`, Debian clang 19.1.7, Debian GCC 14.2 and Apple clang 21 emit separate `ldr`/`str` for these accesses.

- **Static override linking.** Upstream recommends linking `mimalloc.o` as the first object. Cargo cannot place an object from a dependency on the final link line, so the archive is linked with `+whole-archive` instead. This guarantees the object is included but not where.
  - Without it, a release program that calls no `mi_*` function kept libc's `malloc` on the GNU ld targets (aarch64 gnu, both musl); x86_64 gnu (rust-lld) and macOS (ld64) happened to work.
  - On musl the archive must also come before `libc.a`. rustc only orders a crate before its dependencies, so under `override` the sys crate declares `extern crate std` (`src/lib.rs`); without it, the test binary of a `no_std` crate linked `libc.a` first and failed with duplicate `malloc` definitions. Details in [`docs/known-issues.md`](../docs/known-issues.md).

## Bindings

`src/bindings.rs` is generated by `cargo run -p xtask -- bindgen` with bindgen 0.73.2 from `mimalloc.h` and `mimalloc-stats.h`, and is verified per target with `bindgen --check` and `check-symbols`.

The same file was reproduced byte for byte with:

| Target | libclang |
|---|---|
| `aarch64-apple-darwin` | Apple clang 21.0.0 |
| `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu` | Debian trixie `libclang-19-dev` 19.1.7 |
| `aarch64-unknown-linux-musl`, `x86_64-unknown-linux-musl` | Alpine `clang22-dev` 22.1.3 (xtask built with `-C target-feature=-crt-static`) |

Not bound:

- `wchar_t`: its signedness differs between targets (unsigned on Linux aarch64), so `src/lib.rs` defines it per target.
- `mi_collect_reduce`, `mi_stats_merge`: declared in `mimalloc.h` but not defined anywhere in v3.5.3.
- `mi_malloc_size`, `mi_malloc_usable_size`: only defined when `MI_MALLOC_OVERRIDE` is set.
- `static inline` helpers: `mi_wsize_from_size`, `mi_*_csize`, `mi_free_csize*`, `mi_stats_header_init`, `mi_stats_init`.
  - The crate docs in `src/lib.rs` describe how to replace `mi_stats_init` before calling the `*_stats_get` functions.

## Upgrading

1. Move the submodule to the new release commit and update the table above.
2. Re-read the `CMakeLists.txt` changes and update `build.rs` and the build configuration table.
3. Run `cargo run -p xtask -- bindgen` and review the diff, including newly declared functions.
4. Run `cargo run -p xtask -- check-symbols`; exclude any declaration without a definition and record it above.
5. Run the full test matrix (CI) on every supported target, including the Cortex-A72 job.
6. If the release contains upstream commit `a7875866` (the relaxed natural-alignment check), run `cargo bench -p bench` on macOS and on Linux x86_64 and aarch64, and compare `mi_malloc_aligned` with `mi_malloc` at every benchmarked size. If they match within noise, prefer dropping `MiMalloc`'s `mi_malloc` path, which takes its size-class and guarded alignment reasoning with it; keep the path only if the aligned functions remain measurably slower. With v3.5.3 they were already 17–33% slower on an Intel i5-13400 at sizes whose size class is a power of two, which already pass the natural-alignment check that the commit relaxes. Update the aligned slow path entry in `docs/known-issues.md` either way.
7. On aarch64, check whether the release still accesses `free`, `xused` and `local_free` of `mi_page_t` as adjacent 8-byte fields in `mi_malloc` and `mi_free`. If it does not, build without the pairing flags in `build.rs`, compare `alloc_then_free` and `alloc_free_batch` of `cargo bench -p bench` with and without them on Linux aarch64, and drop the flags if they no longer pay off. Update the store-forwarding entry in `docs/known-issues.md` either way.
