//! Tests of `ace_format` `checksum` (formerly the inline unit tests of `src`).

use ace_format::*;

/// Known CRC32C test vector ("123456789" → 0xE3069283) and agreement with the portable crate.
#[test]
fn crc32c_test_vector_and_portable_agreement() {
    assert_eq!(checksum(b"123456789"), 0xE306_9283);
    let data: Vec<u8> = (0..100_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    assert_eq!(checksum(&data), crc32c::crc32c(&data));
}
