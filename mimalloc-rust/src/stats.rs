use core::ffi::{CStr, c_char};
use core::fmt;
use core::ptr::{self, NonNull};

use mimalloc_rust_sys as ffi;

/// mimalloc statistics rendered as JSON, owning the buffer mimalloc allocated for them.
pub struct StatsJson(NonNull<c_char>);

// SAFETY: the buffer is immutable after creation and `mi_free` accepts it from any thread.
unsafe impl Send for StatsJson {}
// SAFETY: as above; shared access only reads the string.
unsafe impl Sync for StatsJson {}

impl AsRef<CStr> for StatsJson {
    fn as_ref(&self) -> &CStr {
        // SAFETY: mimalloc returns a NUL-terminated string that lives until `drop`.
        unsafe { CStr::from_ptr(self.0.as_ptr()) }
    }
}

impl fmt::Debug for StatsJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StatsJson").field(&self.as_ref()).finish()
    }
}

impl Drop for StatsJson {
    fn drop(&mut self) {
        // SAFETY: mimalloc allocated the buffer because we passed none, and documents
        // releasing it with `mi_free`; `self` owns it exclusively.
        unsafe { ffi::mi_free(self.0.as_ptr().cast()) }
    }
}

/// Returns mimalloc's statistics for the current subprocess as JSON.
///
/// `None` means mimalloc could not produce the output; it reports no further reason.
pub fn stats_json() -> Option<StatsJson> {
    // SAFETY: with a null buffer mimalloc allocates the result itself.
    let json = unsafe { ffi::mi_stats_get_json(0, ptr::null_mut()) };
    NonNull::new(json).map(StatsJson)
}
