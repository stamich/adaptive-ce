//! Small safe SIMD dispatch layer.
//!
//! Unsafe architecture intrinsics are isolated in the private `avx2` module; callers use the
//! safe functions of `scan`, which fall back to scalar code when AVX2 is unavailable, and
//! `crc32c`, which exposes the interleaved SSE4.2 checksum kernel.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod avx2;
mod backend;
mod crc32c;
mod scan;

pub use backend::*;
pub use crc32c::*;
pub use scan::*;
