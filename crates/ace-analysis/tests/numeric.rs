//! Tests of `ace_analysis` `numeric` (formerly the inline unit tests of `src`).

use ace_analysis::*;
use ace_core::NumericWidth;

/// Serializes values as little-endian u32 bytes.
fn u32_bytes(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Detects a monotonic u32 counter with high confidence.
#[test]
fn detects_u32_counter() {
    let bytes = u32_bytes(10_000u32..20_000);
    let profile = analyze_numeric(&bytes);
    assert!(profile.detected);
    assert_eq!(profile.width, Some(NumericWidth::U32));
    assert!(profile.monotonic_ratio > 0.99);
    assert!(profile.dod_bit_width_p95 <= 1);
}

/// Detects a u64 timestamp series, including one whose jitter needs more than 16 bits.
#[test]
fn detects_u64_timestamps_with_wide_jitter() {
    let mut value = 1_780_000_000_000_000_000u64;
    let bytes: Vec<u8> = (0..4096u64)
        .flat_map(|i| {
            value += 1_000_000 + (i % 5) * 100;
            value.to_le_bytes()
        })
        .collect();
    let profile = analyze_numeric(&bytes);
    assert!(profile.detected, "confidence {}", profile.confidence);
    assert_eq!(profile.width, Some(NumericWidth::U64));
    assert!(profile.delta_bit_width_p95 >= 20);
}

/// Detects a wrapping u16 counter and reports the wrap as an ordinary +1 step.
#[test]
fn detects_wrapping_u16_counter() {
    let bytes: Vec<u8> = (0..4096u32)
        .flat_map(|i| ((65_000 + i) as u16).to_le_bytes())
        .collect();
    let profile = analyze_numeric(&bytes);
    assert!(profile.detected);
    assert_eq!(profile.width, Some(NumericWidth::U16));
    assert!(
        profile.dod_zero_ratio > 0.99,
        "wrap must not break the fixed step"
    );
}

/// Existing 32-bit data keeps its u32 verdict after the u16 lane was added.
#[test]
fn u16_lane_does_not_steal_u32_data() {
    let bytes = u32_bytes((0..8192u32).map(|i| 1_000_000 + i * 7));
    assert_eq!(analyze_numeric(&bytes).width, Some(NumericWidth::U32));
}

/// Too little data yields the explicit non-numeric default.
#[test]
fn short_input_is_not_numeric() {
    let profile = analyze_numeric(&[1, 2, 3, 4, 5, 6, 7]);
    assert!(!profile.detected);
    assert_eq!(profile, NumericProfile::default());
}

/// Pseudorandom bytes must not be detected as numeric in any lane.
#[test]
fn random_bytes_are_not_numeric() {
    let mut x = 0x2545_f491_4f6c_dd1du64;
    let bytes: Vec<u8> = (0..64 * 1024)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    assert!(!analyze_numeric(&bytes).detected);
}

/// A few huge outliers show up as exception ratio while the p95 envelope stays tiny.
#[test]
fn outliers_raise_exception_ratio() {
    let mut value = 0u32;
    let bytes = u32_bytes((0..4096u32).map(|i| {
        value = value.wrapping_add(if i % 100 == 0 { 1_000_000 } else { 3 });
        value
    }));
    let profile = analyze_numeric(&bytes);
    assert!(profile.exception_ratio > 0.0);
    assert!(profile.delta_bit_width_p95 <= 4);
}
