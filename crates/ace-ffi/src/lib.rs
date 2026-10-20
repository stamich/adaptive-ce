//! C ABI of the Adaptive Compression Engine.
//!
//! This crate exports the `ace_*` functions declared in `include/ace.h`. The conventions every
//! function follows are in `docs/NATIVE-INTEROP.md`, the function list in `docs/FFI.md`.
//!
//! **Stability:** the exported C symbols are the contract (ABI version [`ABI_VERSION`]). The Rust
//! items of this crate are an implementation detail and are not published.

use std::ffi::c_char;

pub mod buffer;
pub mod handle;
pub mod last_error;
pub mod status;

pub use buffer::{ace_buffer_data, ace_buffer_free, ace_buffer_len, buffer_into_raw, AceBuffer};
pub use last_error::{ace_last_error_block, ace_last_error_len, ace_last_error_ptr};
pub use status::*;

/// Version of the C ABI. Bumped when a function is removed or a signature, status value or
/// struct layout changes; adding a function does not change it.
pub const ABI_VERSION: u32 = 1;

/// NUL-terminated crate version handed out by [`ace_engine_version`].
static ENGINE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Returns the version of the C ABI this library implements.
///
/// A binding compares it with the version it was generated against before it uses any other
/// function.
#[no_mangle]
pub extern "C" fn ace_abi_version() -> u32 {
    ABI_VERSION
}

/// Returns the crate version as a NUL-terminated string with static storage duration.
///
/// The string is for diagnostics; nothing should branch on it.
#[no_mangle]
pub extern "C" fn ace_engine_version() -> *const c_char {
    ENGINE_VERSION.as_ptr().cast()
}
