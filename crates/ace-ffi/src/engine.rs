//! `AceEngine` and the one-shot functions.

use ace::Engine;

use crate::args::{handle_ref, input_bytes};
use crate::buffer::{buffer_into_raw, AceBuffer};
use crate::config::{config_from_raw, limits_from_raw, AceConfig, AceLimits};
use crate::dictionary::{collect, AceDictionary};
use crate::guard::{guard, guard_value};
use crate::handle::{clear_out, publish, release};
use crate::json::{explain_json, stats_json};
use crate::last_error::{fail, fail_with};
use crate::status::{AceStatus, ACE_ERR_BUFFER_TOO_SMALL, ACE_ERR_INVALID_ARGUMENT, ACE_OK};

/// A compression engine. Immutable after creation, so any number of threads may share it.
pub struct AceEngine {
    pub(crate) engine: Engine,
}

/// Creates an engine.
///
/// `config` and `limits` may be null for the defaults. On failure `*out` is null. Equivalent to
/// [`ace_engine_new_ex`] without external dictionaries.
///
/// # Safety
///
/// `config` and `limits` must be null or point to readable structs of at least their `struct_size`
/// bytes; `out` must be valid for a write of one pointer.
#[no_mangle]
pub unsafe extern "C" fn ace_engine_new(
    config: *const AceConfig,
    limits: *const AceLimits,
    out: *mut *mut AceEngine,
) -> AceStatus {
    guard(|| {
        // SAFETY: the contract of this function is the contract of `ace_engine_new_ex` with no
        // dictionaries.
        unsafe { ace_engine_new_ex(config, limits, std::ptr::null(), 0, out) }
    })
}

/// Creates an engine that also knows external dictionaries (ABI 1.1).
///
/// A file written with `ACE_DICTIONARY_EXTERNAL` names its dictionary by id only; reading it
/// needs a dictionary with that id, which this engine looks up among the `count` handles in
/// `dictionaries`. The dictionary of `config` is always available to its own engine. Handles are
/// copied, so they may be released after the call. Applies to every reading function, including
/// the range readers opened from the engine.
///
/// `config`, `limits` and (with `count` 0) `dictionaries` may be null. On failure `*out` is null.
///
/// # Safety
///
/// `config` and `limits` must be null or point to readable structs of at least their
/// `struct_size` bytes; `dictionaries` must be valid for reads of `count` handles, each live;
/// `out` must be valid for a write of one pointer.
#[no_mangle]
pub unsafe extern "C" fn ace_engine_new_ex(
    config: *const AceConfig,
    limits: *const AceLimits,
    dictionaries: *const *const AceDictionary,
    count: usize,
    out: *mut *mut AceEngine,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        // SAFETY: readable by the caller's contract.
        let config = match unsafe { config_from_raw(config) } {
            Ok(config) => config,
            Err(status) => return status,
        };
        // SAFETY: readable by the caller's contract.
        let limits = match unsafe { limits_from_raw(limits) } {
            Ok(limits) => limits,
            Err(status) => return status,
        };
        // SAFETY: valid and live by the caller's contract.
        let dictionaries = match unsafe { collect(dictionaries, count) } {
            Ok(dictionaries) => dictionaries,
            Err(status) => return status,
        };
        match Engine::new(config) {
            // SAFETY: `out` was validated above.
            Ok(engine) => unsafe {
                publish(
                    out,
                    AceEngine {
                        engine: engine
                            .with_decode_limits(limits)
                            .with_dictionaries(dictionaries),
                    },
                )
            },
            Err(error) => fail_with(&error),
        }
    })
}

/// Releases an engine. Accepts null.
///
/// # Safety
///
/// `engine` must be null or a handle from [`ace_engine_new`], not used afterwards, and no call
/// using it may be running on another thread.
#[no_mangle]
pub unsafe extern "C" fn ace_engine_free(engine: *mut AceEngine) {
    guard_value((), || {
        // SAFETY: the caller guarantees `engine` is null or live and not used again.
        unsafe { release(engine) };
    });
}

/// Compresses `len` bytes at `input` into a new buffer.
///
/// `stats_json` may be null; otherwise it receives the Stats JSON. On failure both outputs are
/// null.
///
/// # Safety
///
/// `engine` must be a live engine; `input` valid for reads of `len` bytes (null with length 0 is
/// allowed); `out` and `stats_json` null (only `stats_json`) or valid for a pointer write.
#[no_mangle]
pub unsafe extern "C" fn ace_compress(
    engine: *const AceEngine,
    input: *const u8,
    len: usize,
    out: *mut *mut AceBuffer,
    stats_json_out: *mut *mut AceBuffer,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        if !stats_json_out.is_null() {
            // SAFETY: non-null and writable by the caller's contract.
            if let Err(status) = unsafe { clear_out(stats_json_out) } {
                return status;
            }
        }
        // SAFETY: live by the caller's contract.
        let engine = match unsafe { handle_ref(engine, "engine") } {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        // SAFETY: readable by the caller's contract.
        let input = match unsafe { input_bytes(input, len) } {
            Ok(input) => input,
            Err(status) => return status,
        };
        match engine.engine.compress_with_stats(input) {
            Ok((bytes, stats)) => {
                if !stats_json_out.is_null() {
                    // SAFETY: validated above.
                    unsafe {
                        stats_json_out.write(buffer_into_raw(stats_json(&stats).into_bytes()))
                    };
                }
                // SAFETY: validated above.
                unsafe { out.write(buffer_into_raw(bytes)) };
                ACE_OK
            }
            Err(error) => fail_with(&error),
        }
    })
}

/// Decompresses a complete ACE file into a new buffer, under the engine's decode limits.
///
/// # Safety
///
/// `engine` must be a live engine; `input` valid for reads of `len` bytes; `out` valid for a
/// pointer write.
#[no_mangle]
pub unsafe extern "C" fn ace_decompress(
    engine: *const AceEngine,
    input: *const u8,
    len: usize,
    out: *mut *mut AceBuffer,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        // SAFETY: live by the caller's contract.
        let engine = match unsafe { handle_ref(engine, "engine") } {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        // SAFETY: readable by the caller's contract.
        let input = match unsafe { input_bytes(input, len) } {
            Ok(input) => input,
            Err(status) => return status,
        };
        match engine.engine.decompress(input) {
            Ok(bytes) => {
                // SAFETY: validated above.
                unsafe { out.write(buffer_into_raw(bytes)) };
                ACE_OK
            }
            Err(error) => fail_with(&error),
        }
    })
}

/// Decompresses into a caller-provided buffer.
///
/// `*written` always receives the decoded size on success. When `capacity` is too small the call
/// returns [`ACE_ERR_BUFFER_TOO_SMALL`] with the required size in `*written` and writes nothing to
/// `out`. The file is decoded under the engine's limits either way.
///
/// # Safety
///
/// `engine` must be a live engine; `input` valid for reads of `len` bytes; `out` valid for writes
/// of `capacity` bytes (null with capacity 0 is allowed); `written` valid for a `size_t` write.
#[no_mangle]
pub unsafe extern "C" fn ace_decompress_into(
    engine: *const AceEngine,
    input: *const u8,
    len: usize,
    out: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> AceStatus {
    guard(|| {
        if written.is_null() {
            return fail(ACE_ERR_INVALID_ARGUMENT, "written pointer is null");
        }
        // SAFETY: non-null and writable by the caller's contract.
        unsafe { written.write(0) };
        if capacity > 0 && out.is_null() {
            return fail(ACE_ERR_INVALID_ARGUMENT, "output pointer is null");
        }
        // SAFETY: live by the caller's contract.
        let engine = match unsafe { handle_ref(engine, "engine") } {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        // SAFETY: readable by the caller's contract.
        let input = match unsafe { input_bytes(input, len) } {
            Ok(input) => input,
            Err(status) => return status,
        };
        let bytes = match engine.engine.decompress(input) {
            Ok(bytes) => bytes,
            Err(error) => return fail_with(&error),
        };
        // SAFETY: non-null and writable.
        unsafe { written.write(bytes.len()) };
        if bytes.len() > capacity {
            return fail(
                ACE_ERR_BUFFER_TOO_SMALL,
                &format!(
                    "output buffer holds {capacity} bytes, {} are needed",
                    bytes.len()
                ),
            );
        }
        if !bytes.is_empty() {
            // SAFETY: `out` is valid for `capacity >= bytes.len()` bytes and cannot overlap `bytes`.
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
        }
        ACE_OK
    })
}

/// Reports, per block, which pipeline the planner selects and which it considered, as JSON.
///
/// Every candidate is evaluated, so this is much slower than compression. Nothing is written.
///
/// # Safety
///
/// `engine` must be a live engine; `input` valid for reads of `len` bytes; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_explain_json(
    engine: *const AceEngine,
    input: *const u8,
    len: usize,
    out: *mut *mut AceBuffer,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        // SAFETY: live by the caller's contract.
        let engine = match unsafe { handle_ref(engine, "engine") } {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        // SAFETY: readable by the caller's contract.
        let input = match unsafe { input_bytes(input, len) } {
            Ok(input) => input,
            Err(status) => return status,
        };
        match engine.engine.explain(input) {
            Ok(reports) => {
                // SAFETY: validated above.
                unsafe { out.write(buffer_into_raw(explain_json(&reports).into_bytes())) };
                ACE_OK
            }
            Err(error) => fail_with(&error),
        }
    })
}
