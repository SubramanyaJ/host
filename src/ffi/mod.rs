/**
 * ffi/mod.rs
 * 
 * Foreign Function Interface for Flutter/Dart integration
 * Uses C-ABI for maximum compatibility
 */

mod types;
mod session;
mod nat_traversal;

pub use types::*;
pub use session::*;
pub use nat_traversal::*;

use std::os::raw::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::panic;

/// Initialize the library (call once at startup)
#[no_mangle]
pub extern "C" fn pineapple_init() -> i32 {
    // Set up panic hook to prevent unwinding into FFI boundary
    panic::set_hook(Box::new(|panic_info| {
        eprintln!("Pineapple panic: {:?}", panic_info);
    }));
    0
}

/// Get library version string
#[no_mangle]
pub extern "C" fn pineapple_version() -> *const c_char {
    let version = CString::new("1.0.0").unwrap();
    version.into_raw()
}

/// Free a string allocated by the library
#[no_mangle]
pub extern "C" fn pineapple_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Get last error message
static mut LAST_ERROR: Option<String> = None;

#[no_mangle]
pub extern "C" fn pineapple_last_error() -> *const c_char {
    unsafe {
        match &LAST_ERROR {
            Some(err) => {
                let c_str = CString::new(err.as_str()).unwrap();
                c_str.into_raw()
            }
            None => std::ptr::null(),
        }
    }
}

/// Set last error (internal helper)
pub(crate) fn set_last_error(error: &str) {
    unsafe {
        LAST_ERROR = Some(error.to_string());
    }
}

/// Clear last error
#[no_mangle]
pub extern "C" fn pineapple_clear_error() {
    unsafe {
        LAST_ERROR = None;
    }
}

/// Helper to convert C string to Rust string
pub(crate) fn c_str_to_rust(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }
}
