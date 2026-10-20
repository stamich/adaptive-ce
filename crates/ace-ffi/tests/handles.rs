//! Buffers, status codes and the thread-local last error.

use std::thread;

use ace::ErrorKind;
use ace_ffi::last_error::{fail, fail_with, NO_BLOCK};
use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_last_error_block, ace_last_error_len,
    ace_last_error_ptr, buffer_into_raw, status_of, ACE_ERR_CHECKSUM, ACE_ERR_INTERNAL,
    ACE_ERR_INVALID_ARGUMENT, ACE_OK,
};

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test
fn last_message() -> String {
    let (ptr, len) = (ace_last_error_ptr(), ace_last_error_len());
    assert!(!ptr.is_null());
    // SAFETY: the pointer and length come from the same thread-local message and no ACE call
    // happens before the copy.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8(bytes.to_vec()).expect("the message is UTF-8")
}

#[test]
fn buffer_exposes_its_bytes_until_freed() {
    let buffer = buffer_into_raw(vec![1, 2, 3, 4]);
    // SAFETY: `buffer` is a live handle from `buffer_into_raw`, freed exactly once below.
    unsafe {
        assert_eq!(ace_buffer_len(buffer), 4);
        let bytes = std::slice::from_raw_parts(ace_buffer_data(buffer), 4);
        assert_eq!(bytes, [1, 2, 3, 4]);
        ace_buffer_free(buffer);
    }
}

#[test]
fn empty_buffer_has_a_non_null_pointer() {
    let buffer = buffer_into_raw(Vec::new());
    // SAFETY: `buffer` is a live handle, freed once.
    unsafe {
        assert_eq!(ace_buffer_len(buffer), 0);
        assert!(!ace_buffer_data(buffer).is_null());
        ace_buffer_free(buffer);
    }
}

#[test]
fn null_buffer_is_harmless() {
    // SAFETY: null is explicitly allowed by every buffer function.
    unsafe {
        assert_eq!(ace_buffer_len(std::ptr::null()), 0);
        assert!(ace_buffer_data(std::ptr::null()).is_null());
        ace_buffer_free(std::ptr::null_mut());
    }
}

#[test]
fn error_kinds_map_to_distinct_statuses() {
    let kinds = [
        ErrorKind::InvalidConfig,
        ErrorKind::NotAceFile,
        ErrorKind::Unsupported,
        ErrorKind::Corrupt,
        ErrorKind::ChecksumMismatch,
        ErrorKind::LimitExceeded,
        ErrorKind::BlockNotFound,
        ErrorKind::Io,
    ];
    let mut seen: Vec<i32> = kinds.iter().map(|&kind| status_of(kind)).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(!seen.contains(&ACE_OK));
    assert!(!seen.contains(&ACE_ERR_INTERNAL));
}

#[test]
fn fail_records_message_and_returns_status() {
    let status = fail(ACE_ERR_INVALID_ARGUMENT, "bad argument");
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    assert_eq!(last_message(), "bad argument");
    assert_eq!(ace_last_error_block(), NO_BLOCK);
}

#[test]
fn facade_error_carries_its_block_number() {
    let engine = ace::Engine::default();
    let mut file = engine.compress(&[7_u8; 4096]).unwrap_or_default();
    assert!(!file.is_empty());
    let middle = file.len() / 2;
    if let Some(byte) = file.get_mut(middle) {
        *byte ^= 0xFF;
    }
    let error = engine.decompress(&file).err();
    let Some(error) = error else {
        // Flipping one byte must damage the file.
        assert!(error.is_some());
        return;
    };
    let status = fail_with(&error);
    assert_eq!(status, status_of(error.kind()));
    assert!(!last_message().is_empty());
    if status == ACE_ERR_CHECKSUM {
        assert_eq!(ace_last_error_block(), error.block_id().unwrap_or(NO_BLOCK));
    }
}

#[test]
fn last_error_is_per_thread() {
    fail(ACE_ERR_INVALID_ARGUMENT, "main thread");
    let other = thread::spawn(|| {
        assert_eq!(ace_last_error_len(), 0);
        fail(ACE_ERR_INVALID_ARGUMENT, "worker");
        last_message()
    });
    assert_eq!(other.join().ok().as_deref(), Some("worker"));
    assert_eq!(last_message(), "main thread");
}
