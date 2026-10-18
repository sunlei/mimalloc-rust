use mimalloc_rust_sys::{MI_MALLOC_VERSION, MI_STAT_VERSION, mi_stats_get, mi_stats_t, mi_version};

#[test]
fn compiled_library_matches_generated_bindings() {
    // Catches a submodule bump without regenerating the bindings.
    let version = unsafe { mi_version() };
    assert_eq!(u32::try_from(version), Ok(MI_MALLOC_VERSION));
}

#[test]
fn stats_get_accepts_header_initialized_like_mi_stats_init() {
    // `mi_stats_init` is `static inline`, so it is not bound: it zeroes the whole
    // struct and then sets the header fields.
    let mut stats: mi_stats_t = unsafe { core::mem::zeroed() };
    stats.size = size_of::<mi_stats_t>();
    stats.version = MI_STAT_VERSION as usize;
    assert!(unsafe { mi_stats_get(&mut stats) });
}

#[test]
fn no_thp_turns_off_the_allow_thp_default_on_linux() {
    use mimalloc_rust_sys::{mi_option_allow_thp, mi_option_get};
    let allow_thp = unsafe { mi_option_get(mi_option_allow_thp) };
    let expect_disabled = cfg!(all(
        feature = "no_thp",
        any(target_os = "linux", target_os = "android")
    ));
    assert_eq!(allow_thp == 0, expect_disabled, "allow_thp = {allow_thp}");
}
