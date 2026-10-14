//! Per-field hardening of the entropy metadata (ACE 0.5.1 decoder audit): rANS frequency
//! tables, rANS4x lane lengths and Huffman length tables forged to 0, `u16::MAX` /
//! `u32::MAX` and sizes whose arithmetic would overflow are rejected without a panic, and
//! streams must end exactly where the encoder stopped (final rANS state, Huffman padding).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_entropy::{
    huffman_decode, huffman_encode, rans4x_decode, rans4x_encode, rans_decode, rans_encode,
    NormalizedFrequencyTable, RANS4X_LANES, RANS4X_METADATA_BYTES,
};

/// Mixed-alphabet input that exercises every lane.
fn sample() -> Vec<u8> {
    (0u32..40_000)
        .map(|i| b"abracadabra-0123"[(i * 7 % 16) as usize])
        .collect()
}

/// rANS frequency tables whose sum is wrong or whose prefix sums would overflow `u16`.
#[test]
fn rans_frequency_tables_with_bad_sums_are_rejected() {
    let mut all_max = [0u8; 512];
    all_max.fill(0xFF); // 256 * u16::MAX
    assert!(NormalizedFrequencyTable::decode_metadata(&all_max).is_err());
    let mut one_symbol_max = [0u8; 512];
    one_symbol_max[..2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(NormalizedFrequencyTable::decode_metadata(&one_symbol_max).is_err());
    assert!(NormalizedFrequencyTable::decode_metadata(&[0u8; 511]).is_err());
    // An all-zero model only decodes an empty output.
    assert!(rans_decode(&[0u8; 512], &[0, 0, 0x80, 0], 1).is_err());
}

/// rANS4x lane lengths forged to 0, `u32::MAX` and values whose sum overflows.
#[test]
fn rans4x_lane_lengths_reject_zero_max_and_overflow() {
    let data = sample();
    let (metadata, payload) = rans4x_encode(&data).unwrap();
    assert_eq!(metadata.len(), RANS4X_METADATA_BYTES);
    assert_eq!(
        rans4x_decode(&metadata, &payload, data.len()).unwrap(),
        data
    );
    let lengths_at = RANS4X_METADATA_BYTES - RANS4X_LANES * 4;
    for lane in 0..RANS4X_LANES {
        for value in [0u32, 1, u32::MAX, u32::MAX / 2 + 1] {
            let mut forged = metadata.clone();
            let at = lengths_at + lane * 4;
            forged[at..at + 4].copy_from_slice(&value.to_le_bytes());
            if forged == metadata {
                continue;
            }
            assert!(
                rans4x_decode(&forged, &payload, data.len()).is_err(),
                "lane {lane} length {value} decoded"
            );
        }
    }
    for expected in [0, data.len() - 1, data.len() + 1, usize::MAX] {
        assert!(rans4x_decode(&metadata, &payload, expected).is_err());
    }
}

/// Scalar rANS and Huffman reject forged output sizes, including `usize::MAX`.
#[test]
fn scalar_coders_reject_extreme_output_sizes() {
    let data = sample();
    let (metadata, payload) = rans_encode(&data).unwrap();
    // The final-state check rejects every size but the encoded one.
    for expected in [0, 1, data.len() - 1, data.len() + 1, usize::MAX] {
        assert!(rans_decode(&metadata, &payload, expected).is_err());
    }
    let (lengths, bits) = huffman_encode(&data).unwrap();
    // Huffman stores no bit count: up to 7 padding bits may decode as extra symbols, which the
    // block CRC-32C rejects one layer up. Far larger sizes always fail.
    for expected in [0, 1, data.len() / 2, data.len() + 64, usize::MAX] {
        assert!(huffman_decode(&lengths, &bits, expected).is_err());
    }
    let mut padded = bits.clone();
    padded.push(0);
    assert!(huffman_decode(&lengths, &padded, data.len()).is_err());
    let mut forged = lengths.clone();
    forged.fill(u8::MAX); // code lengths beyond any canonical code
    assert!(huffman_decode(&forged, &bits, data.len()).is_err());
}
