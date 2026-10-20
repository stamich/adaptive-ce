//! `AceBuffer`: bytes owned by ACE (`docs/NATIVE-INTEROP.md`, rule 4).

use crate::handle;

/// Bytes produced by ACE, released with [`ace_buffer_free`].
///
/// The type is opaque in C; use [`ace_buffer_data`] and [`ace_buffer_len`].
pub struct AceBuffer {
    bytes: Vec<u8>,
}

/// Moves `bytes` into a new buffer handle owned by the caller.
#[must_use]
pub fn buffer_into_raw(bytes: Vec<u8>) -> *mut AceBuffer {
    handle::into_raw(AceBuffer { bytes })
}

/// Returns a pointer to the bytes of `buffer`.
///
/// The pointer is valid until [`ace_buffer_free`]. It is non-null and valid to pass to a copy of
/// zero bytes even when the buffer is empty. Returns null when `buffer` is null.
///
/// # Safety
///
/// `buffer` must be null or a handle returned by ACE that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn ace_buffer_data(buffer: *const AceBuffer) -> *const u8 {
    // SAFETY: the caller guarantees `buffer` is null or a live handle.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |b| b.bytes.as_ptr())
}

/// Returns the length of `buffer` in bytes, or 0 when `buffer` is null.
///
/// # Safety
///
/// `buffer` must be null or a handle returned by ACE that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn ace_buffer_len(buffer: *const AceBuffer) -> usize {
    // SAFETY: the caller guarantees `buffer` is null or a live handle.
    unsafe { buffer.as_ref() }.map_or(0, |b| b.bytes.len())
}

/// Releases a buffer. Accepts null and does nothing for it.
///
/// # Safety
///
/// `buffer` must be null or a handle returned by ACE, and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ace_buffer_free(buffer: *mut AceBuffer) {
    // SAFETY: the caller guarantees `buffer` is null or a live handle that is not used again.
    unsafe { handle::release(buffer) };
}
