//! NUM1 property tests (ACE 0.4.6 hardening).
//!
//! * every `(width, mode)` pair round-trips arbitrary bytes, including a partial tail lane;
//! * decoding arbitrary bytes never panics and never returns a wrong-length result.
//!
//! The case budget defaults to 64 and follows `PROPTEST_CASES` (release CI: 10 000).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_codecs::{numeric_decode, numeric_encode, numeric_encode_with, NumericMode};
use ace_core::NumericWidth;
use proptest::prelude::*;

/// Case budget: `PROPTEST_CASES` or `default`.
fn budget(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Strategy over every lane width.
fn width() -> impl Strategy<Value = NumericWidth> {
    prop_oneof![
        Just(NumericWidth::U16),
        Just(NumericWidth::U32),
        Just(NumericWidth::U64)
    ]
}

/// Strategy over every NUM1 mode.
fn mode() -> impl Strategy<Value = NumericMode> {
    prop_oneof![
        Just(NumericMode::FrameOfReference),
        Just(NumericMode::Delta),
        Just(NumericMode::DeltaOfDelta)
    ]
}

/// Little-endian counter bytes with a random start, step and tail (structured numeric data).
fn counter_bytes() -> impl Strategy<Value = Vec<u8>> {
    (any::<u64>(), 0u64..1_000, 2usize..2_000, 0usize..8).prop_map(|(start, step, count, tail)| {
        let mut bytes: Vec<u8> = (0..count as u64)
            .flat_map(|i| start.wrapping_add(i.wrapping_mul(step)).to_le_bytes())
            .collect();
        bytes.extend(std::iter::repeat_n(0xA5, tail));
        bytes
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(budget(64)))]

    /// Explicit `(width, mode)` encoding of arbitrary bytes round-trips exactly.
    #[test]
    fn explicit_mode_roundtrip(
        bytes in proptest::collection::vec(any::<u8>(), 16..4_096usize),
        width in width(),
        mode in mode(),
    ) {
        let encoded = numeric_encode_with(&bytes, width, mode).unwrap();
        prop_assert_eq!(numeric_decode(&encoded, bytes.len()).unwrap(), bytes);
    }

    /// Automatic encoding of structured counters round-trips exactly.
    #[test]
    fn automatic_roundtrip_on_counters(bytes in counter_bytes()) {
        let encoded = numeric_encode(&bytes).unwrap();
        prop_assert_eq!(numeric_decode(&encoded, bytes.len()).unwrap(), bytes);
    }

    /// Arbitrary payloads are rejected or decode to exactly `expected` bytes; never a panic.
    #[test]
    fn arbitrary_payload_never_panics(
        payload in proptest::collection::vec(any::<u8>(), 0..512usize),
        expected in 0usize..65_536usize,
    ) {
        if let Ok(decoded) = numeric_decode(&payload, expected) {
            prop_assert_eq!(decoded.len(), expected);
        }
    }

    /// Single-byte corruption of a valid payload is rejected or keeps the declared length.
    #[test]
    fn corrupted_payload_never_panics(bytes in counter_bytes(), position in any::<usize>(), flip in 1u8..=255) {
        let mut encoded = numeric_encode(&bytes).unwrap();
        let index = position % encoded.len();
        encoded[index] ^= flip;
        if let Ok(decoded) = numeric_decode(&encoded, bytes.len()) {
            prop_assert_eq!(decoded.len(), bytes.len());
        }
    }
}
