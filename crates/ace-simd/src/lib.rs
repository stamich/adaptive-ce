//! Small safe SIMD dispatch layer.
//!
//! Unsafe architecture intrinsics are isolated in the private `avx2` module; callers use the
//! safe functions of `scan`, which fall back to scalar code when AVX2 is unavailable, and
//! `crc32c`, which exposes the interleaved SSE4.2 checksum kernel.

mod avx2;
mod backend;
mod crc32c;
mod scan;

pub use backend::*;
pub use crc32c::*;
pub use scan::*;
