//! The hardened boundary: panics are contained, hostile lengths and pointers are rejected.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use ace_ffi::guard::{guard, guard_value};
use ace_ffi::{
    ace_compress, ace_compress_stream, ace_config_default, ace_decompress, ace_decompress_into,
    ace_engine_free, ace_engine_new, ace_last_error_len, ace_last_error_ptr, ace_range_open,
    AceConfig, AceEngine, ACE_ERR_INVALID_ARGUMENT, ACE_ERR_PANIC, ACE_OK,
};

fn message() -> String {
    // SAFETY: pointer and length describe the thread-local message; nothing runs in between.
    let bytes = unsafe { std::slice::from_raw_parts(ace_last_error_ptr(), ace_last_error_len()) };
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn engine() -> *mut AceEngine {
    let mut handle = ptr::null_mut();
    // SAFETY: null configs are the defaults; the output is writable.
    let status = unsafe { ace_engine_new(ptr::null(), ptr::null(), &mut handle) };
    assert_eq!(status, ACE_OK);
    handle
}

#[test]
fn a_panic_becomes_a_status() {
    let status = guard(|| panic!("injected"));
    assert_eq!(status, ACE_ERR_PANIC);
    assert_eq!(message(), "internal panic: injected");
    assert_eq!(guard_value(7, || panic!("{}", String::from("owned"))), 7);
    assert_eq!(message(), "internal panic: owned");
    assert_eq!(guard(|| ACE_OK), ACE_OK);
}

static DELIVERED: AtomicBool = AtomicBool::new(false);

unsafe extern "C-unwind" fn read_one(_: *mut c_void, buf: *mut u8, capacity: usize) -> i64 {
    if capacity == 0 || DELIVERED.swap(true, Ordering::SeqCst) {
        return 0;
    }
    {
        // SAFETY: `buf` has room for at least one byte.
        unsafe { buf.write(1) };
    }
    1
}

unsafe extern "C-unwind" fn panicking_write(_: *mut c_void, _: *const u8, _: usize) -> i32 {
    panic!("the callback panicked");
}

#[test]
fn a_panicking_callback_does_not_unwind_into_the_host() {
    let handle = engine();
    // SAFETY: live engine, valid callbacks, null contexts that the callbacks ignore.
    let status = unsafe {
        ace_compress_stream(
            handle,
            1,
            Some(read_one),
            ptr::null_mut(),
            Some(panicking_write),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    assert_eq!(status, ACE_ERR_PANIC);
    assert!(message().contains("the callback panicked"), "{}", message());
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn hostile_lengths_and_pointers_are_rejected() {
    let handle = engine();
    let mut out = ptr::null_mut();
    let dangling = 0x10_usize as *const u8;
    // SAFETY: the pointers are rejected before any dereference.
    unsafe {
        assert_eq!(
            ace_decompress(handle, ptr::null(), 5, &mut out),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ace_decompress(handle, dangling, usize::MAX, &mut out),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ace_compress(handle, dangling, usize::MAX, &mut out, ptr::null_mut()),
            ACE_ERR_INVALID_ARGUMENT
        );
        let mut reader = ptr::null_mut();
        assert_eq!(
            ace_range_open(handle, dangling, usize::MAX, &mut reader),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert!(reader.is_null());
        let mut written = 99_usize;
        assert_eq!(
            ace_decompress_into(handle, ptr::null(), 0, ptr::null_mut(), 8, &mut written),
            ACE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(written, 0);
        assert_eq!(
            ace_decompress_into(handle, ptr::null(), 0, ptr::null_mut(), 0, ptr::null_mut()),
            ACE_ERR_INVALID_ARGUMENT
        );
    }
    assert!(out.is_null());
    // SAFETY: released once.
    unsafe { ace_engine_free(handle) };
}

#[test]
fn extreme_config_values_fail_cleanly() {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    let config = unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    };
    for mutate in [
        (|c: &mut AceConfig| c.block_size = u64::MAX) as fn(&mut AceConfig),
        |c| c.block_size = 1 << 40,
        |c| c.memory_budget = u64::MAX,
        |c| c.threads = u64::MAX,
        |c| c.block_size = 3,
    ] {
        let mut candidate = config;
        mutate(&mut candidate);
        let mut handle = ptr::null_mut();
        // SAFETY: valid struct, writable output.
        let status = unsafe { ace_engine_new(&candidate, ptr::null(), &mut handle) };
        if status == ACE_OK {
            // SAFETY: released once.
            unsafe { ace_engine_free(handle) };
        } else {
            assert!(handle.is_null());
            assert!(ace_last_error_len() > 0);
        }
    }
}
