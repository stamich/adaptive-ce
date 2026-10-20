//! `AceRangeReader`: random access into one indexed ACE file.

use std::fs::File;
use std::io::Cursor;

use ace::{RangeReader, RangeStats};

use crate::args::{handle_ref, input_bytes};
use crate::buffer::{buffer_into_raw, AceBuffer};
use crate::engine::AceEngine;
use crate::handle::{clear_out, publish, release};
use crate::last_error::{fail, fail_with};
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT, ACE_OK};

enum Source {
    Memory(RangeReader<Cursor<Vec<u8>>>),
    File(RangeReader<File>),
}

/// An open indexed ACE file. Not thread-safe: it keeps a one-block cache.
pub struct AceRangeReader {
    source: Source,
    last: RangeStats,
}

impl AceRangeReader {
    fn len(&self) -> u64 {
        match &self.source {
            Source::Memory(reader) => reader.len(),
            Source::File(reader) => reader.len(),
        }
    }

    fn block_count(&self) -> usize {
        match &self.source {
            Source::Memory(reader) => reader.block_count(),
            Source::File(reader) => reader.block_count(),
        }
    }

    fn read_range(&mut self, start: u64, end: u64) -> ace::Result<Vec<u8>> {
        let (bytes, stats) = match &mut self.source {
            Source::Memory(reader) => reader.read_range_with_stats(start..end)?,
            Source::File(reader) => reader.read_range_with_stats(start..end)?,
        };
        self.last = stats;
        Ok(bytes)
    }

    fn read_block(&mut self, block: u64) -> ace::Result<Vec<u8>> {
        match &mut self.source {
            Source::Memory(reader) => reader.read_block(block),
            Source::File(reader) => reader.read_block(block),
        }
    }
}

/// Opens an indexed ACE file held in memory; the bytes are copied. Decode limits come from the
/// engine.
///
/// # Safety
///
/// `engine` must be live; `file` valid for reads of `len` bytes; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_open(
    engine: *const AceEngine,
    file: *const u8,
    len: usize,
    out: *mut *mut AceRangeReader,
) -> AceStatus {
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
    let bytes = match unsafe { input_bytes(file, len) } {
        Ok(bytes) => bytes.to_vec(),
        Err(status) => return status,
    };
    match engine.engine.open_range_reader(Cursor::new(bytes)) {
        // SAFETY: `out` was validated above.
        Ok(reader) => unsafe {
            publish(
                out,
                AceRangeReader {
                    source: Source::Memory(reader),
                    last: RangeStats::default(),
                },
            )
        },
        Err(error) => fail_with(&error),
    }
}

/// Opens an indexed ACE file on disk; blocks are read on demand. `path` is UTF-8, not
/// NUL-terminated.
///
/// # Safety
///
/// `engine` must be live; `path` valid for reads of `path_len` bytes; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_open_path(
    engine: *const AceEngine,
    path: *const u8,
    path_len: usize,
    out: *mut *mut AceRangeReader,
) -> AceStatus {
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
    let raw = match unsafe { input_bytes(path, path_len) } {
        Ok(raw) => raw,
        Err(status) => return status,
    };
    let Ok(path) = std::str::from_utf8(raw) else {
        return fail(ACE_ERR_INVALID_ARGUMENT, "path is not valid UTF-8");
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return fail_with(&ace::Error::from(error)),
    };
    match engine.engine.open_range_reader(file) {
        // SAFETY: `out` was validated above.
        Ok(reader) => unsafe {
            publish(
                out,
                AceRangeReader {
                    source: Source::File(reader),
                    last: RangeStats::default(),
                },
            )
        },
        Err(error) => fail_with(&error),
    }
}

/// Stores the original size of the file in `*out`.
///
/// # Safety
///
/// `reader` must be live; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_len(reader: *const AceRangeReader, out: *mut u64) -> AceStatus {
    if out.is_null() {
        return fail(ACE_ERR_INVALID_ARGUMENT, "output pointer is null");
    }
    // SAFETY: live by the caller's contract.
    match unsafe { handle_ref(reader, "reader") } {
        Ok(reader) => {
            // SAFETY: non-null and writable.
            unsafe { out.write(reader.len()) };
            ACE_OK
        }
        Err(status) => status,
    }
}

/// Stores the number of blocks in `*out`.
///
/// # Safety
///
/// `reader` must be live; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_block_count(
    reader: *const AceRangeReader,
    out: *mut u64,
) -> AceStatus {
    if out.is_null() {
        return fail(ACE_ERR_INVALID_ARGUMENT, "output pointer is null");
    }
    // SAFETY: live by the caller's contract.
    match unsafe { handle_ref(reader, "reader") } {
        Ok(reader) => {
            // SAFETY: non-null and writable.
            unsafe { out.write(reader.block_count() as u64) };
            ACE_OK
        }
        Err(status) => status,
    }
}

/// Reads the original bytes `[start, end)`.
///
/// A reversed range or one past the end is `ACE_ERR_INVALID_ARGUMENT`.
///
/// # Safety
///
/// `reader` must be live and not used by another thread during the call; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_read(
    reader: *mut AceRangeReader,
    start: u64,
    end: u64,
    out: *mut *mut AceBuffer,
) -> AceStatus {
    // SAFETY: `out` is writable by the caller's contract.
    if let Err(status) = unsafe { clear_out(out) } {
        return status;
    }
    // SAFETY: live and exclusively used by the caller's contract.
    let Some(reader) = (unsafe { reader.as_mut() }) else {
        return fail(ACE_ERR_INVALID_ARGUMENT, "reader is null");
    };
    match reader.read_range(start, end) {
        Ok(bytes) => {
            // SAFETY: validated above.
            unsafe { out.write(buffer_into_raw(bytes)) };
            ACE_OK
        }
        Err(error) => fail_with(&error),
    }
}

/// Reads one whole block by number.
///
/// # Safety
///
/// As [`ace_range_read`].
#[no_mangle]
pub unsafe extern "C" fn ace_range_read_block(
    reader: *mut AceRangeReader,
    block: u64,
    out: *mut *mut AceBuffer,
) -> AceStatus {
    // SAFETY: `out` is writable by the caller's contract.
    if let Err(status) = unsafe { clear_out(out) } {
        return status;
    }
    // SAFETY: live and exclusively used by the caller's contract.
    let Some(reader) = (unsafe { reader.as_mut() }) else {
        return fail(ACE_ERR_INVALID_ARGUMENT, "reader is null");
    };
    match reader.read_block(block) {
        Ok(bytes) => {
            // SAFETY: validated above.
            unsafe { out.write(buffer_into_raw(bytes)) };
            ACE_OK
        }
        Err(error) => fail_with(&error),
    }
}

/// Reports the work of the last [`ace_range_read`] on this reader: bytes asked for, compressed
/// bytes read, blocks decoded. Any output pointer may be null.
///
/// # Safety
///
/// `reader` must be live; each non-null output writable.
#[no_mangle]
pub unsafe extern "C" fn ace_range_last_stats(
    reader: *const AceRangeReader,
    logical_bytes: *mut u64,
    physical_bytes: *mut u64,
    blocks: *mut u64,
) -> AceStatus {
    // SAFETY: live by the caller's contract.
    let reader = match unsafe { handle_ref(reader, "reader") } {
        Ok(reader) => reader,
        Err(status) => return status,
    };
    // SAFETY: each pointer is checked for null before the write.
    unsafe {
        if !logical_bytes.is_null() {
            logical_bytes.write(reader.last.logical_bytes);
        }
        if !physical_bytes.is_null() {
            physical_bytes.write(reader.last.physical_bytes);
        }
        if !blocks.is_null() {
            blocks.write(reader.last.blocks as u64);
        }
    }
    ACE_OK
}

/// Releases a reader. Accepts null.
///
/// # Safety
///
/// `reader` must be null or a handle from an `ace_range_open*` function, not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ace_range_free(reader: *mut AceRangeReader) {
    // SAFETY: the caller guarantees `reader` is null or live and not used again.
    unsafe { release(reader) };
}
