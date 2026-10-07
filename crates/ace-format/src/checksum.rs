//! CRC32C integrity checksum.
//!
//! Uses the interleaved SSE4.2 kernel from `ace-simd` when available (≈3× a single CRC chain)
//! and the portable `crc32c` crate otherwise; both produce identical values.

/// Computes CRC32C for ACE headers, indexes and reconstructed block payloads.
pub fn checksum(bytes: &[u8]) -> u32 {
    ace_simd::crc32c_hardware(bytes).unwrap_or_else(|| crc32c::crc32c(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known CRC32C test vector ("123456789" → 0xE3069283) and agreement with the portable crate.
    #[test]
    fn crc32c_test_vector_and_portable_agreement() {
        assert_eq!(checksum(b"123456789"), 0xE306_9283);
        let data: Vec<u8> = (0..100_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        assert_eq!(checksum(&data), crc32c::crc32c(&data));
    }
}
