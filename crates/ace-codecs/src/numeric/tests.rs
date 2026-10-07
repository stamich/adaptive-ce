//! Unit tests of the NUM1 codec (round-trips, exact estimates, header validation, hostile input).

use ace_core::NumericWidth;

use crate::*;

/// Serializes a u32 sequence to little-endian bytes.
fn bytes_u32(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Serializes a u64 sequence to little-endian bytes.
fn bytes_u64(values: impl IntoIterator<Item = u64>) -> Vec<u8> {
    values.into_iter().flat_map(u64::to_le_bytes).collect()
}

/// Serializes a u16 sequence to little-endian bytes.
fn bytes_u16(values: impl IntoIterator<Item = u16>) -> Vec<u8> {
    values.into_iter().flat_map(u16::to_le_bytes).collect()
}

/// Verifies monotonic u32 counters choose a compact numeric representation and round-trip exactly.
#[test]
fn u32_counter_roundtrip() {
    let mut input = bytes_u32(1000u32..50_000);
    input.extend_from_slice(&[1, 2, 3]);
    let encoded = numeric_encode(&input).unwrap();
    assert!(encoded.len() < input.len() / 2);
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// Verifies millisecond-like u64 timestamps use a reversible compact path.
#[test]
fn u64_timestamp_roundtrip() {
    let mut value = 1_780_000_000_000u64;
    let input = bytes_u64((0..20_000u64).map(|i| {
        value += 1000 + (i % 3);
        value
    }));
    let encoded = numeric_encode(&input).unwrap();
    assert!(encoded.len() < input.len() / 2);
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// A wrapping u16 counter must compress to almost nothing: the wrap is a +1 step in the ring.
#[test]
fn u16_wrapping_counter_roundtrip() {
    let mut input = bytes_u16((0..40_000u32).map(|i| (60_000 + i) as u16));
    input.push(0xAB);
    let encoded = numeric_encode(&input).unwrap();
    let info = numeric_inspect(&encoded).unwrap();
    assert_eq!(info.width, NumericWidth::U16);
    assert_eq!(info.mode, NumericMode::DeltaOfDelta);
    assert_eq!(info.bit_width, 0, "constant +1 step needs zero packed bits");
    assert_eq!(info.tail_bytes, 1);
    assert_eq!(encoded.len(), NUMERIC_HEADER_SIZE + 1);
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// Every `(width, mode)` pair must round-trip, with and without tail bytes, for noisy data.
#[test]
fn every_width_and_mode_roundtrips() {
    let mut state = 0xdead_beefu64;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state >> 16
    };
    for tail in [0usize, 1, 3] {
        for (width, lane) in [
            (NumericWidth::U16, 2usize),
            (NumericWidth::U32, 4),
            (NumericWidth::U64, 8),
        ] {
            let mut input: Vec<u8> = (0..300 * lane).map(|_| next() as u8).collect();
            input.extend((0..tail.min(lane - 1)).map(|_| next() as u8));
            for mode in NumericMode::ALL {
                let encoded = numeric_encode_with(&input, width, mode).unwrap();
                assert_eq!(
                    numeric_decode(&encoded, input.len()).unwrap(),
                    input,
                    "width={width:?} mode={mode:?} tail={tail}"
                );
            }
        }
    }
}

/// Estimates are exact: they equal the length of the payload that is actually produced.
#[test]
fn estimate_equals_encoded_length() {
    let input = bytes_u32((0..5000u32).map(|i| i * i % 1_000_003));
    for width in [NumericWidth::U16, NumericWidth::U32, NumericWidth::U64] {
        for est in estimate_every_mode(&input, width).unwrap() {
            let mode = est.mode;
            let encoded = numeric_encode_with(&input, width, mode).unwrap();
            assert_eq!(est.encoded_bytes, encoded.len(), "{width:?} {mode:?}");
            assert_eq!(est.bit_width, numeric_inspect(&encoded).unwrap().bit_width);
        }
    }
}

/// Verifies validated fixed-step evidence can bypass the generic numeric mode search.
#[test]
fn direct_fixed_step_encoder_roundtrip() {
    let mut value = 10_000u32;
    let first = value + 3;
    let input = bytes_u32((0..10_000).map(|_| {
        value = value.wrapping_add(3);
        value
    }));
    let encoded =
        numeric_encode_fixed_step(&input, NumericWidth::U32, first as u64, 3, 10_000, 0).unwrap();
    assert_eq!(encoded[5], NumericMode::DeltaOfDelta as u8);
    assert_eq!(encoded[6], 0);
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// The generic encoder and the direct fixed-step encoder must emit identical bytes.
#[test]
fn fixed_step_encoder_matches_generic_encoder() {
    for width in [NumericWidth::U16, NumericWidth::U32, NumericWidth::U64] {
        let input: Vec<u8> = match width {
            NumericWidth::U16 => bytes_u16((0..1000u32).map(|i| (7 + i * 5) as u16)),
            NumericWidth::U32 => bytes_u32((0..1000u32).map(|i| 7 + i * 5)),
            NumericWidth::U64 => bytes_u64((0..1000u64).map(|i| 7 + i * 5)),
        };
        let generic = numeric_encode_with(&input, width, NumericMode::DeltaOfDelta).unwrap();
        let direct = numeric_encode_fixed_step(&input, width, 7, 5, 1000, 0).unwrap();
        assert_eq!(generic, direct, "{width:?}");
    }
}

/// Verifies the zero-bit-width DoD decoder reconstructs a fixed-step u32 stream directly.
#[test]
fn zero_width_u32_dod_roundtrip() {
    let mut value = 10_000u32;
    let input = bytes_u32((0..10_000).map(|_| {
        value = value.wrapping_add(3);
        value
    }));
    let encoded = numeric_encode(&input).unwrap();
    assert_eq!(
        encoded[6], 0,
        "fixed-step DoD should require zero packed bits"
    );
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// Verifies the zero-bit-width DoD decoder reconstructs a fixed-step u64 stream directly.
#[test]
fn zero_width_u64_dod_roundtrip() {
    let mut value = 1_780_000_000_000u64;
    let input = bytes_u64((0..10_000).map(|_| {
        value = value.wrapping_add(1000);
        value
    }));
    let encoded = numeric_encode(&input).unwrap();
    assert_eq!(
        encoded[6], 0,
        "fixed-step DoD should require zero packed bits"
    );
    assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
}

/// `numeric_inspect` reports header fields without decoding and agrees with the encoder.
#[test]
fn inspect_reports_header_fields() {
    let input = bytes_u64((0..4096u64).map(|i| 1_000_000 + i * 1000 + (i % 7)));
    let encoded = numeric_encode(&input).unwrap();
    let info = numeric_inspect(&encoded).unwrap();
    assert_eq!(info.width, NumericWidth::U64);
    assert_eq!(info.value_count, 4096);
    assert_eq!(info.tail_bytes, 0);
    assert_eq!(info.first_value, 1_000_000);
    assert_eq!(info.packed_bytes + NUMERIC_HEADER_SIZE, encoded.len());
    assert!(info.bits_per_value() > 0.0 && info.bits_per_value() < 16.0);
    assert!(!info.mode.label().is_empty());
}

/// Fewer than two complete values cannot be encoded as numeric.
#[test]
fn too_short_input_is_rejected() {
    assert!(numeric_encode(&[1, 2, 3]).is_err());
    assert!(numeric_encode_with(&[0u8; 7], NumericWidth::U32, NumericMode::Delta).is_err());
    assert!(estimate_numeric(&[0u8; 3]).is_none());
}

/// Builds a valid small payload used as the starting point of malformed-input mutations.
fn valid_payload() -> (Vec<u8>, usize) {
    let input = bytes_u32((0..64u32).map(|i| i * 3 + (i % 5)));
    (numeric_encode(&input).unwrap(), input.len())
}

/// Each individual header corruption must be rejected with an error, never a panic.
#[test]
fn malformed_headers_are_rejected() {
    let (good, size) = valid_payload();
    assert_eq!(numeric_decode(&good, size).unwrap().len(), size);

    let mutate = |offset: usize, value: u8| {
        let mut bad = good.clone();
        bad[offset] = value;
        bad
    };
    // Magic, width, mode, bit width beyond the lane.
    assert!(numeric_decode(&mutate(0, b'X'), size).is_err());
    assert!(numeric_decode(&mutate(4, 3), size).is_err());
    assert!(numeric_decode(&mutate(4, 0), size).is_err());
    assert!(numeric_decode(&mutate(5, 0), size).is_err());
    assert!(numeric_decode(&mutate(5, 9), size).is_err());
    assert!(numeric_decode(&mutate(6, 33), size).is_err());
    // Count changes break the declared size and the packed length.
    assert!(numeric_decode(&mutate(8, 63), size).is_err());
    assert!(numeric_decode(&mutate(11, 0xff), size).is_err());
    // First value outside the u32 lane.
    assert!(numeric_decode(&mutate(20, 1), size).is_err());
    // Wrong expected size, truncation and trailing garbage.
    assert!(numeric_decode(&good, size + 1).is_err());
    assert!(numeric_decode(&good[..good.len() - 1], size).is_err());
    assert!(numeric_decode(&good[..NUMERIC_HEADER_SIZE - 1], size).is_err());
    let mut padded = good.clone();
    padded.push(0);
    assert!(numeric_decode(&padded, size).is_err());
    // A zero-bit-width header that still carries packed bytes is inconsistent.
    let mut zero = good.clone();
    zero[6] = 0;
    assert!(numeric_decode(&zero, size).is_err());
}

/// Hostile counts must fail on the size check instead of allocating gigabytes.
#[test]
fn huge_declared_count_does_not_allocate() {
    let (mut bad, _) = valid_payload();
    bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(numeric_decode(&bad, 1 << 20).is_err());
}

/// Arbitrary bytes must never panic the decoder (a cheap deterministic fuzz smoke test).
#[test]
fn decoder_survives_pseudorandom_garbage() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for round in 0..2000usize {
        let len = 36 + (round % 90);
        let mut data: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        // Half of the rounds start from a syntactically plausible header.
        if round % 2 == 0 && data.len() >= 8 {
            data[0..4].copy_from_slice(b"NUM1");
            data[4] = [2u8, 4, 8][round % 3];
            data[5] = 1 + (round % 3) as u8;
        }
        let _ = numeric_decode(&data, round % 300);
        let _ = numeric_inspect(&data);
    }
}
