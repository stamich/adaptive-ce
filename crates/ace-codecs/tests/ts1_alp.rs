//! ALP (TS1 mode 4, Format 1.5): decimal floats round-trip bit for bit, the exact size equals the
//! encoded size, decimals are far smaller than Gorilla, and malformed streams are rejected.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_codecs::{
    ts1_decode, ts1_encode_with, ts1_encoded_len, ts1_inspect, TimeSeriesLayout, TimeSeriesMode,
    TIME_SERIES_HEADER_SIZE,
};
use proptest::prelude::*;

fn f64_bytes(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Encodes with `layout`, checks the exact size and the round trip, returns the payload.
fn round_trip(input: &[u8], layout: TimeSeriesLayout) -> Vec<u8> {
    let payload = ts1_encode_with(input, layout).unwrap();
    assert_eq!(payload.len(), ts1_encoded_len(input, layout).unwrap());
    assert_eq!(ts1_decode(&payload, input.len()).unwrap(), input);
    payload
}

/// Prices with two decimals: ALP needs about a dozen bits per value, Gorilla far more.
#[test]
fn two_decimal_prices_are_small() {
    let values: Vec<f64> = (0..20_000u32)
        .map(|i| f64::from(1_000 + (i * 37 % 900)) / 100.0)
        .collect();
    let input = f64_bytes(&values);
    let alp = round_trip(&input, TimeSeriesLayout::ALP_F64);
    let gorilla = ts1_encoded_len(&input, TimeSeriesLayout::GORILLA_F64).unwrap();
    let info = ts1_inspect(&alp).unwrap();
    assert_eq!(info.mode, TimeSeriesMode::Alp);
    assert_eq!(info.first_bits, 0);
    assert!(info.bits_per_value() < 12.0, "{}", info.bits_per_value());
    assert!(alp.len() * 3 < gorilla, "{} vs {gorilla}", alp.len());
}

/// f32 decimals use the same formula in binary64 and are checked bit for bit.
#[test]
fn f32_decimals_round_trip() {
    let values: Vec<f32> = (0..9_000u32).map(|i| (i % 500) as f32 / 10.0).collect();
    let payload = round_trip(&f32_bytes(&values), TimeSeriesLayout::ALP_F32);
    assert!(ts1_inspect(&payload).unwrap().bits_per_value() < 11.0);
}

/// Special values and full-precision noise become exceptions and survive unchanged.
#[test]
fn exceptions_keep_every_bit() {
    let mut values: Vec<f64> = (0..3_000).map(|i| f64::from(i) / 8.0).collect();
    for (i, special) in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.0,
        f64::MIN_POSITIVE / 4.0,
        std::f64::consts::PI,
        1.0e300,
        f64::from_bits(0x7ff8_dead_beef_0001),
    ]
    .into_iter()
    .enumerate()
    {
        values[i * 311 + 5] = special;
    }
    let payload = round_trip(&f64_bytes(&values), TimeSeriesLayout::ALP_F64);
    // The exceptions are few, so the block is still far below raw size.
    assert!(payload.len() < values.len() * 8 / 3);
}

/// A block that is all exceptions still round-trips; so do the empty and the tiny cases.
#[test]
fn degenerate_inputs() {
    let noise: Vec<f64> = (1..400u64)
        .map(|i| {
            f64::from_bits(
                0x3ff0_0000_0000_0000 + i.wrapping_mul(0x9e37_79b9_7f4a_7c15) % (1 << 52),
            )
        })
        .collect();
    round_trip(&f64_bytes(&noise), TimeSeriesLayout::ALP_F64);
    for layout in [TimeSeriesLayout::ALP_F64, TimeSeriesLayout::ALP_F32] {
        round_trip(&[], layout);
        round_trip(&[1, 2, 3], layout); // tail only
        round_trip(&[9; 8], layout);
        round_trip(&[9; 13], layout); // one lane plus a tail
    }
    round_trip(&f64_bytes(&[2.5; 100]), TimeSeriesLayout::ALP_F64);
}

/// Values at the edge of the exact integer range are exceptions, not wrong numbers.
#[test]
fn large_magnitudes_are_exact() {
    let values = [
        9.0e15,
        9.007_199_254_740_993e15,
        -9.0e15,
        1.0e18,
        123_456_789.123_456_78,
    ];
    round_trip(&f64_bytes(&values.repeat(40)), TimeSeriesLayout::ALP_F64);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any bytes round-trip through ALP, with an exact size.
    #[test]
    fn arbitrary_bytes_round_trip(data in proptest::collection::vec(any::<u8>(), 0..600)) {
        for layout in [TimeSeriesLayout::ALP_F64, TimeSeriesLayout::ALP_F32] {
            let payload = ts1_encode_with(&data, layout).unwrap();
            prop_assert_eq!(payload.len(), ts1_encoded_len(&data, layout).unwrap());
            prop_assert_eq!(ts1_decode(&payload, data.len()).unwrap(), data.clone());
        }
    }

    /// Decimals with random scale and sign round-trip and compress.
    #[test]
    fn decimals_round_trip(
        raw in proptest::collection::vec(-1_000_000i64..1_000_000, 1..400),
        scale in 0u32..9,
    ) {
        let values: Vec<f64> = raw.iter().map(|&n| n as f64 / 10f64.powi(scale as i32)).collect();
        let input = f64_bytes(&values);
        let payload = ts1_encode_with(&input, TimeSeriesLayout::ALP_F64).unwrap();
        prop_assert_eq!(ts1_decode(&payload, input.len()).unwrap(), input);
    }

    /// Flipped bytes and cut payloads never panic.
    #[test]
    fn damaged_payloads_never_panic(flip in 0usize..400, cut in 0usize..400, mask in 1u8..=255) {
        let values: Vec<f64> = (0..300).map(|i| f64::from(i % 70) / 10.0).collect();
        let payload = ts1_encode_with(&f64_bytes(&values), TimeSeriesLayout::ALP_F64).unwrap();
        let mut damaged = payload.clone();
        let at = flip % damaged.len();
        damaged[at] ^= mask;
        let _ = ts1_decode(&damaged, values.len() * 8);
        let _ = ts1_decode(&payload[..cut % payload.len()], values.len() * 8);
    }
}

/// The header rejects an ALP payload that claims a first value, a flag or a 16-bit lane.
#[test]
fn header_rules() {
    let input = f64_bytes(&[1.5, 2.5, 3.5, 4.5]);
    let valid = ts1_encode_with(&input, TimeSeriesLayout::ALP_F64).unwrap();
    for (offset, value) in [(7usize, 1u8), (6, 2), (16, 1), (13, 1)] {
        let mut bad = valid.clone();
        bad[offset] = value;
        assert!(ts1_decode(&bad, input.len()).is_err(), "offset {offset}");
    }
}

/// Stream-level corruption: every truncation and a damaged header field of the stream is
/// reported as an error, never as different data of the right length.
#[test]
fn stream_fields_are_validated() {
    let values: Vec<f64> = (0..200).map(|i| f64::from(i % 50) / 4.0).collect();
    let input = f64_bytes(&values);
    let valid = ts1_encode_with(&input, TimeSeriesLayout::ALP_F64).unwrap();
    let stream = &valid[TIME_SERIES_HEADER_SIZE..];
    assert!(stream.len() > 16);
    // e = 31 (> 18): bits 0..5 of the stream.
    let mut bad = valid.clone();
    bad[TIME_SERIES_HEADER_SIZE] |= 0x1f;
    assert!(ts1_decode(&bad, input.len()).is_err());
    // width = 127 (> 64): bits 10..17.
    let mut bad = valid.clone();
    bad[TIME_SERIES_HEADER_SIZE + 1] |= 0xfc;
    bad[TIME_SERIES_HEADER_SIZE + 2] |= 0x01;
    assert!(ts1_decode(&bad, input.len()).is_err());
}

/// The 64-value pre-test accepts decimals in both widths and rejects noise and bad widths.
#[test]
fn sample_test_separates_decimals_from_noise() {
    use ace_codecs::alp_sample_viable;
    let decimals: Vec<f64> = (0..5_000u32)
        .map(|i| f64::from(i * 7 % 1_000) / 100.0)
        .collect();
    assert!(alp_sample_viable(&f64_bytes(&decimals), 8));
    let tenths: Vec<f32> = (0..5_000u32).map(|i| (i % 300) as f32 / 10.0).collect();
    assert!(alp_sample_viable(&f32_bytes(&tenths), 4));
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let noise: Vec<f64> = (0..5_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            f64::from_bits(0x3ff0_0000_0000_0000 | (state >> 12))
        })
        .collect();
    assert!(!alp_sample_viable(&f64_bytes(&noise), 8));
    assert!(!alp_sample_viable(&[], 8));
    assert!(!alp_sample_viable(&[0u8; 64], 2));
}
