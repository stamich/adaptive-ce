//! Bodies of the `ffi_decode` and `ffi_handles` fuzz targets.
//!
//! They live here, not in `fuzz/`, so that `tests/fuzzing.rs` can run them on generated inputs in
//! every `cargo test` and the fuzz targets stay one-line wrappers. They drive the library only
//! through its exported C functions, exactly as a host would.
//!
//! The hardening invariant: whatever the input, no call returns [`ACE_ERR_PANIC`] or
//! [`ACE_ERR_INTERNAL`], a failing call leaves its output handle null, and every handle that was
//! created is released.

use std::ptr;

use crate::buffer::{ace_buffer_data, ace_buffer_free, ace_buffer_len, AceBuffer};
use crate::config::{ace_config_default, ace_limits_default, AceConfig, AceLimits};
use crate::engine::{
    ace_compress, ace_decompress, ace_decompress_into, ace_engine_free, ace_engine_new, AceEngine,
};
use crate::range::{
    ace_range_block_count, ace_range_free, ace_range_len, ace_range_open, ace_range_read,
    ace_range_read_block, AceRangeReader,
};
use crate::status::{AceStatus, ACE_ERR_INTERNAL, ACE_ERR_PANIC, ACE_OK};

/// Largest input a fuzz run hands to the compressor, to keep one run fast.
const MAX_COMPRESS: usize = 4096;

/// Output limit of the decoding engine, so hostile headers cannot make one run slow.
const MAX_OUTPUT: u64 = 1 << 22;

/// Panics (so the fuzzer reports a crash) when `status` shows a contained bug.
fn check(status: AceStatus) -> AceStatus {
    assert!(
        status != ACE_ERR_PANIC && status != ACE_ERR_INTERNAL,
        "the C ABI reported a bug: status {status}"
    );
    status
}

/// Copies a buffer's bytes out and releases it.
fn consume(buffer: *mut AceBuffer) -> Vec<u8> {
    // SAFETY: `buffer` is a live handle returned by the library; it is released exactly once here.
    unsafe {
        let bytes =
            std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer)).to_vec();
        ace_buffer_free(buffer);
        bytes
    }
}

fn default_config() -> AceConfig {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    }
}

fn new_engine(
    config: Option<&AceConfig>,
    limits: Option<&AceLimits>,
) -> (AceStatus, *mut AceEngine) {
    let mut handle = ptr::null_mut();
    let config = config.map_or(ptr::null(), ptr::from_ref);
    let limits = limits.map_or(ptr::null(), ptr::from_ref);
    // SAFETY: the pointers are null or reference live structs; `handle` is writable.
    let status = check(unsafe { ace_engine_new(config, limits, &mut handle) });
    assert_eq!(
        status == ACE_OK,
        !handle.is_null(),
        "engine handle and status disagree"
    );
    (status, handle)
}

/// Garbage through `ace_decompress`, `ace_decompress_into` and the range functions.
pub fn decode(data: &[u8]) {
    let mut limits = std::mem::MaybeUninit::<AceLimits>::uninit();
    // SAFETY: `ace_limits_default` initialises the whole struct.
    let mut limits = unsafe {
        ace_limits_default(limits.as_mut_ptr());
        limits.assume_init()
    };
    limits.max_output_size = MAX_OUTPUT;
    let (status, engine) = new_engine(None, Some(&limits));
    assert_eq!(status, ACE_OK);

    let mut out = ptr::null_mut();
    // SAFETY: live engine; `data` is a live slice; `out` is writable.
    let status = check(unsafe { ace_decompress(engine, data.as_ptr(), data.len(), &mut out) });
    assert_eq!(status == ACE_OK, !out.is_null());
    if !out.is_null() {
        drop(consume(out));
    }

    let capacity = usize::from(data.first().copied().unwrap_or(0));
    let mut buffer = vec![0_u8; capacity];
    let mut written = 0_usize;
    // SAFETY: as above; `buffer` has `capacity` writable bytes and `written` is writable.
    check(unsafe {
        ace_decompress_into(
            engine,
            data.as_ptr(),
            data.len(),
            buffer.as_mut_ptr(),
            capacity,
            &mut written,
        )
    });

    let mut reader = ptr::null_mut();
    // SAFETY: live engine and slice; `reader` is writable.
    let status = check(unsafe { ace_range_open(engine, data.as_ptr(), data.len(), &mut reader) });
    assert_eq!(status == ACE_OK, !reader.is_null());
    if !reader.is_null() {
        read_ranges(reader, data);
        // SAFETY: released once.
        unsafe { ace_range_free(reader) };
    }
    // SAFETY: released once.
    unsafe { ace_engine_free(engine) };
}

fn read_ranges(reader: *mut AceRangeReader, data: &[u8]) {
    let (mut length, mut blocks) = (0_u64, 0_u64);
    // SAFETY: live reader; both outputs are writable.
    unsafe {
        assert_eq!(check(ace_range_len(reader, &mut length)), ACE_OK);
        assert_eq!(check(ace_range_block_count(reader, &mut blocks)), ACE_OK);
    }
    let mut seed = data.iter().fold(0_u64, |acc, &byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(byte))
    });
    for _ in 0..4 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let (a, b) = (seed >> 33, seed >> 11 & 0xFFFF_FFFF);
        let (start, end) = if length == 0 {
            (a, b)
        } else {
            (a % (length + 1), b % (length + 1))
        };
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let mut out = ptr::null_mut();
        // SAFETY: live reader; `out` is writable.
        let status = check(unsafe { ace_range_read(reader, start, end, &mut out) });
        assert_eq!(status == ACE_OK, !out.is_null());
        if !out.is_null() {
            drop(consume(out));
        }
        let mut block = ptr::null_mut();
        // SAFETY: as above.
        let status =
            check(unsafe { ace_range_read_block(reader, seed % (blocks + 1), &mut block) });
        assert_eq!(status == ACE_OK, !block.is_null());
        if !block.is_null() {
            drop(consume(block));
        }
    }
}

/// Random sequences of valid and invalid calls over engines, buffers and readers.
pub fn handles(data: &[u8]) {
    let mut engines: Vec<*mut AceEngine> = Vec::new();
    let mut files: Vec<Vec<u8>> = Vec::new();
    let mut readers: Vec<*mut AceRangeReader> = Vec::new();
    let mut cursor = data.iter().copied();
    let mut next = move || cursor.next().unwrap_or(0);

    for _ in 0..data.len().min(64) {
        match next() % 8 {
            0 => {
                let mut config = default_config();
                config.profile = u32::from(next() % 4);
                config.block_size = [0, 1024, 4096, 65_536, 3][usize::from(next()) % 5];
                config.threads = u64::from(next() % 3);
                config.write_index = next() % 4;
                if next() % 16 == 0 {
                    config.reserved[usize::from(next()) % 5] = 1;
                }
                if next() % 16 == 0 {
                    config.struct_size = usize::from(next());
                }
                let (status, handle) = new_engine(Some(&config), None);
                if status == ACE_OK {
                    engines.push(handle);
                }
            }
            1 => {
                let Some(&engine) = engines.last() else {
                    continue;
                };
                let length = usize::from(next()) * 16 % MAX_COMPRESS;
                let input: Vec<u8> = (0..length)
                    .map(|i| (i as u8).wrapping_mul(next() | 1))
                    .collect();
                let mut out = ptr::null_mut();
                // SAFETY: live engine and slice; `out` is writable.
                let status = check(unsafe {
                    ace_compress(
                        engine,
                        input.as_ptr(),
                        input.len(),
                        &mut out,
                        ptr::null_mut(),
                    )
                });
                assert_eq!(status == ACE_OK, !out.is_null());
                if !out.is_null() {
                    files.push(consume(out));
                }
            }
            2 => {
                let (Some(&engine), Some(file)) = (engines.first(), files.last()) else {
                    continue;
                };
                let mut out = ptr::null_mut();
                // SAFETY: live engine and slice; `out` is writable.
                let status =
                    check(unsafe { ace_decompress(engine, file.as_ptr(), file.len(), &mut out) });
                assert_eq!(status, ACE_OK, "a file written by ACE decodes");
                drop(consume(out));
            }
            3 => {
                let (Some(&engine), Some(file)) = (engines.first(), files.last()) else {
                    continue;
                };
                let mut reader = ptr::null_mut();
                // SAFETY: live engine and slice; `reader` is writable.
                let status = check(unsafe {
                    ace_range_open(engine, file.as_ptr(), file.len(), &mut reader)
                });
                assert_eq!(status == ACE_OK, !reader.is_null());
                if status == ACE_OK {
                    readers.push(reader);
                }
            }
            4 => {
                let Some(&reader) = readers.last() else {
                    continue;
                };
                read_ranges(reader, &[next(), next()]);
            }
            5 => {
                if let Some(reader) = readers.pop() {
                    // SAFETY: released once; removed from the list.
                    unsafe { ace_range_free(reader) };
                }
            }
            6 => {
                if !engines.is_empty() {
                    let index = usize::from(next()) % engines.len();
                    // SAFETY: released once; removed from the list.
                    unsafe { ace_engine_free(engines.swap_remove(index)) };
                }
            }
            _ => {
                // Invalid arguments are rejected, never dereferenced.
                let mut out = ptr::null_mut();
                // SAFETY: a null engine and a null/oversized input are rejected before any use.
                unsafe {
                    check(ace_decompress(
                        ptr::null(),
                        ptr::null(),
                        usize::from(next()),
                        &mut out,
                    ));
                    assert!(out.is_null());
                    check(ace_decompress(ptr::null(), ptr::null(), 0, ptr::null_mut()));
                }
            }
        }
    }
    for reader in readers {
        // SAFETY: each is released once.
        unsafe { ace_range_free(reader) };
    }
    for engine in engines {
        // SAFETY: each is released once.
        unsafe { ace_engine_free(engine) };
    }
}
