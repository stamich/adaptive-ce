//! Tests of `ace_analysis` `numeric_prefilter` (formerly the inline unit tests of `src`).

use ace_analysis::*;
use ace_core::NumericWidth;

/// Serializes values as little-endian u32 bytes.
fn u32_bytes(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Grouped validation: a single broken step anywhere (inside a group of eight, at a group
/// boundary or in the remainder) rejects the block; every length is handled.
#[test]
fn strong_evidence_checks_every_position() {
    for count in [16usize, 17, 23, 24, 25, 1000, 1003] {
        let values: Vec<u32> = (0..count as u32).map(|i| 500 + 7 * i).collect();
        let evidence = strong_numeric_evidence(&u32_bytes(values.clone()), NumericWidth::U32)
            .expect("valid fixed-step block");
        assert_eq!((evidence.value_count, evidence.first_delta), (count, 7));
        for broken in [2, 3, 9, 10, 17, count - 1]
            .into_iter()
            .filter(|&b| b < count)
        {
            let mut bad = values.clone();
            bad[broken] += 1;
            assert_eq!(
                strong_numeric_evidence(&u32_bytes(bad), NumericWidth::U32),
                None,
                "count {count}, broken {broken}"
            );
        }
    }
}

/// Monotonic fixed-step counters must qualify for the strong numeric route.
#[test]
fn fixed_step_u32_is_strong_numeric() {
    let mut value = 10_000u32;
    let bytes = u32_bytes((0..4096).map(|_| {
        value += 3;
        value
    }));
    let result = numeric_prefilter(&bytes);
    assert!(result.strong_numeric);
    assert_eq!(result.width_hint, Some(NumericWidth::U32));
    assert!(validate_strong_numeric(&bytes, NumericWidth::U32));
}

/// Evidence must carry exactly the values the direct encoder needs.
#[test]
fn evidence_reports_first_value_step_and_tail() {
    let mut bytes = u32_bytes((0..100u32).map(|i| 50 + i * 4));
    bytes.extend_from_slice(&[9, 9]);
    let evidence = strong_numeric_evidence(&bytes, NumericWidth::U32).unwrap();
    assert_eq!(evidence.first_value, 50);
    assert_eq!(evidence.first_delta, 4);
    assert_eq!(evidence.value_count, 100);
    assert_eq!(evidence.tail_bytes, 2);
}

/// A single outlier breaks the strict fixed-step invariant.
#[test]
fn outlier_is_not_strong_fixed_step() {
    let mut value = 10_000u32;
    let bytes = u32_bytes((0..4096).map(|i| {
        value = value.wrapping_add(if i == 2048 { 7 } else { 3 });
        value
    }));
    assert!(strong_numeric_evidence(&bytes, NumericWidth::U32).is_none());
}

/// Sawtooth/reset data cannot enter NumericFast even if local deltas are small.
#[test]
fn sawtooth_is_not_strong_fixed_step() {
    let bytes = u32_bytes((0..4096u32).map(|i| i % 256));
    assert!(strong_numeric_evidence(&bytes, NumericWidth::U32).is_none());
}

/// Constant zero blocks are deliberately left to generic RLE/RAW routing.
#[test]
fn zero_block_is_not_strong_numeric() {
    let bytes = vec![0u8; 64 * 1024];
    assert!(!numeric_prefilter(&bytes).strong_numeric);
}

/// Deterministic high-entropy bytes must not qualify as likely numeric.
#[test]
fn random_bytes_are_not_likely_numeric() {
    let mut x = 0x9e37_79b9u32;
    let mut bytes = Vec::new();
    for _ in 0..64 * 1024 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push((x & 0xff) as u8);
    }
    assert!(!numeric_prefilter(&bytes).likely_numeric);
}

/// REGRESSION (0.4.5): u64 nanosecond timestamps with ~21-bit jitter are likely numeric.
///
/// Their deltas need 17–32 bits, which the old fixed 16-bit "small" limit rejected, sending
/// the block down the Generic route where Numeric was diagnostic-only (1.64× instead of ~6×).
#[test]
fn wide_jitter_u64_timestamps_are_likely_numeric() {
    let mut value = 1_780_000_000_000_000_000u64;
    let bytes: Vec<u8> = (0..4096u64)
        .flat_map(|i| {
            value += 1_000_000 + (i % 5) * 100;
            value.to_le_bytes()
        })
        .collect();
    let result = numeric_prefilter(&bytes);
    assert!(result.likely_numeric, "confidence {}", result.confidence);
    assert!(!result.strong_numeric, "jitter must not enter NumericFast");
    assert_eq!(result.width_hint, Some(NumericWidth::U64));
}

/// The same 21-bit deltas stored in a u32 lane remain *not* small (limit stays 16 bits).
#[test]
fn small_limit_is_relative_to_lane_width() {
    assert_eq!(NumericWidth::U16.small_delta_bits(), 8);
    assert_eq!(NumericWidth::U32.small_delta_bits(), 16);
    assert_eq!(NumericWidth::U64.small_delta_bits(), 32);
    let mut value = 0u32;
    let bytes = u32_bytes((0..2048u32).map(|i| {
        value = value.wrapping_add(1_000_000 + (i % 5) * 100);
        value
    }));
    assert!(!numeric_prefilter(&bytes).likely_numeric);
}

/// A wrapping u16 counter is a perfect step *within the ring*, but the strict NumericFast
/// validator requires non-decreasing values, so it must fall back to NumericGeneral.
#[test]
fn wrapping_u16_counter_is_likely_but_not_fast() {
    let bytes: Vec<u8> = (0..4096u32)
        .flat_map(|i| ((65_000 + i) as u16).to_le_bytes())
        .collect();
    let result = numeric_prefilter(&bytes);
    assert!(result.likely_numeric);
    assert_eq!(result.width_hint, Some(NumericWidth::U16));
    assert!(strong_numeric_evidence(&bytes, NumericWidth::U16).is_none());
}

/// A non-wrapping fixed-step u16 counter does qualify for NumericFast.
#[test]
fn fixed_step_u16_is_strong_numeric() {
    // 4096 * 3 + 100 = 12_388 < 65_535, so the sequence never wraps.
    let bytes: Vec<u8> = (0..4096u32)
        .map(|i| (100 + i * 3) as u16)
        .flat_map(u16::to_le_bytes)
        .collect();
    let evidence = strong_numeric_evidence(&bytes, NumericWidth::U16).unwrap();
    assert_eq!(evidence.first_delta, 3);
    assert_eq!(evidence.value_count, 4096);
}
