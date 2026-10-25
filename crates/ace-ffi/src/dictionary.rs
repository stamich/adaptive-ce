//! `AceDictionary`: a preset dictionary for compression, ABI 1.1 (`docs/DICTIONARIES.md`).
//!
//! A dictionary is immutable once created, so any number of threads may read the same handle.
//! It is passed to [`ace_config`](crate::AceConfig) (the dictionary a writer uses) or to
//! [`ace_engine_new_ex`](crate::ace_engine_new_ex) (the external dictionaries a reader
//! resolves).

use ace::Dictionary;

use crate::args::{handle_ref, input_bytes};
use crate::guard::{guard, guard_value};
use crate::handle::{clear_out, publish, release};
use crate::last_error::{fail, fail_with};
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT};

/// A preset dictionary: LZ history that precedes every block it is used for.
///
/// The type is opaque in C. Create it with [`ace_dictionary_new`] or [`ace_dictionary_train`] and
/// release it with [`ace_dictionary_free`].
pub struct AceDictionary {
    pub(crate) dictionary: Dictionary,
}

/// Creates a dictionary from `len` bytes (1 to 65 535) at `bytes`.
///
/// `id` names the dictionary inside files; 0 derives it from the content, which is what you want
/// unless another system already assigns ids. On failure `*out` is null.
///
/// # Safety
///
/// `bytes` must be valid for reads of `len` bytes; `out` valid for a pointer write.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_new(
    bytes: *const u8,
    len: usize,
    id: u64,
    out: *mut *mut AceDictionary,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        // SAFETY: readable by the caller's contract.
        let bytes = match unsafe { input_bytes(bytes, len) } {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let created = if id == 0 {
            Dictionary::new(bytes)
        } else {
            Dictionary::with_id(id, bytes)
        };
        match created {
            // SAFETY: `out` was validated above.
            Ok(dictionary) => unsafe { publish(out, AceDictionary { dictionary }) },
            Err(error) => fail_with(&error),
        }
    })
}

/// Trains a dictionary of about `size` bytes (64 to 65 535) from `count` sample inputs.
///
/// `samples[i]` points to `lengths[i]` bytes. At least two samples are needed, and 100 to 1 000
/// typical inputs of one data source are usually enough. The result depends only on the samples,
/// in order, and `size`: the same samples give the same dictionary on every machine. On failure
/// `*out` is null.
///
/// # Safety
///
/// `samples` and `lengths` must be valid for reads of `count` elements, and every `samples[i]`
/// for reads of `lengths[i]` bytes; `out` valid for a pointer write.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_train(
    samples: *const *const u8,
    lengths: *const usize,
    count: usize,
    size: usize,
    out: *mut *mut AceDictionary,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        if count > 0 && (samples.is_null() || lengths.is_null()) {
            return fail(ACE_ERR_INVALID_ARGUMENT, "sample arrays are null");
        }
        if count > isize::MAX as usize / std::mem::size_of::<usize>() {
            return fail(ACE_ERR_INVALID_ARGUMENT, "sample count is too large");
        }
        let mut borrowed: Vec<&[u8]> = Vec::new();
        if borrowed.try_reserve_exact(count).is_err() {
            return fail(ACE_ERR_INVALID_ARGUMENT, "sample count is too large");
        }
        for index in 0..count {
            // SAFETY: `index < count` and both arrays hold `count` elements by the contract.
            let (pointer, length) = unsafe { (samples.add(index).read(), lengths.add(index).read()) };
            // SAFETY: `pointer` is valid for `length` bytes by the contract.
            match unsafe { input_bytes(pointer, length) } {
                Ok(sample) => borrowed.push(sample),
                Err(status) => return status,
            }
        }
        match Dictionary::train(&borrowed, size) {
            // SAFETY: `out` was validated above.
            Ok(dictionary) => unsafe { publish(out, AceDictionary { dictionary }) },
            Err(error) => fail_with(&error),
        }
    })
}

/// Returns the identifier files use for this dictionary, or 0 when `dictionary` is null.
///
/// # Safety
///
/// `dictionary` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_id(dictionary: *const AceDictionary) -> u64 {
    guard_value(0, || {
        // SAFETY: null or live by the caller's contract.
        unsafe { dictionary.as_ref() }.map_or(0, |handle| handle.dictionary.id())
    })
}

/// Returns a pointer to the dictionary bytes, valid until [`ace_dictionary_free`], or null when
/// `dictionary` is null. Store them to hand the dictionary to a reader later.
///
/// # Safety
///
/// `dictionary` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_data(dictionary: *const AceDictionary) -> *const u8 {
    guard_value(std::ptr::null(), || {
        // SAFETY: null or live by the caller's contract.
        unsafe { dictionary.as_ref() }.map_or(std::ptr::null(), |handle| handle.dictionary.as_bytes().as_ptr())
    })
}

/// Returns the number of dictionary bytes, or 0 when `dictionary` is null.
///
/// # Safety
///
/// `dictionary` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_len(dictionary: *const AceDictionary) -> usize {
    guard_value(0, || {
        // SAFETY: null or live by the caller's contract.
        unsafe { dictionary.as_ref() }.map_or(0, |handle| handle.dictionary.len())
    })
}

/// Releases a dictionary. Accepts null. Engines created with it keep their own copy, so it may
/// be released right after [`ace_engine_new`](crate::ace_engine_new) returns.
///
/// # Safety
///
/// `dictionary` must be null or a live handle, not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ace_dictionary_free(dictionary: *mut AceDictionary) {
    guard_value((), || {
        // SAFETY: null or live and not used again by the caller's contract.
        unsafe { release(dictionary) };
    });
}

/// Borrows the dictionaries of an array of handles.
///
/// # Errors
///
/// [`ACE_ERR_INVALID_ARGUMENT`] for a null array with a non-zero count or a null element.
///
/// # Safety
///
/// `handles` must be valid for reads of `count` pointers, each null or live.
pub(crate) unsafe fn collect(
    handles: *const *const AceDictionary,
    count: usize,
) -> Result<Vec<Dictionary>, AceStatus> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if handles.is_null() {
        return Err(fail(ACE_ERR_INVALID_ARGUMENT, "dictionary array is null"));
    }
    let mut dictionaries = Vec::new();
    if dictionaries.try_reserve_exact(count).is_err() {
        return Err(fail(ACE_ERR_INVALID_ARGUMENT, "dictionary count is too large"));
    }
    for index in 0..count {
        // SAFETY: `index < count`; the array holds `count` pointers by the contract.
        let pointer = unsafe { handles.add(index).read() };
        // SAFETY: live by the contract.
        let handle = unsafe { handle_ref(pointer, "dictionary") }?;
        dictionaries.push(handle.dictionary.clone());
    }
    Ok(dictionaries)
}
