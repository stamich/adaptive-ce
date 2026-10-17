//! Tests of `ace_cost` `time_series` (formerly the inline unit tests of `src`).

use ace_analysis::FloatWidth;
use ace_codecs::{ts1_encode_with, TimeSeriesLayout, TIME_SERIES_HEADER_SIZE};
use ace_cost::*;

/// Little-endian f64 bytes of `values`.
fn f64_bytes(values: impl IntoIterator<Item = f64>) -> Vec<u8> {
    values.into_iter().flat_map(f64::to_le_bytes).collect()
}

/// A constant series costs one bit per value and the estimate is near-exact.
#[test]
fn gorilla_constant_is_one_bit_per_value() {
    let data = f64_bytes(std::iter::repeat_n(21.5, 32_768));
    let estimate = estimate_gorilla(&data, FloatWidth::F64).unwrap();
    let actual = ts1_encode_with(&data, TimeSeriesLayout::GORILLA_F64)
        .unwrap()
        .len() as u64;
    assert_eq!(estimate.estimated_bytes, actual);
    assert_eq!(estimate.confidence, 1.0);
    assert!(!estimate.exact);
}

/// On a uniform smooth series the sample estimate lands within 10 % of the real size
/// (each window restarts without an XOR window, a small pessimistic bias).
#[test]
fn gorilla_smooth_estimate_is_close() {
    let mut value = 20.0f64;
    let mut state = 0x1234_5678u32;
    let data = f64_bytes((0..32_768).map(|_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        value += f64::from(state % 17) * 0.001 - 0.008;
        value
    }));
    let estimate = estimate_gorilla(&data, FloatWidth::F64).unwrap();
    let actual = ts1_encode_with(&data, TimeSeriesLayout::GORILLA_F64)
        .unwrap()
        .len() as f64;
    let error = (estimate.estimated_bytes as f64 - actual).abs() / actual;
    assert!(error < 0.10, "error {error}");
}

/// Short inputs fall back to the exact size; a tail is accounted for.
#[test]
fn gorilla_short_and_tail() {
    let data = f64_bytes([1.0, 2.0, 3.0]);
    let estimate = estimate_gorilla(&data, FloatWidth::F64).unwrap();
    assert!(estimate.estimated_bytes > TIME_SERIES_HEADER_SIZE as u64);
    let mut with_tail = f64_bytes(std::iter::repeat_n(5.0, 1024));
    with_tail.extend_from_slice(&[1, 2, 3]);
    let estimate = estimate_gorilla(&with_tail, FloatWidth::F64).unwrap();
    let actual = ts1_encode_with(&with_tail, TimeSeriesLayout::GORILLA_F64)
        .unwrap()
        .len() as u64;
    assert_eq!(estimate.estimated_bytes, actual);
}

/// RunDelta estimates are exact by construction.
#[test]
fn run_delta_is_exact() {
    let data: Vec<u8> = (0..65_536u32)
        .flat_map(|i| (1_000 + i / 200 * 7).to_le_bytes())
        .collect();
    let estimate = estimate_run_delta(&data, 4).unwrap();
    let actual = ts1_encode_with(&data, TimeSeriesLayout::run_delta(4))
        .unwrap()
        .len() as u64;
    assert_eq!(estimate.estimated_bytes, actual);
    assert!(estimate.exact);
    assert!(estimate.ratio_to(data.len()) < 0.01);
}

/// The ALP estimate is the exact payload size for both float widths.
#[test]
fn alp_estimate_is_exact() {
    let f64s: Vec<u8> = (0..30_000u32)
        .flat_map(|i| (f64::from(i * 7 % 2_000) / 100.0).to_le_bytes())
        .collect();
    let f32s: Vec<u8> = (0..30_000u32)
        .flat_map(|i| (f64::from(i * 7 % 900) as f32 / 10.0).to_le_bytes())
        .collect();
    for (data, width) in [(&f64s, FloatWidth::F64), (&f32s, FloatWidth::F32)] {
        let estimate = estimate_alp(data, width).unwrap();
        let actual = ts1_encode_with(data, alp_layout(width)).unwrap().len() as u64;
        assert_eq!(estimate.estimated_bytes, actual);
        assert!(estimate.exact);
        assert_eq!(estimate.layout, alp_layout(width));
        assert!(estimate.ratio_to(data.len()) < 0.35, "{estimate:?}");
    }
}
