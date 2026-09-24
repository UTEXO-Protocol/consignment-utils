//! C ABI for language bindings.
//!
//! `rgb_consignment_parse` accepts a binary consignment and returns a
//! newly-allocated JSON document matching [`crate::ConsignmentInfo`].

use std::ffi::{c_char, CString};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;

/// Parse a binary RGB consignment and return a newly allocated JSON C string.
///
/// On success `*err_out` is set to null. On failure the return value is null
/// and `*err_out` is a newly allocated error message.
///
/// Both strings must be freed with [`rgb_consignment_string_free`].
///
/// # Safety
///
/// * `data` must be valid for `len` bytes, or `data` may be null when `len` is 0.
/// * `err_out` must be a valid writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rgb_consignment_parse(
    data: *const u8,
    len: usize,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    if err_out.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        *err_out = ptr::null_mut();
    }

    if data.is_null() && len != 0 {
        unsafe {
            *err_out = to_cstring("null data pointer");
        }
        return ptr::null_mut();
    }

    let bytes = if data.is_null() || len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };

    // A panic must never unwind across the C boundary: with `extern "C"` that
    // aborts the whole host process (e.g. a Go service). Turn it into an error.
    let result = panic::catch_unwind(AssertUnwindSafe(|| parse_to_json(bytes)));

    match result {
        Ok(Ok(json)) => to_cstring(&json),
        Ok(Err(msg)) => {
            unsafe {
                *err_out = to_cstring(&msg);
            }
            ptr::null_mut()
        }
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            unsafe {
                *err_out = to_cstring(&format!("internal error (panic): {msg}"));
            }
            ptr::null_mut()
        }
    }
}

fn parse_to_json(bytes: &[u8]) -> Result<String, String> {
    let info = crate::parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&info).map_err(|e| format!("serialize: {e}"))
}

/// Free a string returned by [`rgb_consignment_parse`].
///
/// # Safety
///
/// `s` must be null or a pointer previously returned by this crate's C ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rgb_consignment_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    drop(unsafe { CString::from_raw(s) });
}

fn to_cstring(s: &str) -> *mut c_char {
    CString::new(s.replace('\0', ""))
        .expect("NUL bytes already stripped")
        .into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::ptr;

    #[test]
    fn parse_garbage_sets_error() {
        let data = b"nope";
        let mut err = ptr::null_mut();
        let json = unsafe { rgb_consignment_parse(data.as_ptr(), data.len(), &mut err) };
        assert!(json.is_null());
        assert!(!err.is_null());
        let msg = unsafe { CStr::from_ptr(err) }.to_string_lossy();
        assert!(msg.contains("not a valid RGB consignment"), "{msg}");
        unsafe { rgb_consignment_string_free(err) };
    }

    #[test]
    fn null_err_out_returns_null() {
        let data = b"nope";
        let json = unsafe { rgb_consignment_parse(data.as_ptr(), data.len(), ptr::null_mut()) };
        assert!(json.is_null());
    }
}
