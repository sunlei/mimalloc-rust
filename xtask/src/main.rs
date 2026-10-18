//! Maintenance tasks for the mimalloc bindings.
//!
//! - `bindgen`: regenerate `mimalloc-rust-sys/src/bindings.rs` from the vendored headers.
//! - `bindgen --check`: regenerate in memory and fail if the committed file differs.
//! - `check-symbols`: build the static library and fail if any bound function has no
//!   defined symbol in it. This proves presence only, not signatures or visibility.
//!
//! Run the checks natively on every supported target: the headers include libc headers
//! (`assert.h`, `string.h`), so each target's own sysroot must be visible to libclang.
//! On musl, build with `RUSTFLAGS="-C target-feature=-crt-static"`: libclang is loaded with
//! `dlopen`, which a statically linked musl binary cannot do.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::{env, fs};

const BINDINGS: &str = "mimalloc-rust-sys/src/bindings.rs";
const INCLUDE_DIR: &str = "mimalloc-rust-sys/c_src/mimalloc/include";
const USAGE: &str = "usage: cargo run -p xtask -- bindgen [--check] | check-symbols";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["bindgen"] => bindgen(false),
        ["bindgen", "--check"] => bindgen(true),
        ["check-symbols"] => check_symbols(),
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

fn check_symbols() -> ExitCode {
    let root = workspace_root();
    let bindings = fs::read_to_string(root.join(BINDINGS)).expect("reading committed bindings");
    let declared: BTreeSet<&str> = bindings
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("pub fn "))
        .filter_map(|rest| rest.split('(').next())
        .collect();
    // Guards against a bindgen output format this scan no longer recognizes, which would
    // otherwise pass with nothing checked.
    assert!(
        !declared.is_empty(),
        "no `pub fn` declarations found in {BINDINGS}"
    );

    let library = build_static_library(&root);
    let defined = defined_symbols(&library);
    let missing: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|name| !defined.contains(*name))
        .collect();
    if missing.is_empty() {
        println!(
            "all {} bound functions are defined in {}",
            declared.len(),
            library.display()
        );
        return ExitCode::SUCCESS;
    }
    eprintln!("functions bound but not defined in {}:", library.display());
    for name in missing {
        eprintln!("  {name}");
    }
    ExitCode::FAILURE
}

/// Builds the sys crate and returns the path of the static library its build script produced.
fn build_static_library(root: &Path) -> PathBuf {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--package",
            "mimalloc-rust-sys",
            "--message-format=json",
        ])
        .stderr(Stdio::inherit())
        .output()
        .expect("running cargo build");
    assert!(output.status.success(), "cargo build failed");
    let stdout = String::from_utf8(output.stdout).expect("cargo emits UTF-8 JSON");
    let out_dir = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "build-script-executed")
        .filter(|message| {
            message["package_id"]
                .as_str()
                .is_some_and(|id| id.contains("mimalloc-rust-sys"))
        })
        .find_map(|message| message["out_dir"].as_str().map(PathBuf::from))
        .expect("cargo reported the mimalloc-rust-sys build script output directory");
    out_dir.join("libmimalloc.a")
}

fn defined_symbols(library: &Path) -> BTreeSet<String> {
    let output = Command::new("nm")
        .args(["-g", "-P"])
        .arg(library)
        .output()
        .expect("running nm");
    assert!(
        output.status.success(),
        "nm failed on {}",
        library.display()
    );
    // Mach-O prefixes C symbols with an underscore.
    let prefix = if cfg!(target_vendor = "apple") {
        "_"
    } else {
        ""
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (name, kind) = (fields.next()?, fields.next()?);
            (kind != "U" && kind.len() == 1).then_some(name)
        })
        .filter_map(|name| name.strip_prefix(prefix))
        .map(str::to_owned)
        .collect()
}

fn generate_bindings(include_dir: &Path) -> String {
    bindgen::Builder::default()
        .header_contents(
            "wrapper.h",
            "#include <mimalloc.h>\n#include <mimalloc-stats.h>\n",
        )
        .clang_arg(format!("-I{}", include_dir.display()))
        .allowlist_function("mi_.*")
        .allowlist_type("mi_.*")
        .allowlist_var("MI_.*")
        // Signedness differs between targets; the sys crate defines it per target.
        .blocklist_type("wchar_t")
        // Declared in mimalloc.h but not linkable: the first two have no definition in
        // the v3.5.3 sources, the last two only exist with MI_MALLOC_OVERRIDE.
        .blocklist_function("mi_collect_reduce|mi_stats_merge|mi_malloc_size|mi_malloc_usable_size")
        .use_core()
        // `char` is unsigned on Linux aarch64 and signed elsewhere; `c_char` keeps one
        // file valid for every target.
        .ctypes_prefix("core::ffi")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        // bindgen defaults to an older target that cannot emit edition 2024 code;
        // match the workspace MSRV instead.
        .rust_target(
            env!("CARGO_PKG_RUST_VERSION")
                .parse()
                .expect("valid workspace rust-version"),
        )
        .rust_edition(bindgen::RustEdition::Edition2024)
        .merge_extern_blocks(true)
        // Independent of whichever rustfmt happens to be installed, so output is reproducible.
        .formatter(bindgen::Formatter::Prettyplease)
        .generate()
        .expect("bindgen failed to parse the mimalloc headers")
        .to_string()
}
