# AGENTS.md

Guidance for coding agents working in this repository. The [README](README.md) covers usage and features, [`mimalloc-rust-sys/UPSTREAM.md`](mimalloc-rust-sys/UPSTREAM.md) the relation to upstream mimalloc, and [`docs/known-issues.md`](docs/known-issues.md) known problems; read the relevant one before changing that area.

## Layout

| Path | Role |
|---|---|
| `mimalloc-rust-sys/` | Compiles the vendored mimalloc (`c_src/mimalloc`, a submodule pinned to an upstream release) in `build.rs` and exposes the generated bindings in `src/bindings.rs` |
| `mimalloc-rust/` | `MiMalloc` (`GlobalAlloc`), `version()`, `usable_size()`, `stats_json()` |
| `bench/` | Criterion benchmark of the allocation entry points |
| `dlopen-plugin/` | Test-only crate: a `cdylib` using `MiMalloc` with `local_dynamic_tls`, which its test loads with `dlopen` after filling glibc's static TLS surplus |
| `downstream-c/` | Test-only crate: a C shared library built against `DEP_MIMALLOC_INCLUDE_DIR`, checking `override` across the library boundary |
| `upstream-tests/` | Test-only crate: compiles the upstream C tests that `CMakeLists.txt` registers (the list is in its `build.rs`) with `DEP_MIMALLOC_DEFINES` and runs each as a `harness = false` test; `qsbr` compiles its own copy of the allocator and links only that |
| `xtask/` | `bindgen`, `bindgen --check` and `check-symbols` |

`default-members` holds only the two library crates. Keep it that way: `bench` would make every `cargo test` build criterion, `xtask` needs libclang, `downstream-c` and `dlopen-plugin` enable `override` and `local_dynamic_tls`, which Cargo would unify into the other members' tests, and `upstream-tests` adds a multi-second stress run.

## Setup

- Run `git submodule update --init` after cloning; `build.rs` fails without the sources.
- The xtask needs libclang. On musl hosts build it with `RUSTFLAGS="-C target-feature=-crt-static"`, because a static musl binary cannot `dlopen` libclang.

## Verification

`.github/workflows/ci.yml` is the authoritative list of checks; run the ones your change affects before presenting it. The usual minimum is `cargo test --locked`, `cargo +nightly fmt --all --check` and `cargo +nightly clippy --locked --workspace --all-targets --all-features -- -D warnings`.

- The `override` probe (`mimalloc-rust/tests/override_probe.rs`) only discriminates in release builds.
- Most upstream tests print `succeeded: N` and return the number of failed checks, `fork` prints `forks: N, failed: N`, and the stress tests abort on corruption. Tests that cannot run on a target print `skipped:` there. The Cortex-A72 job runs them all under qemu as well. With `debug`, guarded sampling only happens when `MIMALLOC_GUARDED_SAMPLE_RATE` is set before the process starts; the `guarded` line of the stress statistics shows whether it did. Run them with `--release`, as CI does: cc compiles mimalloc at `-O0` in dev builds, which is not the code that ships.
- `xtask` checks are per target: the headers pull in the target's libc headers, so a pass on macOS says nothing about Linux.
- When judging a CI run, read the logs for the number of tests that passed, not only the job status.

## Rules

- Never edit `mimalloc-rust-sys/src/bindings.rs` by hand. Regenerate it with `cargo run -p xtask -- bindgen` and review the diff.
- Never modify files under `mimalloc-rust-sys/c_src/`. Upgrade upstream by moving the submodule and following the Upgrading section of `UPSTREAM.md`.
- `build.rs` mirrors the defines and flags that upstream's `CMakeLists.txt` produces. Record every intentional difference in the Build configuration table or Intentional differences section of `UPSTREAM.md`.
- `build.rs` alone decides `MI_DEBUG` and `MI_GUARDED`: it defines the C macros and emits `cfg(mi_debug)` and `cfg(mi_guarded)`. Rust code reads `mimalloc_rust_sys::MI_DEBUG`, `MI_GUARDED` and `MI_MALLOC_ALIGN` and never re-derives them from features.
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
