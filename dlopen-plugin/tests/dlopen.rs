//! Loads the plugin with `dlopen` after other libraries have used up glibc's static TLS
//! surplus, as in a process that already loaded several initial-exec libraries. With the
//! default initial-exec model, mimalloc's thread locals would need that surplus and `dlopen`
//! would fail with "cannot allocate memory in static TLS block".

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn main() {
    glibc::main();
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn main() {
    println!("skipped: the static TLS surplus is specific to glibc");
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod glibc {
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::{env, fs, mem};

    const RTLD_NOW: c_int = 2;
    // Each copy of the hog library takes this much static TLS (`src/hog.c`), well below what
    // the plugin would need with initial-exec, so filling leaves no room for it.
    const HOG_TLS: usize = 16;
    // Far more than any static TLS surplus glibc reserves.
    const MAX_HOGS: usize = 64 * 1024 / HOG_TLS;

    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
    }

    fn load(path: &Path) -> Result<*mut c_void, String> {
        let path = CString::new(path.as_os_str().as_bytes()).expect("path without NUL");
        // SAFETY: `path` is a valid C string. The libraries loaded here have no constructors
        // beyond those of Rust's std and mimalloc.
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
        if !handle.is_null() {
            return Ok(handle);
        }
        // SAFETY: `dlerror` has no preconditions; after a failed `dlopen` it returns the
        // message for it.
        let message = unsafe { dlerror() };
        // SAFETY: the message is a valid C string until the next `dl*` call on this thread.
        let message = unsafe { CStr::from_ptr(message) };
        Err(message.to_string_lossy().into_owned())
    }

    pub(super) fn main() {
        let hog = Path::new(env!("OUT_DIR")).join("libhog.so");
        let copies = Path::new(env!("OUT_DIR")).join("hogs");
        // Cargo places a cdylib next to the test executable without a hash in its name.
        let exe = env::current_exe().expect("test executable path");
        let plugin = exe.with_file_name("libdlopen_plugin.so");

        // glibc treats each file as a separate library, so every copy takes its own TLS.
        let _ = fs::remove_dir_all(&copies);
        fs::create_dir_all(&copies).expect("creating the copy directory");
        let mut loaded = 0;
        loop {
            assert!(loaded < MAX_HOGS, "the static TLS surplus never ran out");
            let copy = copies.join(format!("libhog{loaded}.so"));
            fs::copy(&hog, &copy).expect("copying the hog library");
            match load(&copy) {
                Ok(_) => loaded += 1,
                Err(message) => {
                    assert!(message.contains("static TLS"), "{message}");
                    break;
                }
            }
        }
        println!("filled the static TLS surplus with {loaded} libraries of {HOG_TLS} bytes");

        let handle = load(&plugin).unwrap_or_else(|message| panic!("{message}"));
        // SAFETY: `handle` is a loaded library and the name is a valid C string.
        let symbol = unsafe { dlsym(handle, c"plugin_run".as_ptr()) };
        assert!(!symbol.is_null(), "plugin_run not found");
        // SAFETY: `plugin_run` is defined in src/lib.rs with this signature.
        let plugin_run: extern "C" fn() -> usize = unsafe { mem::transmute(symbol) };
        assert_eq!(plugin_run(), 1100);
        println!("loaded the plugin and allocated from it on two threads");

        fs::remove_dir_all(&copies).expect("removing the copy directory");
    }
}
