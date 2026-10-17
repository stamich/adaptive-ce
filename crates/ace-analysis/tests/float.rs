//! Tests of `ace_analysis` `float` (formerly the inline unit tests of `src`).

use ace_analysis::*;

/// Encodes `values` as little-endian f64 bytes.
fn f64_bytes(values: impl IntoIterator<Item = f64>) -> Vec<u8> {
    values.into_iter().flat_map(f64::to_le_bytes).collect()
}

/// Encodes `values` as little-endian f32 bytes.
fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

/// Windows cover start, thirds and end, and merge when the block is short.
#[test]
fn sample_windows_are_deterministic_and_merged() {
    assert_eq!(
        lane_sample_windows(32_768),
        vec![0..64, 10_922..10_986, 21_845..21_909, 32_704..32_768]
    );
    assert_eq!(lane_sample_windows(100), vec![0..100]);
    assert_eq!(lane_sample_windows(10), vec![0..10]);
    assert!(lane_sample_windows(0).is_empty());
}

/// A slowly varying f64 series is admitted as f64.
#[test]
fn smooth_f64_is_admitted() {
    let data = f64_bytes((0..32_768).map(|i| 20.0 + f64::from(i) * 0.001));
    let prefilter = float_prefilter(&data);
    let profile = prefilter.admitted.expect("admitted");
    assert_eq!(profile.width, FloatWidth::F64);
    assert!(profile.exponent_distinct <= 4);
}

/// A slowly varying f32 series is admitted as f32, not f64.
#[test]
fn smooth_f32_is_admitted_as_f32() {
    let data = f32_bytes((0..65_536).map(|i| 20.0 + i as f32 * 0.25));
    let profile = float_prefilter(&data).admitted.expect("admitted");
    assert_eq!(profile.width, FloatWidth::F32);
}

/// A constant series is admitted with an all-zero XOR stream.
#[test]
fn constant_is_admitted() {
    let data = f64_bytes(std::iter::repeat_n(21.5, 4096));
    let profile = float_prefilter(&data).admitted.expect("admitted");
    assert_eq!(profile.xor_zero_ratio, 1.0);
    assert_eq!(profile.mean_xor_meaningful_bits, 0.0);
}

/// Integers read as floats are rejected (subnormals / implausible magnitudes).
#[test]
fn integers_are_rejected() {
    let timestamps: Vec<u8> = (0..32_768u64)
        .flat_map(|i| (1_700_000_000_000 + i * 1000).to_le_bytes())
        .collect();
    assert_eq!(
        float_prefilter(&timestamps).rejection,
        Some(FloatRejection::Subnormal)
    );
    let nanos: Vec<u8> = (0..32_768u64)
        .flat_map(|i| (1_700_000_000_000_000_000 + i * 1_000_017).to_le_bytes())
        .collect();
    let prefilter = float_prefilter(&nanos);
    assert_eq!(prefilter.admitted, None);
    assert_eq!(
        prefilter.rejection,
        Some(FloatRejection::ImplausibleMagnitude)
    );
}

/// Zero padding, NaN-heavy and random data are rejected.
#[test]
fn degenerate_inputs_are_rejected() {
    assert_eq!(
        float_prefilter(&[0u8; 65_536]).rejection,
        Some(FloatRejection::ZeroDominated)
    );
    let nans = f64_bytes((0..8192).map(|i| if i % 4 == 0 { f64::NAN } else { 1.5 }));
    assert_eq!(
        float_prefilter(&nans).rejection,
        Some(FloatRejection::NonFinite)
    );
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let random: Vec<u8> = (0..8192)
        .flat_map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()
        })
        .collect();
    let prefilter = float_prefilter(&random);
    assert_eq!(prefilter.admitted, None);
    assert_eq!(prefilter.rejection, Some(FloatRejection::ExponentSpread));
    assert_eq!(
        float_prefilter(&[1u8; 100]).rejection,
        Some(FloatRejection::TooShort)
    );
}

/// Meaningful bits are counted inside the lane, not inside the u64 accumulator.
#[test]
fn meaningful_bits_are_lane_relative() {
    // Alternating 1.0 / -1.0 flips only the sign bit: one meaningful bit per transition.
    let data = f32_bytes((0..4096).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }));
    let profile = float_profile(&data, FloatWidth::F32).expect("profile");
    assert_eq!(profile.mean_xor_meaningful_bits, 1.0);
    assert_eq!(profile.sign_change_ratio, 1.0);
}
