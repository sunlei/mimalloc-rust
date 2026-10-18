//! Compiles the vendored mimalloc sources into a static library.
//!
//! Defines and flags mirror what upstream's `CMakeLists.txt` produces for a release
//! static library; every intentional difference is recorded in `UPSTREAM.md`.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let mimalloc_dir = manifest_dir.join("c_src/mimalloc");
    let static_source = mimalloc_dir.join("src/static.c");
    assert!(
        static_source.is_file(),
        "mimalloc sources are missing at {}; run `git submodule update --init`",
        mimalloc_dir.display()
    );
    println!("cargo:rerun-if-changed=c_src/mimalloc/include");
    println!("cargo:rerun-if-changed=c_src/mimalloc/src");

    let target_os = cfg("CARGO_CFG_TARGET_OS");
    let target_env = cfg("CARGO_CFG_TARGET_ENV");
    let target_arch = cfg("CARGO_CFG_TARGET_ARCH");
    let is_musl = target_env == "musl";

    let mut build = cc::Build::new();
    build
        .file(&static_source)
        .include(mimalloc_dir.join("include"))
        .include(mimalloc_dir.join("src"));

    // Without explicit values `MI_DEBUG` defaults to 2 whenever `NDEBUG` is undefined
    // (types.h), and cc never defines `NDEBUG`.
    if feature("DEBUG") {
        build.define("MI_DEBUG", "2");
    } else {
        build.define("MI_DEBUG", "0").define("NDEBUG", "1");
    }
    if feature("SECURE") {
        build.define("MI_SECURE", "4");
    }
    if is_musl {
        // Upstream cannot detect musl itself and expects the builder to set this.
        build.define("MI_LIBC_MUSL", "1");
    }
    if feature("NO_THP") && matches!(target_os.as_str(), "linux" | "android") {
        // This is what upstream's `MI_NO_THP=ON` turns into; the C-level `MI_NO_THP`
        // macro only skips `madvise` and leaves the runtime option enabled.
        build.define("MI_DEFAULT_ALLOW_THP", "0");
    }
    if feature("OVERRIDE") {
        // Replacing malloc can break a process silently, so only targets where the
        // override tests pass are accepted.
        let verified = matches!(
            (
                target_os.as_str(),
                target_env.as_str(),
                target_arch.as_str()
            ),
            ("macos", _, "aarch64") | ("linux", "gnu" | "musl", "x86_64" | "aarch64")
        );
        assert!(
            verified,
            "the `override` feature is not supported on {}",
            cfg("TARGET")
        );
        build
            .define("MI_MALLOC_OVERRIDE", None)
            .flag("-fno-builtin-malloc");
        if target_os == "macos" {
            // Interpose only takes effect in a shared library; the zone constructor does
            // the work in a static build. Both are kept to match upstream's defines.
            build
                .define("MI_OSX_ZONE", "1")
                .define("MI_OSX_INTERPOSE", "1");
        }
        // A single-pass linker (GNU ld) reaches this archive before std references
        // `malloc`, so a program that calls no `mi_*` function would silently keep libc's
        // allocator. This keeps the object in the link but does not order it; the
        // `extern crate std` in lib.rs keeps it before musl's `libc.a`.
        build.link_lib_modifier("+whole-archive");
    }

    // Upstream uses local-dynamic for static musl builds (issue #644).
    let tls_model = if feature("LOCAL_DYNAMIC_TLS") || is_musl {
        "local-dynamic"
    } else {
        "initial-exec"
    };
    build.flag(format!("-ftls-model={tls_model}"));

    build.flag("-fvisibility=hidden");
    let is_clang = build.get_compiler().is_like_clang();
    if is_clang {
        build.flag("-mno-outline");
    }
    if target_arch == "aarch64" {
        match target_os.as_str() {
            // Every Apple Silicon CPU supports armv8.3-a, upstream's default on arm64 macOS.
            "macos" => {
                build.flag("-march=armv8.3-a");
                if is_clang {
                    build.flag("-Xclang").flag("-fno-split-cold-code");
                }
            }
            // Upstream's default `-march=armv8.1-a` would raise the CPU baseline above
            // Rust's ARMv8.0 Linux targets; outline atomics still pick LSE at runtime.
            "linux" => {
                build.flag("-moutline-atomics");
            }
            _ => {}
        }
    }

    build.compile("mimalloc");
}

fn cfg(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("{key} is set by Cargo"))
}

fn feature(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}
