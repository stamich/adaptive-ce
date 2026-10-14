//! Per-field hardening of the NUM1 and TS1 headers (ACE 0.5.1 decoder audit).
//!
//! Every length / count field read from a payload is forged to 0, its maximum (`u32::MAX`)
//! and values whose size arithmetic would overflow; the parser must reject them with an
//! error before decoding any value, never panic and never allocate from the forged value.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_codecs::{
    numeric_decode, numeric_encode, numeric_inspect, ts1_decode, ts1_encode_with, ts1_inspect,
    TimeSeriesLayout, NUMERIC_HEADER_SIZE, TIME_SERIES_HEADER_SIZE,
};

/// A valid NUM1 payload (u32 counter with a sparse tail).
fn num1_payload() -> (Vec<u8>, usize) {
    let input: Vec<u8> = (0u32..1000)
        .flat_map(|v| (v * 3).to_le_bytes())
        .chain([7u8, 9])
        .collect();
    (numeric_encode(&input).unwrap(), input.len())
}

/// A valid TS1 RunDelta payload over u64 lanes.
fn ts1_payload() -> (Vec<u8>, usize) {
    let input: Vec<u8> = (0u64..512)
        .flat_map(|v| (v / 16).to_le_bytes())
        .chain([1u8, 2, 3])
        .collect();
    let payload = ts1_encode_with(&input, TimeSeriesLayout::run_delta(8)).unwrap();
    (payload, input.len())
}

/// `payload` with the little-endian `u32` at `offset` replaced by `value`.
fn with_u32(payload: &[u8], offset: usize, value: u32) -> Vec<u8> {
    let mut forged = payload.to_vec();
    forged[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    forged
}

/// The untouched fixtures are accepted (guards the offsets used below).
#[test]
fn fixtures_are_valid() {
    let (num1, len) = num1_payload();
    assert_eq!(numeric_decode(&num1, len).unwrap().len(), len);
    let (ts1, len) = ts1_payload();
    assert_eq!(ts1_decode(&ts1, len).unwrap().len(), len);
}

/// NUM1 value count, tail byte count and packed byte count (offsets 8, 12, 32).
#[test]
fn num1_count_fields_reject_zero_max_and_overflow() {
    let (payload, len) = num1_payload();
    for offset in [8usize, 12, 32] {
        for value in [0u32, 1, u32::MAX, u32::MAX - 1, 0x8000_0000] {
            let forged = with_u32(&payload, offset, value);
            if forged == payload {
                continue;
            }
            let inspected = numeric_inspect(&forged);
            let decoded = numeric_decode(&forged, len);
            assert!(
                inspected.is_err() || decoded.is_err(),
                "NUM1 field @{offset} = {value} was accepted"
            );
            assert!(decoded.is_err(), "NUM1 field @{offset} = {value} decoded");
        }
    }
}

/// A forged `expected_size` larger than the declared size is rejected before allocation.
#[test]
fn num1_and_ts1_reject_expected_size_mismatch() {
    let (num1, len) = num1_payload();
    for expected in [0, len - 1, len + 1, usize::MAX] {
        assert!(numeric_decode(&num1, expected).is_err());
    }
    let (ts1, len) = ts1_payload();
    for expected in [0, len - 1, len + 1, usize::MAX] {
        assert!(ts1_decode(&ts1, expected).is_err());
    }
}

/// NUM1 first delta at the signed extremes and a bit width above the lane width.
#[test]
fn num1_first_delta_and_bit_width_extremes() {
    let (payload, _) = num1_payload();
    for delta in [i64::MIN, i64::MAX, 1i64 << 33, -(1i64 << 33)] {
        let mut forged = payload.clone();
        forged[24..32].copy_from_slice(&delta.to_le_bytes());
        assert!(numeric_inspect(&forged).is_err(), "first delta {delta}");
    }
    let mut forged = payload.clone();
    forged[6] = 33; // u32 lanes
    assert!(numeric_inspect(&forged).is_err());
    assert!(numeric_inspect(&payload[..NUMERIC_HEADER_SIZE - 1]).is_err());
}

/// TS1 value count and stream bit count (offsets 8, 24).
#[test]
fn ts1_count_fields_reject_zero_max_and_overflow() {
    let (payload, len) = ts1_payload();
    for offset in [8usize, 24] {
        for value in [0u32, 1, u32::MAX, u32::MAX - 7, 0x8000_0000] {
            let forged = with_u32(&payload, offset, value);
            if forged == payload {
                continue;
            }
            assert!(
                ts1_decode(&forged, len).is_err(),
                "TS1 field @{offset} = {value} decoded"
            );
        }
    }
}

/// TS1 tail length (offset 12), lane width (offset 6) and reserved bytes.
#[test]
fn ts1_small_fields_are_validated() {
    let (payload, _) = ts1_payload();
    for (offset, value) in [(12usize, 8u8), (12, 255), (6, 0), (6, 3), (6, 255), (13, 1)] {
        let mut forged = payload.clone();
        forged[offset] = value;
        assert!(
            ts1_inspect(&forged).is_err(),
            "TS1 byte @{offset} = {value}"
        );
    }
    assert!(ts1_inspect(&payload[..TIME_SERIES_HEADER_SIZE - 1]).is_err());
}
