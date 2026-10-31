# mimalloc-rust

Rust bindings to [mimalloc](https://github.com/microsoft/mimalloc) v3, built from a pinned upstream submodule and linked statically.

- `mimalloc-rust`: the `MiMalloc` global allocator plus `version()`, `usable_size()` and `stats_json()`.
- `mimalloc-rust-sys`: raw bindings to the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.

The vendored mimalloc version and how the build relates to upstream are documented in [`mimalloc-rust-sys/UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md).

## Usage

```toml
[dependencies]
mimalloc-rust = { git = "https://github.com/sunlei/mimalloc-rust", tag = "v0.2.1" }
```

```rust
use mimalloc_rust::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

Cargo fetches the mimalloc submodule together with the git dependency.

mimalloc is compiled with the optimization level of the Cargo profile. To keep allocation fast in dev builds, add this to the application's `Cargo.toml`:

```toml
[profile.dev.package.mimalloc-rust-sys]
opt-level = 3
```

On Linux aarch64, mimalloc's atomics are compiled inline only when the Rust target has LSE, for example with `-C target-cpu=native`; otherwise they go through outline helpers that pick LSE at runtime. See [`UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md#intentional-differences).

Only one copy of mimalloc may be linked: the sys crate declares `links = "mimalloc"`, so Cargo rejects any other crate in the dependency graph that declares the same `links` value.

## Features

No feature is needed for a typical program: the default build mirrors upstream's release configuration (see [`mimalloc-rust-sys/UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md)). Enable one only when its condition applies.

| Feature | When to enable | Effect |
|---|---|---|
| `local_dynamic_tls` | The final artifact is loaded with `dlopen` (ELF), such as a Python extension module or a plugin | Local-dynamic TLS model. With the default initial-exec model, glibc refuses to load the artifact ("cannot allocate memory in static TLS block") once other libraries have used up the static TLS reserved for `dlopen`. musl targets always use it |
| `override` | An executable whose C code or libc should allocate from mimalloc too | Replaces the C allocation functions (`malloc`, `free`, `posix_memalign`, ...) of the executable, so C code, allocations inside libc (checked with `asprintf`) and `std::alloc::System` also use mimalloc. See [Overriding malloc](#overriding-malloc) |
| `no_thp` | Transparent huge pages cost more memory or latency than they save in your deployment | Defaults mimalloc's `allow_thp` option to off on Linux/Android; it can still be re-enabled at runtime. No effect elsewhere. `MIMALLOC_ALLOW_THP=0` does the same without rebuilding |
| `secure` | Hardening against heap exploits matters more than allocation speed | Upstream secure mode (`MI_SECURE=4`): guard pages, encrypted free lists, randomized allocation |
| `debug` | Investigating heap corruption or a suspected mimalloc bug | mimalloc internal invariant checks (`MI_DEBUG=2`), independent of the Cargo profile. `MiMalloc::dealloc` then also reports a `Layout` whose size exceeds the block's usable size, unless its alignment exceeds its size or the block is guarded; mimalloc prints the error and still frees the block. `MiMalloc` then uses the aligned allocation functions for every alignment above 1, which are slower for some sizes; see [docs/known-issues.md](docs/known-issues.md) |

## Runtime options

mimalloc reads its options from `MIMALLOC_*` environment variables, listed in upstream's [Environment Options](https://github.com/microsoft/mimalloc#environment-options), or takes them from `mi_option_set`, which needs a direct dependency on `mimalloc-rust-sys` and may only be called while no other thread can allocate (see the `mimalloc-rust-sys` docs). Upstream's defaults are general-purpose; change one only to address a measured problem. Some options are read only when the process or a thread's heap is initialized, such as `guarded_sample_rate` with `debug`, so a call made after allocation has started may not affect them; set those through the environment.

## Overriding malloc

With `override`, linking the crate is enough: `#[global_allocator]` is optional, and `use mimalloc_rust as _;` keeps an otherwise unused dependency in the link.

- Executables only. In a `cdylib` or `dylib` the replacement does not reliably take effect, and the build cannot detect the crate type, so this is not checked.
- On Linux the executable's own `malloc` definitions take precedence over libc's. On macOS a constructor makes mimalloc the default malloc zone.
- On musl the static `libc.a` also defines `malloc`, so the mimalloc archive must come before it on the link line; the sys crate depends on std under `override` so that rustc orders it that way. See [`docs/known-issues.md`](docs/known-issues.md).
- Enabling it on a target outside the list below is a build error.

## Supported targets

| Target | Verified |
|---|---|
| `aarch64-apple-darwin` | build and tests, including `override` |
| `x86_64-unknown-linux-gnu` | build and tests, including `override` |
| `x86_64-unknown-linux-musl` | build and tests, including `override` (see the musl note above) |
| `aarch64-unknown-linux-gnu` | build and tests, including `override` and ARMv8.0 without LSE |
| `aarch64-unknown-linux-musl` | build and tests, including `override` (see the musl note above) and ARMv8.0 without LSE |

Windows is not supported.

## Maintenance

```sh
git submodule update --init           # after cloning
cargo test                            # mimalloc-rust and mimalloc-rust-sys
cargo run -p xtask -- bindgen         # regenerate mimalloc-rust-sys/src/bindings.rs
cargo run -p xtask -- bindgen --check # fail if the committed bindings differ for this target
cargo run -p xtask -- check-symbols   # fail if a bound function is not defined in the library
cargo bench -p bench                  # compare mi_malloc, mi_malloc_aligned and MiMalloc
```

The xtask needs libclang. Run the checks natively on each target, because the headers include the target's libc headers. On musl hosts, build the xtask with `RUSTFLAGS="-C target-feature=-crt-static"`: a statically linked musl binary cannot `dlopen` libclang.

## License

MIT, see [LICENSE](LICENSE). The vendored mimalloc sources in `mimalloc-rust-sys/c_src/mimalloc` keep their own MIT license from Microsoft Corporation and Daan Leijen.
