# AGENTS.md

Guidance for coding agents working in this repository. The [README](README.md) covers usage and features, [`mimalloc-rust-sys/UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md) the relation to upstream mimalloc, and [`docs/known-issues.md`](docs/known-issues.md) known problems; read the relevant one before changing that area.

## Layout

| Path | Role |
|---|---|
| `mimalloc-rust-sys/` | Compiles the vendored mimalloc (`c_src/mimalloc`, a submodule pinned to an upstream release) in `build.rs` and exposes the generated bindings in `src/bindings.rs` |
| `mimalloc-rust/` | `MiMalloc` (`GlobalAlloc`), `version()`, `usable_size()`, `stats_json()` |
| `bench/` | Criterion benchmark of the allocation entry points |
| `downstream-c/` | Test-only crate: a C shared library built against `DEP_MIMALLOC_INCLUDE_DIR`, checking `override` across the library boundary |
| `upstream-tests/` | Test-only crate: compiles upstream's `test/test-api.c` and `test/test-stress.c` with `DEP_MIMALLOC_DEFINES` and runs each as a `harness = false` test |
| `xtask/` | `bindgen`, `bindgen --check` and `check-symbols` |

`default-members` holds only the two library crates. Keep it that way: `bench` would make every `cargo test` build criterion, `xtask` needs libclang, `downstream-c` enables `override`, which Cargo would unify into the other members' tests, and `upstream-tests` adds a multi-second stress run.

## Setup

- Run `git submodule update --init` after cloning; `build.rs` fails without the sources.
- The xtask needs libclang. On musl hosts build it with `RUSTFLAGS="-C target-feature=-crt-static"`, because a static musl binary cannot `dlopen` libclang.

## Verification

CI (`.github/workflows/ci.yml`) runs the following; run the parts your change affects before presenting it.

```sh
cargo test --locked
cargo test --locked --release
cargo test --locked --release -p upstream-tests
# each of: secure debug no_thp local_dynamic_tls secure,debug override override,secure override,local_dynamic_tls override,debug
cargo test --locked --features mimalloc-rust/<feature>[,mimalloc-rust/<feature>]
cargo test --locked --release --features mimalloc-rust/override
cargo test --locked --release -p downstream-c       # not on static musl
cargo run --locked -p xtask -- bindgen --check
cargo run --locked -p xtask -- check-symbols
cargo +nightly fmt --all --check
cargo +nightly clippy --locked --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo +nightly doc --locked --no-deps -p mimalloc-rust -p mimalloc-rust-sys
cargo +1.99.0 test --locked                         # MSRV
```

- The `override` probe (`mimalloc-rust/tests/override_probe.rs`) only discriminates in release builds.
- `test-api` prints `succeeded: N` and returns the number of failed checks; `test-stress` aborts on corruption. The Cortex-A72 job runs both under qemu as well.
- `xtask` checks are per target: the headers pull in the target's libc headers, so a pass on macOS says nothing about Linux.
- When judging a CI run, read the logs for the number of tests that passed, not only the job status.

## Rules

- Never edit `mimalloc-rust-sys/src/bindings.rs` by hand. Regenerate it with `cargo run -p xtask -- bindgen` and review the diff.
- Never modify files under `mimalloc-rust-sys/c_src/`. Upgrade upstream by moving the submodule and following the Upgrading section of `UPSTREAM.md`.
- `build.rs` mirrors the defines and flags that upstream's `CMakeLists.txt` produces. Record every intentional difference in the Build configuration table or Intentional differences section of `UPSTREAM.md`.
- `build.rs` alone decides `MI_GUARDED`: it defines the C macro and emits `cfg(mi_guarded)`. Rust code reads `mimalloc_rust_sys::MI_GUARDED` and `MI_MALLOC_ALIGN` and never re-derives them from features.
- The targets that accept `override` are listed in `build.rs`. Changing that list means updating the README's Supported targets table, the CI matrix and the tests together.
- Every `unsafe` block or impl carries a `SAFETY:` comment; the workspace lints in the root `Cargo.toml` enforce one unsafe operation per block.
- Features must stay valid on every target, because Cargo unifies them across the dependency graph: fail the build only when a feature's promise cannot be kept.
- Do not name other Rust wrappers of mimalloc in files, comments or commit messages.

## CI notes

- GitHub cannot run JavaScript actions in Alpine containers on Arm64, so container jobs check out with plain `git` and do not use `rust-cache`.
- The lint job uses an unpinned nightly on purpose; `--allow-downgrade` falls back to the newest nightly with rustfmt and clippy.

## Writing

- Repository text, comments and commit messages are in English.
- Keep each Markdown paragraph on a single line.
- Entries in `docs/known-issues.md` use the fields Status, Targets, Symptom, Cause and If you hit it, plus fields such as Mitigation, Fix, Upstream status or Residual risk where they apply.
- Commits follow Conventional Commits with an optional scope such as `sys` or `xtask`: an imperative subject of at most 60 characters, then a blank line and a bulleted body.
