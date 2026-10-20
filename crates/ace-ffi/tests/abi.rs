//! The library-level entry points: ABI version and engine version.

use std::ffi::CStr;

use ace_ffi::{ace_abi_version, ace_engine_version, ABI_VERSION};

#[test]
fn abi_version_is_one() {
    assert_eq!(ace_abi_version(), 1);
    assert_eq!(ace_abi_version(), ABI_VERSION);
}

#[test]
fn engine_version_is_the_crate_version() {
    let pointer = ace_engine_version();
    assert!(!pointer.is_null());
    // SAFETY: `ace_engine_version` returns a NUL-terminated string with static storage duration.
    let text = unsafe { CStr::from_ptr(pointer) };
    assert_eq!(text.to_str(), Ok(env!("CARGO_PKG_VERSION")));
    assert_eq!(text.to_str(), Ok("0.7.0"));
}
