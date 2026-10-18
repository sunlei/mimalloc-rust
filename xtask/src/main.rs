//! Maintenance tasks for the mimalloc bindings.
//!
//! - `bindgen`: regenerate `mimalloc-rust-sys/src/bindings.rs` from the vendored headers.
//! - `bindgen --check`: regenerate in memory and fail if the committed file differs.
//!
//! Run the check natively on every supported target: the headers include libc headers
//! (`assert.h`, `string.h`), so each target's own sysroot must be visible to libclang.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

const BINDINGS: &str = "mimalloc-rust-sys/src/bindings.rs";
const INCLUDE_DIR: &str = "mimalloc-rust-sys/c_src/mimalloc/include";
const USAGE: &str = "usage: cargo run -p xtask -- bindgen [--check]";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["bindgen"] => bindgen(false),
        ["bindgen", "--check"] => bindgen(true),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

fn bindgen(check: bool) -> ExitCode {
    let root = workspace_root();
    let generated = generate_bindings(&root.join(INCLUDE_DIR));
    let path = root.join(BINDINGS);
    if !check {
        fs::write(&path, generated).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        println!("wrote {}", path.display());
        return ExitCode::SUCCESS;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    if committed == generated {
        println!("{} is up to date for this target", path.display());
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "{} differs from the bindings generated for this target; \
             run `cargo run -p xtask -- bindgen` and inspect the diff",
            path.display()
        );
        ExitCode::FAILURE
    }
}

fn generate_bindings(include_dir: &Path) -> String {
    bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <mimalloc.h>\n#include <mimalloc-stats.h>\n")
        .clang_arg(format!("-I{}", include_dir.display()))
        .allowlist_function("mi_.*")
        .allowlist_type("mi_.*")
        .allowlist_var("MI_.*")
        // Signedness differs between targets; the sys crate defines it per target.
        .blocklist_type("wchar_t")
        .use_core()
        // `char` is unsigned on Linux aarch64 and signed elsewhere; `c_char` keeps one
        // file valid for every target.
        .ctypes_prefix("core::ffi")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        // bindgen defaults to an older target that cannot emit edition 2024 code;
        // match the workspace MSRV instead.
        .rust_target(bindgen::RustTarget::stable(99, 0).expect("valid Rust version"))
        .rust_edition(bindgen::RustEdition::Edition2024)
        .merge_extern_blocks(true)
        // Independent of whichever rustfmt happens to be installed, so output is reproducible.
        .formatter(bindgen::Formatter::Prettyplease)
        .generate()
        .expect("bindgen failed to parse the mimalloc headers")
        .to_string()
}
