//! Tests of `ace_simd` `crc32c` (formerly the inline unit tests of `src`).

use ace_simd::*;

/// CRC32C (Castagnoli) polynomial, reflected.
const POLY: u32 = 0x82F6_3B78;

/// Bit-at-a-time reference CRC32C.
fn reference(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Matches the reference for every length around the stripe threshold and for large blocks.
#[test]
fn hardware_crc_matches_reference() {
    let Some(_) = crc32c_hardware(b"") else {
        return;
    };
    let mut state = 0x1234_5678u64;
    let data: Vec<u8> = (0..300_010)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    assert_eq!(crc32c_hardware(b"123456789"), Some(0xE306_9283));
    for len in (0..2_000).chain([3 * 256, 3 * 256 + 7, 65_536, 262_144, 299_999]) {
        for offset in [0usize, 1, 5] {
            let slice = &data[offset..offset + len];
            assert_eq!(
                crc32c_hardware(slice),
                Some(reference(slice)),
                "len={len} offset={offset}"
            );
        }
    }
}
