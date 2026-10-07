//! TS1 property tests (ACE 0.5.0).
//!
//! * every layout round-trips arbitrary bytes bit-exactly and its exact size estimate equals
//!   the encoded length;
//! * float series built from arbitrary IEEE bit patterns (NaN payloads, infinities,
//!   subnormals) are restored bit for bit;
//! * decoding arbitrary or corrupted payloads never panics and never returns a wrong length.
//!
//! The case budget defaults to 64 and follows `PROPTEST_CASES` (release CI: 10 000).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_codecs::{ts1_decode, ts1_encode, ts1_encode_with, ts1_encoded_len, TimeSeriesLayout};
use proptest::prelude::*;

/// Case budget: `PROPTEST_CASES` or `default`.
fn budget(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Strategy over every TS1 layout.
fn layout() -> impl Strategy<Value = TimeSeriesLayout> {
    proptest::sample::select(TimeSeriesLayout::ALL.to_vec())
}

/// Slowly drifting f64 series with injected special bit patterns.
fn float_series() -> impl Strategy<Value = Vec<u8>> {
    (
        proptest::collection::vec(any::<u64>(), 1..16),
        2usize..1_500,
        any::<f64>(),
    )
        .prop_map(|(specials, count, start)| {
            let mut value = if start.is_finite() { start } else { 1.0 };
            (0..count)
                .flat_map(|i| {
                    let bits = if i % 97 == 0 {
                        specials[i % specials.len()]
                    } else {
                        value += (i % 5) as f64 * 0.125;
                        value.to_bits()
                    };
                    bits.to_le_bytes()
                })
                .collect()
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(budget(64)))]

    /// Every layout round-trips arbitrary bytes and predicts its exact size.
    #[test]
    fn every_layout_roundtrips_arbitrary_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..4_096usize),
        layout in layout(),
    ) {
        let encoded = ts1_encode_with(&bytes, layout).unwrap();
        prop_assert_eq!(ts1_encoded_len(&bytes, layout).unwrap(), encoded.len());
        prop_assert_eq!(ts1_decode(&encoded, bytes.len()).unwrap(), bytes);
    }

    /// Float series with special values are restored bit for bit by the automatic encoder.
    #[test]
    fn float_series_are_bit_exact(bytes in float_series()) {
        let encoded = ts1_encode(&bytes).unwrap();
        prop_assert_eq!(ts1_decode(&encoded, bytes.len()).unwrap(), bytes);
    }

    /// Arbitrary payloads are rejected or decode to exactly `expected` bytes; never a panic.
    #[test]
    fn arbitrary_payload_never_panics(
        payload in proptest::collection::vec(any::<u8>(), 0..512usize),
        expected in 0usize..65_536usize,
    ) {
        if let Ok(decoded) = ts1_decode(&payload, expected) {
            prop_assert_eq!(decoded.len(), expected);
        }
    }

    /// Single-byte corruption of a valid payload is rejected or keeps the declared length.
    #[test]
    fn corrupted_payload_never_panics(
        bytes in float_series(),
        position in any::<usize>(),
        flip in 1u8..=255,
    ) {
        let mut encoded = ts1_encode(&bytes).unwrap();
        let index = position % encoded.len();
        encoded[index] ^= flip;
        if let Ok(decoded) = ts1_decode(&encoded, bytes.len()) {
            prop_assert_eq!(decoded.len(), bytes.len());
        }
    }
}
