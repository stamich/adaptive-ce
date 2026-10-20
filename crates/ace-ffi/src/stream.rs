//! Streaming through host callbacks (`docs/FFI.md`, "Streaming").

use std::ffi::c_void;
use std::io::{self, Cursor, Read, Write};

use crate::args::handle_ref;
use crate::buffer::buffer_into_raw;
use crate::buffer::AceBuffer;
use crate::engine::AceEngine;
use crate::guard::guard;
use crate::handle::clear_out;
use crate::json::stats_json;
use crate::last_error::{fail, fail_with};
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT, ACE_ERR_IO, ACE_OK};

/// Host callback that supplies input: stores up to `capacity` bytes in `buf` and returns how many
/// (0 = end of input), or a negative value on failure.
pub type AceReadFn =
    Option<unsafe extern "C-unwind" fn(ctx: *mut c_void, buf: *mut u8, capacity: usize) -> i64>;

/// Host callback that receives output: consumes `len` bytes at `buf` and returns `ACE_OK`, or any
/// other status to abort.
pub type AceWriteFn =
    Option<unsafe extern "C-unwind" fn(ctx: *mut c_void, buf: *const u8, len: usize) -> AceStatus>;

/// Size of the pieces handed to a write callback.
const WRITE_CHUNK: usize = 64 * 1024;

/// Adapts a read callback to [`Read`], remembering why it failed.
struct CallbackReader {
    callback: unsafe extern "C-unwind" fn(*mut c_void, *mut u8, usize) -> i64,
    ctx: *mut c_void,
    failure: Option<String>,
}

impl Read for CallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // SAFETY: the host promised the callback and context stay valid for the call, and `buf` is
        // a live, writable slice of the capacity passed.
        let produced = unsafe { (self.callback)(self.ctx, buf.as_mut_ptr(), buf.len()) };
        match usize::try_from(produced) {
            Ok(count) if count <= buf.len() => Ok(count),
            Ok(count) => {
                let message = format!(
                    "read callback returned {count} for a capacity of {}",
                    buf.len()
                );
                self.failure = Some(message.clone());
                Err(io::Error::other(message))
            }
            Err(_) => {
                let message = format!("read callback failed with {produced}");
                self.failure = Some(message.clone());
                Err(io::Error::other(message))
            }
        }
    }
}

/// Adapts a write callback to [`Write`], remembering why it failed.
struct CallbackWriter {
    callback: unsafe extern "C-unwind" fn(*mut c_void, *const u8, usize) -> AceStatus,
    ctx: *mut c_void,
    failure: Option<String>,
}

impl Write for CallbackWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let piece = buf.get(..buf.len().min(WRITE_CHUNK)).unwrap_or(buf);
        // SAFETY: as for the read callback; `piece` is a live slice.
        let status = unsafe { (self.callback)(self.ctx, piece.as_ptr(), piece.len()) };
        if status == ACE_OK {
            Ok(piece.len())
        } else {
            let message = format!("write callback failed with status {status}");
            self.failure = Some(message.clone());
            Err(io::Error::other(message))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn callback_failure(
    reader: &CallbackReader,
    writer: Option<&CallbackWriter>,
    error: &ace::Error,
) -> AceStatus {
    let message = reader
        .failure
        .as_deref()
        .or_else(|| writer.and_then(|w| w.failure.as_deref()));
    match message {
        Some(message) => fail(ACE_ERR_IO, message),
        None => fail_with(error),
    }
}

/// Compresses exactly `size` bytes supplied by `read` and delivers the ACE file through `write`.
///
/// The engine must use a fixed block size. The compressed file is assembled in memory (the header
/// is completed at the end) and passed to `write` in pieces of at most 64 KiB once the run has
/// finished. `stats_json` may be null; it receives only `input_bytes`, `output_bytes`,
/// `format_minor` and `blocks`, the other counters being 0.
///
/// # Safety
///
/// `engine` must be live; `read` and `write` must be valid function pointers for the duration of
/// the call with `read_ctx` / `write_ctx` valid arguments for them; `stats_json` null or writable.
#[no_mangle]
pub unsafe extern "C" fn ace_compress_stream(
    engine: *const AceEngine,
    size: u64,
    read: AceReadFn,
    read_ctx: *mut c_void,
    write: AceWriteFn,
    write_ctx: *mut c_void,
    stats_json_out: *mut *mut AceBuffer,
) -> AceStatus {
    guard(|| {
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
        let (Some(read), Some(write)) = (read, write) else {
            return fail(
                ACE_ERR_INVALID_ARGUMENT,
                "read and write callbacks must not be null",
            );
        };
        let mut reader = CallbackReader {
            callback: read,
            ctx: read_ctx,
            failure: None,
        };
        let mut sink = Cursor::new(Vec::new());
        let stats = match engine.engine.compress_stream(&mut reader, &mut sink, size) {
            Ok(stats) => stats,
            Err(error) => return callback_failure(&reader, None, &error),
        };
        let mut writer = CallbackWriter {
            callback: write,
            ctx: write_ctx,
            failure: None,
        };
        if let Err(error) = writer.write_all(sink.get_ref()) {
            let message = writer.failure.unwrap_or_else(|| error.to_string());
            return fail(ACE_ERR_IO, &message);
        }
        if !stats_json_out.is_null() {
            // SAFETY: validated above.
            unsafe { stats_json_out.write(buffer_into_raw(stats_json(&stats).into_bytes())) };
        }
        ACE_OK
    })
}

/// Decompresses an ACE file supplied by `read` and delivers the original bytes through `write`,
/// holding one block at a time.
///
/// # Safety
///
/// As [`ace_compress_stream`].
#[no_mangle]
pub unsafe extern "C" fn ace_decompress_stream(
    engine: *const AceEngine,
    read: AceReadFn,
    read_ctx: *mut c_void,
    write: AceWriteFn,
    write_ctx: *mut c_void,
) -> AceStatus {
    guard(|| {
        // SAFETY: live by the caller's contract.
        let engine = match unsafe { handle_ref(engine, "engine") } {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        let (Some(read), Some(write)) = (read, write) else {
            return fail(
                ACE_ERR_INVALID_ARGUMENT,
                "read and write callbacks must not be null",
            );
        };
        let mut reader = CallbackReader {
            callback: read,
            ctx: read_ctx,
            failure: None,
        };
        let mut writer = CallbackWriter {
            callback: write,
            ctx: write_ctx,
            failure: None,
        };
        match engine.engine.decompress_from(&mut reader, &mut writer) {
            Ok(()) => ACE_OK,
            Err(error) => callback_failure(&reader, Some(&writer), &error),
        }
    })
}
