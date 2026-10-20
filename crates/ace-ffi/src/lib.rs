//! C ABI of the Adaptive Compression Engine.
//!
//! This crate exports the `ace_*` functions declared in `include/ace.h`. The conventions every
//! function follows are in `docs/NATIVE-INTEROP.md`, the function list in `docs/FFI.md`.
//!
//! **Stability:** the exported C symbols are the contract (ABI version [`ACE_ABI_VERSION`]). The Rust
//! items of this crate are an implementation detail and are not published.

use std::ffi::c_char;

pub mod args;
pub mod buffer;
pub mod config;
pub mod engine;
pub mod guard;
pub mod handle;
pub mod json;
pub mod last_error;
pub mod range;
pub mod status;
pub mod stream;

pub use buffer::{ace_buffer_data, ace_buffer_free, ace_buffer_len, buffer_into_raw, AceBuffer};
pub use config::{ace_config_default, ace_limits_default, AceConfig, AceLimits};
pub use engine::{
    ace_compress, ace_decompress, ace_decompress_into, ace_engine_free, ace_engine_new,
    ace_explain_json, AceEngine,
};
pub use last_error::{ace_last_error_block, ace_last_error_len, ace_last_error_ptr};
pub use range::{
    ace_range_block_count, ace_range_free, ace_range_last_stats, ace_range_len, ace_range_open,
    ace_range_open_path, ace_range_read, ace_range_read_block, AceRangeReader,
};
pub use status::*;
pub use stream::{ace_compress_stream, ace_decompress_stream, AceReadFn, AceWriteFn};

/// Version of the C ABI. Bumped when a function is removed or a signature, status value or
/// struct layout changes; adding a function does not change it.
pub const ACE_ABI_VERSION: u32 = 1;

/// NUL-terminated crate version handed out by [`ace_engine_version`].
static ENGINE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Returns the version of the C ABI this library implements.
///
/// A binding compares it with the version it was generated against before it uses any other
/// function.
#[no_mangle]
pub extern "C" fn ace_abi_version() -> u32 {
    ACE_ABI_VERSION
}

/// Returns the crate version as a NUL-terminated string with static storage duration.
///
/// The string is for diagnostics; nothing should branch on it.
#[no_mangle]
pub extern "C" fn ace_engine_version() -> *const c_char {
    ENGINE_VERSION.as_ptr().cast()
}
