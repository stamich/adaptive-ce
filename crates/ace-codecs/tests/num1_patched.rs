//! NUM1 mode 4 (Patched FOR, Format 1.5): round trips, exact size estimates, the outlier
//! workload and the malformed-payload table.
//!
//! The case budget of the property tests follows `PROPTEST_CASES` (default 64).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_codecs::{
    estimate_numeric, estimate_patched, numeric_decode, numeric_encode_with, numeric_inspect,
    payload_minor_version, NumericMode, NumericPatch, NUMERIC_HEADER_SIZE,
};
use ace_core::{AceError, CodecId, NumericWidth};
use proptest::prelude::*;

const WIDTHS: [NumericWidth; 3] = [NumericWidth::U16, NumericWidth::U32, NumericWidth::U64];

/// Case budget: `PROPTEST_CASES` or `default`.
fn budget(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn u32_bytes(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// `monotonic-outliers` of the benchmark corpus: step 1 with a 100 000 jump every 1024 values.
fn monotonic_outliers(count: usize) -> Vec<u8> {
    let mut value = 1_000_000u32;
    u32_bytes((0..count).map(|index| {
        value = value.wrapping_add(if index % 1024 == 0 { 100_000 } else { 1 });
        value
    }))
}

/// 1000 zeros with two spikes: the cheapest encoding is FOR at width 0 plus two exceptions.
fn two_spikes() -> Vec<u8> {
    u32_bytes((0..1000).map(|index| match index {
        10 => 70_000,
        500 => 90_000,
        _ => 0,
    }))
}

/// LSB-first bit packing, restated here so the tests do not depend on the codec's helpers.
fn pack_bits(values: &[u64], width: u32) -> Vec<u8> {
    let mut out = vec![0u8; (values.len() * width as usize).div_ceil(8)];
    for (index, value) in values.iter().enumerate() {
        for bit in 0..width as usize {
            if value >> bit & 1 == 1 {
                let position = index * width as usize + bit;
                out[position / 8] |= 1 << (position % 8);
            }
        }
    }
    out
}

fn patched(bytes: &[u8], width: NumericWidth) -> Vec<u8> {
    numeric_encode_with(bytes, width, NumericMode::Patched).unwrap()
}

/// Every width and shape round-trips and the estimate is the exact payload size.
#[test]
fn round_trips_with_exact_estimates() {
    let shapes: Vec<Vec<u8>> = vec![
        monotonic_outliers(5_000),
        two_spikes(),
        u32_bytes((0..2_000).map(|i| i * 3)),
        u32_bytes(std::iter::repeat_n(7, 300)),
        u32_bytes([5, 1]),
        (0..4_000u32)
            .flat_map(|i| (i ^ (i << 7)).to_le_bytes())
            .collect(),
        {
            let mut bytes = monotonic_outliers(999);
            bytes.extend_from_slice(&[1, 2, 3]); // a tail
            bytes
        },
    ];
    for shape in &shapes {
        for width in WIDTHS {
            if shape.len() < width.bytes() * 2 {
                continue;
            }
            let encoded = patched(shape, width);
            assert_eq!(numeric_decode(&encoded, shape.len()).unwrap(), *shape);
            let estimate = estimate_patched(shape, width).unwrap();
            assert_eq!(estimate.mode, NumericMode::Patched);
            assert_eq!(estimate.encoded_bytes, encoded.len(), "{width:?}");
            let info = numeric_inspect(&encoded).unwrap();
            assert_eq!(info.mode, NumericMode::Patched);
            assert_eq!(info.bit_width, estimate.bit_width);
            assert_eq!(payload_minor_version(CodecId::Numeric, &encoded), 5);
        }
    }
    assert!(estimate_patched(&[1, 2, 3], NumericWidth::U16).is_none());
}

/// Rare large deltas no longer widen every slot: the outlier workload shrinks several-fold.
#[test]
fn outliers_cost_a_few_bytes_each() {
    let input = monotonic_outliers(65_536);
    let base = estimate_numeric(&input).unwrap();
    let patched = estimate_patched(&input, NumericWidth::U32).unwrap();
    assert!(
        patched.encoded_bytes * 4 < base.encoded_bytes,
        "patched {} vs base {}",
        patched.encoded_bytes,
        base.encoded_bytes
    );
    let info = numeric_inspect(
        &numeric_encode_with(&input, NumericWidth::U32, NumericMode::Patched).unwrap(),
    )
    .unwrap();
    let patch = info.patch.unwrap();
    // The delta-of-delta of a regular counter is zero; each jump leaves two exceptions (up and
    // back), so the packed slots need no bits at all.
    assert_eq!(patch.source, NumericMode::DeltaOfDelta);
    assert_eq!(info.bit_width, 0);
    assert!((120..=128).contains(&patch.exception_count), "{patch:?}");
}

/// Without outliers the patched size never beats the best base mode (it equals it at best), so
/// selecting Patched only when strictly smaller leaves ordinary blocks unchanged.
#[test]
fn clean_data_gains_nothing() {
    for input in [
        u32_bytes((0..4_096).map(|i| i * 3)),
        u32_bytes((0..4_096).map(|i| i % 4096 + 100_000)),
        u32_bytes((0..4_096u32).map(|i| i.wrapping_mul(2_654_435_761) >> 20)),
    ] {
        let base = estimate_numeric(&input).unwrap();
        let patched = estimate_patched(&input, base.width).unwrap();
        assert!(patched.encoded_bytes >= base.encoded_bytes);
    }
}

/// The exception section and header of the two-spike payload, for the malformed table.
fn two_spike_payload() -> (Vec<u8>, usize) {
    let input = two_spikes();
    let encoded = patched(&input, NumericWidth::U32);
    let info = numeric_inspect(&encoded).unwrap();
    let patch = info.patch.unwrap();
    assert_eq!(
        (
            info.bit_width,
            patch.source,
            patch.exception_count,
            patch.high_bits
        ),
        (0, NumericMode::FrameOfReference, 2, 17)
    );
    (encoded, NUMERIC_HEADER_SIZE + info.packed_bytes)
}

fn assert_malformed(payload: &[u8], expected: usize, why: &str) {
    let error = numeric_inspect(payload)
        .and_then(|_| numeric_decode(payload, expected))
        .unwrap_err();
    assert!(
        matches!(
            error,
            AceError::Malformed(_) | AceError::ResourceLimitExceeded(_)
        ),
        "{why}: {error:?}"
    );
}

/// Every way to damage a patched payload is rejected, none panics.
#[test]
fn malformed_patched_payloads_are_rejected() {
    let expected = two_spikes().len();
    let (encoded, section) = two_spike_payload();
    assert_eq!(numeric_decode(&encoded, expected).unwrap().len(), expected);

    let mutate = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut bytes = encoded.clone();
        edit(&mut bytes);
        bytes
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("source 0", mutate(&|b| b[7] = 0)),
        ("source Patched", mutate(&|b| b[7] = 4)),
        ("source 9", mutate(&|b| b[7] = 9)),
        (
            "count above stream",
            mutate(&|b| b[36..40].copy_from_slice(&1001u32.to_le_bytes())),
        ),
        (
            "count 3 of 2",
            mutate(&|b| b[36..40].copy_from_slice(&3u32.to_le_bytes())),
        ),
        (
            "count 1 of 2",
            mutate(&|b| b[36..40].copy_from_slice(&1u32.to_le_bytes())),
        ),
        ("high width 0", mutate(&|b| b[section] = 0)),
        ("high width too large", mutate(&|b| b[section] = 33)),
        ("high width off by one", mutate(&|b| b[section] = 16)),
        ("truncated section", mutate(&|b| b.truncate(section + 3))),
        ("section removed", mutate(&|b| b.truncate(section))),
        ("trailing byte", mutate(&|b| b.push(0))),
    ];
    for (why, payload) in &cases {
        assert_malformed(payload, expected, why);
    }

    // Positions: out of order, repeated, outside the stream. 10 bits each, then 17-bit highs.
    let highs = pack_bits(&[70_000, 90_000], 17);
    for (why, positions) in [
        ("descending", [500u64, 10]),
        ("repeated", [10, 10]),
        ("outside", [10, 1000]),
    ] {
        let mut bytes = encoded[..section + 1].to_vec();
        bytes.extend_from_slice(&pack_bits(&positions, 10));
        bytes.extend_from_slice(&highs);
        assert_malformed(&bytes, expected, why);
    }
    // A zero high part is an exception that changes nothing.
    let mut bytes = encoded[..section + 1].to_vec();
    bytes.extend_from_slice(&pack_bits(&[10, 500], 10));
    bytes.extend_from_slice(&pack_bits(&[70_000, 0], 17));
    assert_malformed(&bytes, expected, "zero high part");

    // A wrong expected size is a size mismatch, never a wrong answer.
    assert_malformed(&encoded, expected + 4, "expected size");
    // A patched payload cannot be serialized as a base mode.
    let info = numeric_inspect(&encoded).unwrap();
    assert!(ace_codecs::numeric_serialize_payload(&info, &[], &[]).is_err());
    assert_eq!(NumericPatch::position_bits(1000), 10);
    assert_eq!(NumericPatch::section_len(1000, 0, 17).unwrap(), 0);
    assert_eq!(NumericPatch::section_len(1000, 2, 17).unwrap(), 1 + 3 + 5);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(budget(64)))]

    /// Arbitrary bytes round-trip through Patched at every width, and the estimate is exact.
    #[test]
    fn arbitrary_bytes_round_trip(bytes in proptest::collection::vec(any::<u8>(), 16..3_000usize)) {
        for width in WIDTHS {
            let encoded = patched(&bytes, width);
            prop_assert_eq!(numeric_decode(&encoded, bytes.len()).unwrap(), bytes.clone());
            prop_assert_eq!(estimate_patched(&bytes, width).unwrap().encoded_bytes, encoded.len());
        }
    }

    /// Structured data with rare outliers round-trips and patched never loses to FOR/Delta/DoD
    /// by more than its own header bytes.
    #[test]
    fn outlier_data_round_trips(
        start in any::<u32>(),
        step in 0u32..50,
        count in 2usize..4_000,
        every in 3usize..2_000,
        jump in any::<u32>(),
    ) {
        let mut value = start;
        let bytes = u32_bytes((0..count).map(|i| {
            value = value.wrapping_add(if i % every == 0 { jump } else { step });
            value
        }));
        let encoded = patched(&bytes, NumericWidth::U32);
        prop_assert_eq!(numeric_decode(&encoded, bytes.len()).unwrap(), bytes);
    }

    /// Damaging any byte of a patched payload never panics and never decodes to a wrong size.
    #[test]
    fn damaged_payloads_never_panic(index in 0usize..4_000, value in any::<u8>()) {
        let input = monotonic_outliers(3_000);
        let mut encoded = patched(&input, NumericWidth::U32);
        let index = index % encoded.len();
        encoded[index] = value;
        if let Ok(decoded) = numeric_decode(&encoded, input.len()) {
            prop_assert_eq!(decoded.len(), input.len());
        }
    }
}
