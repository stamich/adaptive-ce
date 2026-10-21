//! Helpers for opaque handles and out-parameters (`docs/NATIVE-INTEROP.md`, rule 3).

use crate::last_error::fail;
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT, ACE_OK};

/// Moves `value` to the heap and returns the pointer the host keeps.
pub fn into_raw<T>(value: T) -> *mut T {
    Box::into_raw(Box::new(value))
}

/// Drops a handle created by [`into_raw`]; null is ignored.
///
/// # Safety
///
/// `handle` must be null or a pointer from [`into_raw`] that has not been released.
pub unsafe fn release<T>(handle: *mut T) {
    if !handle.is_null() {
        // SAFETY: non-null and, by the caller's contract, a live `Box` allocation.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Validates an out-parameter and sets it to null, so a failing call leaves null behind.
///
/// # Errors
///
/// Returns [`ACE_ERR_INVALID_ARGUMENT`] (with the last error set) when `out` is null.
///
/// # Safety
///
/// `out` must be null or valid for a write of one pointer.
pub unsafe fn clear_out<T>(out: *mut *mut T) -> Result<(), AceStatus> {
    if out.is_null() {
        return Err(fail(ACE_ERR_INVALID_ARGUMENT, "output pointer is null"));
    }
    // SAFETY: `out` is non-null and the caller guarantees it is writable.
    unsafe { out.write(std::ptr::null_mut()) };
    Ok(())
}

/// Stores `value` as a new handle in `out` and returns [`ACE_OK`].
///
/// # Safety
///
/// `out` must be valid for a write of one pointer; [`clear_out`] has already checked it.
pub unsafe fn publish<T>(out: *mut *mut T, value: T) -> AceStatus {
    // SAFETY: the caller guarantees `out` is writable.
    unsafe { out.write(into_raw(value)) };
    ACE_OK
}
