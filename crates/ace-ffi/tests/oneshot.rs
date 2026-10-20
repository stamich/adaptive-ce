//! One-shot compress and decompress through the `extern "C"` API, compared with the Rust facade.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::ptr;

use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress, ace_config_default,
    ace_decompress, ace_decompress_into, ace_engine_free, ace_engine_new, ace_last_error_len,
    ace_limits_default, AceBuffer, AceConfig, AceEngine, AceLimits, ACE_ERR_BUFFER_TOO_SMALL,
    ACE_ERR_INVALID_ARGUMENT, ACE_ERR_LIMIT_EXCEEDED, ACE_ERR_NOT_ACE_FILE, ACE_OK,
};

fn sample() -> Vec<u8> {
    let mut data = Vec::new();
    for index in 0_u32..40_000 {
        data.extend_from_slice(&(1_000 + index / 3).to_le_bytes());
        data.push(b"ABCDEFG"[(index % 7) as usize]);
    }
    data
}

fn take(buffer: *mut AceBuffer) -> Vec<u8> {
    assert!(!buffer.is_null());
    // SAFETY: `buffer` is a live handle; it is freed once, after the copy.
    unsafe {
        let bytes =
            std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer)).to_vec();
        ace_buffer_free(buffer);
        bytes
    }
}

fn engine(config: Option<&AceConfig>, limits: Option<&AceLimits>) -> *mut AceEngine {
    let mut handle = ptr::null_mut();
    let config = config.map_or(ptr::null(), ptr::from_ref);
    let limits = limits.map_or(ptr::null(), ptr::from_ref);
    // SAFETY: the pointers are null or reference live structs; `handle` is writable.
    let status = unsafe { ace_engine_new(config, limits, &mut handle) };
    assert_eq!(status, ACE_OK);
    assert!(!handle.is_null());
    handle
}

fn compress(engine: *const AceEngine, input: &[u8]) -> (Vec<u8>, String) {
    let (mut out, mut stats) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: live engine and input; both outputs are writable.
    let status = unsafe { ace_compress(engine, input.as_ptr(), input.len(), &mut out, &mut stats) };
    assert_eq!(status, ACE_OK);
    (take(out), String::from_utf8(take(stats)).unwrap())
}

#[test]
fn compress_matches_the_facade_byte_for_byte() {
    let input = sample();
    let handle = engine(None, None);
    let (bytes, _) = compress(handle, &input);
    assert_eq!(bytes, ace::Engine::default().compress(&input).unwrap());
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn every_profile_matches_the_facade() {
    let input = sample();
    for (code, profile) in [
        (0, ace::Profile::Fast),
        (1, ace::Profile::Balanced),
        (2, ace::Profile::Dense),
    ] {
        let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
        // SAFETY: `ace_config_default` initialises the whole struct.
        let mut config = unsafe {
            ace_config_default(config.as_mut_ptr());
            config.assume_init()
        };
        config.profile = code;
        config.block_size = 64 * 1024;
        let handle = engine(Some(&config), None);
        let (bytes, _) = compress(handle, &input);
        let reference = ace::Engine::new(
            ace::Config::builder()
                .profile(profile)
                .block_size(64 * 1024)
                .build()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(bytes, reference.compress(&input).unwrap(), "profile {code}");
        // SAFETY: released once.
        unsafe { ace_engine_free(handle) };
    }
}

#[test]
fn decompress_round_trips() {
    let input = sample();
    let handle = engine(None, None);
    let (packed, _) = compress(handle, &input);
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; `out` is writable.
    let status = unsafe { ace_decompress(handle, packed.as_ptr(), packed.len(), &mut out) };
    assert_eq!(status, ACE_OK);
    assert_eq!(take(out), input);
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn empty_input_round_trips_with_a_null_pointer() {
    let handle = engine(None, None);
    let (mut packed, mut out) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: a zero length may come with a null pointer.
    unsafe {
        assert_eq!(
            ace_compress(handle, ptr::null(), 0, &mut packed, ptr::null_mut()),
            ACE_OK
        );
    }
    let packed = take(packed);
    // SAFETY: live engine and input.
    unsafe {
        assert_eq!(
            ace_decompress(handle, packed.as_ptr(), packed.len(), &mut out),
            ACE_OK
        );
    }
    assert!(take(out).is_empty());
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn stats_json_has_the_documented_keys_in_order() {
    let input = sample();
    let handle = engine(None, None);
    let (packed, stats) = compress(handle, &input);
    let keys = [
        "input_bytes",
        "output_bytes",
        "format_minor",
        "blocks",
        "raw_blocks",
        "rle_blocks",
        "lz_blocks",
        "numeric_blocks",
        "time_series_blocks",
        "ratio",
        "plan_distribution",
    ];
    let mut cursor = 0;
    for key in keys {
        let at = stats[cursor..]
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("{key} in {stats}"));
        cursor += at;
    }
    assert!(stats.contains(&format!("\"input_bytes\":{}", input.len())));
    assert!(stats.contains(&format!("\"output_bytes\":{}", packed.len())));
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn decompress_into_reports_the_required_size() {
    let input = sample();
    let handle = engine(None, None);
    let (packed, _) = compress(handle, &input);
    let mut written = 0_usize;
    let mut small = vec![0_u8; 10];
    // SAFETY: live engine and input; `small` has 10 writable bytes.
    let status = unsafe {
        ace_decompress_into(
            handle,
            packed.as_ptr(),
            packed.len(),
            small.as_mut_ptr(),
            small.len(),
            &mut written,
        )
    };
    assert_eq!(status, ACE_ERR_BUFFER_TOO_SMALL);
    assert_eq!(written, input.len());
    assert!(
        small.iter().all(|&byte| byte == 0),
        "a too-small buffer is left untouched"
    );
    let mut exact = vec![0_u8; input.len()];
    // SAFETY: as above with a buffer of the required size.
    let status = unsafe {
        ace_decompress_into(
            handle,
            packed.as_ptr(),
            packed.len(),
            exact.as_mut_ptr(),
            exact.len(),
            &mut written,
        )
    };
    assert_eq!(status, ACE_OK);
    assert_eq!(exact, input);
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn limits_cross_the_boundary() {
    let input = sample();
    let plain = engine(None, None);
    let (packed, _) = compress(plain, &input);
    let mut limits = std::mem::MaybeUninit::<AceLimits>::uninit();
    // SAFETY: `ace_limits_default` initialises the whole struct.
    let mut limits = unsafe {
        ace_limits_default(limits.as_mut_ptr());
        limits.assume_init()
    };
    limits.max_output_size = 1_000;
    let strict = engine(None, Some(&limits));
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; `out` is writable.
    let status = unsafe { ace_decompress(strict, packed.as_ptr(), packed.len(), &mut out) };
    assert_eq!(status, ACE_ERR_LIMIT_EXCEEDED);
    assert!(out.is_null());
    assert!(ace_last_error_len() > 0);
    // SAFETY: released once each.
    unsafe {
        ace_engine_free(strict);
        ace_engine_free(plain);
    }
}

#[test]
fn garbage_is_not_an_ace_file() {
    let handle = engine(None, None);
    let mut out = ptr::null_mut();
    let garbage = [0x13_u8; 64];
    // SAFETY: live engine and input; `out` is writable.
    let status = unsafe { ace_decompress(handle, garbage.as_ptr(), garbage.len(), &mut out) };
    assert_eq!(status, ACE_ERR_NOT_ACE_FILE);
    assert!(out.is_null());
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn config_validation() {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    let config = unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    };
    let mut handle = ptr::null_mut();

    let mut unknown_profile = config;
    unknown_profile.profile = 9;
    let mut reserved = config;
    reserved.reserved[0] = 1;
    let mut too_small = config;
    too_small.struct_size = 1;
    for bad in [unknown_profile, reserved, too_small] {
        // SAFETY: valid struct, writable output.
        let status = unsafe { ace_engine_new(&bad, ptr::null(), &mut handle) };
        assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
        assert!(handle.is_null());
    }

    // A caller that only knows the fields up to `profile` gets defaults for the rest.
    let mut older = config;
    older.struct_size = std::mem::offset_of!(AceConfig, block_size);
    older.block_size = 7; // beyond the declared size: ignored
                          // SAFETY: valid struct, writable output.
    let status = unsafe { ace_engine_new(&older, ptr::null(), &mut handle) };
    assert_eq!(status, ACE_OK);
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn null_engine_and_null_output_are_rejected() {
    let mut out = ptr::null_mut();
    let data = [1_u8, 2, 3];
    // SAFETY: null handles are rejected with a status, never dereferenced.
    unsafe {
        assert_eq!(
            ace_compress(ptr::null(), data.as_ptr(), 3, &mut out, ptr::null_mut()),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert!(out.is_null());
        assert_eq!(
            ace_decompress(ptr::null(), data.as_ptr(), 3, ptr::null_mut()),
            ACE_ERR_INVALID_ARGUMENT
        );
        let handle = engine(None, None);
        assert_eq!(
            ace_compress(handle, ptr::null(), 3, &mut out, ptr::null_mut()),
            ACE_ERR_INVALID_ARGUMENT
        );
        ace_engine_free(handle);
    }
}
