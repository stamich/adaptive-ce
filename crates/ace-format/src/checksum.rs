//! CRC32C integrity checksum.
//!
//! Uses the interleaved SSE4.2 kernel from `ace-simd` when available (≈3× a single CRC chain)
//! and the portable `crc32c` crate otherwise; both produce identical values.

/// Computes CRC32C for ACE headers, indexes and reconstructed block payloads.
pub fn checksum(bytes: &[u8]) -> u32 {
    ace_simd::crc32c_hardware(bytes).unwrap_or_else(|| crc32c::crc32c(bytes))
}
