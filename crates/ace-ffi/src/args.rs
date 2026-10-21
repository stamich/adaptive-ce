//! Validation of pointer arguments shared by the exported functions.

use crate::last_error::fail;
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT};

/// Borrows `len` bytes at `ptr` for the duration of a call.
///
/// A zero length is valid with a null pointer. A null pointer with a non-zero length, or a length
/// above `isize::MAX`, is an invalid argument.
///
/// # Errors
///
/// [`ACE_ERR_INVALID_ARGUMENT`] with the last error set.
///
/// # Safety
///
/// When `len` is non-zero, `ptr` must be valid for reads of `len` bytes for the lifetime `'a`.
pub unsafe fn input_bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], AceStatus> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(fail(ACE_ERR_INVALID_ARGUMENT, "input pointer is null"));
    }
    if isize::try_from(len).is_err() {
        return Err(fail(
            ACE_ERR_INVALID_ARGUMENT,
            "input length exceeds isize::MAX",
        ));
    }
    // SAFETY: non-null, length checked, validity for reads is the caller's contract.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Borrows a handle for the duration of a call.
///
/// # Errors
///
/// [`ACE_ERR_INVALID_ARGUMENT`] with the last error set when `handle` is null.
///
/// # Safety
///
/// `handle` must be null or a live handle of type `T` that is not released during the call.
pub unsafe fn handle_ref<'a, T>(handle: *const T, what: &str) -> Result<&'a T, AceStatus> {
    // SAFETY: the caller guarantees `handle` is null or live.
    match unsafe { handle.as_ref() } {
        Some(reference) => Ok(reference),
        None => Err(fail(ACE_ERR_INVALID_ARGUMENT, &format!("{what} is null"))),
    }
}
