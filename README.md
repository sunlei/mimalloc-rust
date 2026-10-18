# mimalloc-rust

Rust bindings to [mimalloc](https://github.com/microsoft/mimalloc) v3, built from a pinned upstream submodule and linked statically.

- `mimalloc-rust`: the `MiMalloc` global allocator plus `version()`, `usable_size()` and `stats_json()`.
- `mimalloc-rust-sys`: raw bindings to the linkable C ABI of `mimalloc.h` and `mimalloc-stats.h`.

The vendored mimalloc version and how the build relates to upstream are documented in [`mimalloc-rust-sys/UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md).

## Usage

```toml
[dependencies]
mimalloc-rust = { git = "https://github.com/sunlei/mimalloc-rust", tag = "v0.1.0" }
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

Only one copy of mimalloc may be linked: the sys crate declares `links = "mimalloc"`, so Cargo rejects any other crate in the dependency graph that declares the same `links` value.

## Features

| Feature | Effect |
|---|---|
| `secure` | Upstream secure mode (`MI_SECURE=4`): guard pages, encrypted free lists, randomized allocation |
| `debug` | mimalloc internal invariant checks (`MI_DEBUG=2`), independent of the Cargo profile |
| `no_thp` | Defaults mimalloc's `allow_thp` option to off on Linux/Android; it can still be re-enabled at runtime. No effect elsewhere |
| `local_dynamic_tls` | Local-dynamic TLS model, needed when the final artifact is loaded with `dlopen` (ELF). musl targets always use it |

## Supported targets

| Target | Verified |
|---|---|
| `aarch64-apple-darwin` | build and tests |
| `x86_64-unknown-linux-gnu` | build and tests |
| `x86_64-unknown-linux-musl` | build and tests |
| `aarch64-unknown-linux-gnu` | build and tests, including ARMv8.0 without LSE |
| `aarch64-unknown-linux-musl` | build and tests, including ARMv8.0 without LSE |

Windows is not supported.

## Maintenance

```sh
git submodule update --init           # after cloning
cargo test                            # mimalloc-rust and mimalloc-rust-sys
cargo run -p xtask -- bindgen         # regenerate mimalloc-rust-sys/src/bindings.rs
cargo run -p xtask -- bindgen --check # fail if the committed bindings differ for this target
cargo run -p xtask -- check-symbols   # fail if a bound function is not defined in the library
```

The xtask needs libclang. Run the checks natively on each target, because the headers include the target's libc headers. On musl hosts, build the xtask with `RUSTFLAGS="-C target-feature=-crt-static"`: a statically linked musl binary cannot `dlopen` libclang.
