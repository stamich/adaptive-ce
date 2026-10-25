//! Small safe SIMD dispatch layer.
//!
//! Unsafe architecture intrinsics are isolated in the private `avx2` (x86_64) and `neon`
//! (aarch64) modules; callers use the safe functions of `scan`, which fall back to scalar code
//! when neither is available, and `crc32c`, which exposes the interleaved hardware checksum
//! kernel (SSE4.2 on x86_64, the ARMv8 CRC extension on aarch64). All backends return the same
//! values, so the written bytes do not depend on the CPU.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod avx2;
mod backend;
mod crc32c;
mod neon;
mod scan;

pub use backend::*;
pub use crc32c::*;
pub use scan::*;
