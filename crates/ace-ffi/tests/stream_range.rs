//! Streaming callbacks and range reads through the `extern "C"` API.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::ffi::c_void;
use std::ptr;

use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress_stream, ace_config_default,
    ace_decompress_stream, ace_engine_free, ace_engine_new, ace_range_block_count, ace_range_free,
    ace_range_last_stats, ace_range_len, ace_range_open, ace_range_open_path, ace_range_read,
    ace_range_read_block, AceBuffer, AceConfig, AceEngine, AceRangeReader, AceStatus,
    ACE_ERR_BLOCK_NOT_FOUND, ACE_ERR_INVALID_ARGUMENT, ACE_ERR_IO, ACE_OK,
};

const BLOCK: u64 = 64 * 1024;

fn sample() -> Vec<u8> {
    (0_u32..120_000)
        .flat_map(|i| (i / 5).to_le_bytes())
        .collect()
}

fn engine() -> *mut AceEngine {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    let mut config = unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    };
    config.block_size = BLOCK;
    let mut handle = ptr::null_mut();
    // SAFETY: valid config, writable output.
    let status = unsafe { ace_engine_new(&config, ptr::null(), &mut handle) };
    assert_eq!(status, ACE_OK);
    handle
}

fn take(buffer: *mut AceBuffer) -> Vec<u8> {
    // SAFETY: `buffer` is a live handle freed once after the copy.
    unsafe {
        let bytes =
            std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer)).to_vec();
        ace_buffer_free(buffer);
        bytes
    }
}

struct Source {
    data: Vec<u8>,
    position: usize,
    fail_after: Option<usize>,
}

unsafe extern "C" fn read_callback(ctx: *mut c_void, buf: *mut u8, capacity: usize) -> i64 {
    // SAFETY: `ctx` is the `Source` the test passed and outlives the call.
    let source = unsafe { &mut *ctx.cast::<Source>() };
    if source
        .fail_after
        .is_some_and(|limit| source.position >= limit)
    {
        return -1;
    }
    let count = capacity.min(source.data.len() - source.position).min(1000);
    // SAFETY: `buf` has room for `capacity >= count` bytes.
    unsafe { ptr::copy_nonoverlapping(source.data.as_ptr().add(source.position), buf, count) };
    source.position += count;
    count as i64
}

unsafe extern "C" fn write_callback(ctx: *mut c_void, buf: *const u8, len: usize) -> AceStatus {
    // SAFETY: `ctx` is the `Vec` the test passed; `buf` holds `len` bytes.
    unsafe { (*ctx.cast::<Vec<u8>>()).extend_from_slice(std::slice::from_raw_parts(buf, len)) };
    ACE_OK
}

unsafe extern "C" fn failing_write(_: *mut c_void, _: *const u8, _: usize) -> AceStatus {
    ACE_ERR_IO
}

fn compress_streaming(engine: *const AceEngine, data: &[u8]) -> (Vec<u8>, String) {
    let mut source = Source {
        data: data.to_vec(),
        position: 0,
        fail_after: None,
    };
    let mut sink: Vec<u8> = Vec::new();
    let mut stats = ptr::null_mut();
    // SAFETY: live engine, valid callbacks and contexts that outlive the call.
    let status = unsafe {
        ace_compress_stream(
            engine,
            data.len() as u64,
            Some(read_callback),
            ptr::from_mut(&mut source).cast(),
            Some(write_callback),
            ptr::from_mut(&mut sink).cast(),
            &mut stats,
        )
    };
    assert_eq!(status, ACE_OK);
    (sink, String::from_utf8(take(stats)).unwrap())
}

#[test]
fn streaming_compress_equals_one_shot() {
    let data = sample();
    let handle = engine();
    let (streamed, stats) = compress_streaming(handle, &data);
    let reference = ace::Engine::new(
        ace::Config::builder()
            .block_size(BLOCK as usize)
            .build()
            .unwrap(),
    )
    .unwrap()
    .compress(&data)
    .unwrap();
    assert_eq!(streamed, reference);
    assert!(stats.contains(&format!("\"input_bytes\":{}", data.len())));
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn streaming_decompress_round_trips() {
    let data = sample();
    let handle = engine();
    let (packed, _) = compress_streaming(handle, &data);
    let mut source = Source {
        data: packed,
        position: 0,
        fail_after: None,
    };
    let mut sink: Vec<u8> = Vec::new();
    // SAFETY: live engine, valid callbacks and contexts that outlive the call.
    let status = unsafe {
        ace_decompress_stream(
            handle,
            Some(read_callback),
            ptr::from_mut(&mut source).cast(),
            Some(write_callback),
            ptr::from_mut(&mut sink).cast(),
        )
    };
    assert_eq!(status, ACE_OK);
    assert_eq!(sink, data);
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn callback_failures_surface_as_io_errors() {
    let data = sample();
    let handle = engine();
    let mut source = Source {
        data: data.clone(),
        position: 0,
        fail_after: Some(5000),
    };
    let mut sink: Vec<u8> = Vec::new();
    // SAFETY: live engine, valid callbacks and contexts that outlive the call.
    let status = unsafe {
        ace_compress_stream(
            handle,
            data.len() as u64,
            Some(read_callback),
            ptr::from_mut(&mut source).cast(),
            Some(write_callback),
            ptr::from_mut(&mut sink).cast(),
            ptr::null_mut(),
        )
    };
    assert_eq!(status, ACE_ERR_IO);

    let (packed, _) = compress_streaming(handle, &data);
    let mut source = Source {
        data: packed,
        position: 0,
        fail_after: None,
    };
    // SAFETY: as above.
    let status = unsafe {
        ace_decompress_stream(
            handle,
            Some(read_callback),
            ptr::from_mut(&mut source).cast(),
            Some(failing_write),
            ptr::null_mut(),
        )
    };
    assert_eq!(status, ACE_ERR_IO);

    // SAFETY: null callbacks are rejected with a status.
    let status =
        unsafe { ace_decompress_stream(handle, None, ptr::null_mut(), None, ptr::null_mut()) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

fn open(handle: *const AceEngine, packed: &[u8]) -> *mut AceRangeReader {
    let mut reader = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
    let status = unsafe { ace_range_open(handle, packed.as_ptr(), packed.len(), &mut reader) };
    assert_eq!(status, ACE_OK);
    reader
}

#[test]
fn range_reads_equal_the_facade_reader() {
    let data = sample();
    let handle = engine();
    let (packed, _) = compress_streaming(handle, &data);
    let reader = open(handle, &packed);
    let (mut length, mut blocks) = (0_u64, 0_u64);
    // SAFETY: live reader, writable outputs.
    unsafe {
        assert_eq!(ace_range_len(reader, &mut length), ACE_OK);
        assert_eq!(ace_range_block_count(reader, &mut blocks), ACE_OK);
    }
    assert_eq!(length, data.len() as u64);
    assert_eq!(blocks, (data.len() as u64).div_ceil(BLOCK));

    let mut facade = ace::RangeReader::open(std::io::Cursor::new(packed.clone())).unwrap();
    for (start, end) in [
        (0_u64, 10_u64),
        (BLOCK - 5, BLOCK + 5),
        (1000, 300_000),
        (length - 1, length),
        (7, 7),
    ] {
        let mut out = ptr::null_mut();
        // SAFETY: live reader, writable output.
        let status = unsafe { ace_range_read(reader, start, end, &mut out) };
        assert_eq!(status, ACE_OK);
        assert_eq!(
            take(out),
            facade.read_range(start..end).unwrap(),
            "{start}..{end}"
        );
    }
    let (mut logical, mut physical, mut decoded) = (0_u64, 0_u64, 0_u64);
    // SAFETY: live reader, writable outputs.
    unsafe {
        assert_eq!(
            ace_range_last_stats(reader, &mut logical, &mut physical, &mut decoded),
            ACE_OK
        );
    }
    assert_eq!((logical, decoded), (0, 0), "the last read was empty");

    let mut out = ptr::null_mut();
    // SAFETY: live reader, writable output.
    unsafe {
        assert_eq!(ace_range_read_block(reader, 1, &mut out), ACE_OK);
        assert_eq!(take(out), facade.read_block(1).unwrap());
        assert_eq!(
            ace_range_read_block(reader, blocks, &mut out),
            ACE_ERR_BLOCK_NOT_FOUND
        );
        assert!(out.is_null());
        assert_eq!(
            ace_range_read(reader, 10, 5, &mut out),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ace_range_read(reader, 0, length + 1, &mut out),
            ACE_ERR_INVALID_ARGUMENT
        );
        ace_range_free(reader);
        ace_range_free(ptr::null_mut());
        ace_engine_free(handle);
    }
}

#[test]
fn range_reads_from_a_path() {
    let data = sample();
    let handle = engine();
    let (packed, _) = compress_streaming(handle, &data);
    let path = std::env::temp_dir().join(format!("ace-ffi-range-{}.ace", std::process::id()));
    std::fs::write(&path, &packed).unwrap();
    let text = path.to_str().unwrap().as_bytes();
    let mut reader = ptr::null_mut();
    // SAFETY: live engine, UTF-8 path bytes, writable output.
    let status = unsafe { ace_range_open_path(handle, text.as_ptr(), text.len(), &mut reader) };
    assert_eq!(status, ACE_OK);
    let mut out = ptr::null_mut();
    // SAFETY: live reader, writable output.
    let status = unsafe { ace_range_read(reader, 100, 200_000, &mut out) };
    assert_eq!(status, ACE_OK);
    assert_eq!(take(out), data[100..200_000]);
    let (mut logical, mut physical, mut decoded) = (0_u64, 0_u64, 0_u64);
    // SAFETY: live reader, writable outputs.
    unsafe {
        assert_eq!(
            ace_range_last_stats(reader, &mut logical, &mut physical, &mut decoded),
            ACE_OK
        );
    }
    assert_eq!(logical, 199_900);
    assert!(physical > 0 && decoded >= 3);

    let missing = b"/nonexistent/ace-ffi.ace";
    let mut none = ptr::null_mut();
    // SAFETY: as above.
    let status = unsafe { ace_range_open_path(handle, missing.as_ptr(), missing.len(), &mut none) };
    assert_eq!(status, ACE_ERR_IO);
    assert!(none.is_null());
    // SAFETY: released once each.
    unsafe {
        ace_range_free(reader);
        ace_engine_free(handle);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_file_without_an_index_is_not_range_readable() {
    let handle = engine();
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    let mut config = unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    };
    config.write_index = 0;
    let mut plain = ptr::null_mut();
    // SAFETY: valid config, writable output.
    let status = unsafe { ace_engine_new(&config, ptr::null(), &mut plain) };
    assert_eq!(status, ACE_OK);
    let data = sample();
    let mut packed = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
    let status = unsafe {
        ace_ffi::ace_compress(
            plain,
            data.as_ptr(),
            data.len(),
            &mut packed,
            ptr::null_mut(),
        )
    };
    assert_eq!(status, ACE_OK);
    let packed = take(packed);
    let mut reader = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
    let status = unsafe { ace_range_open(handle, packed.as_ptr(), packed.len(), &mut reader) };
    assert_ne!(status, ACE_OK);
    assert!(reader.is_null());
    // SAFETY: released once each.
    unsafe {
        ace_engine_free(plain);
        ace_engine_free(handle);
    }
}
