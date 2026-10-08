//! TS1 size estimators (ACE 0.5.0).
//!
//! | Candidate | Estimator | Cost |
//! |---|---|---|
//! | Gorilla | the real encoder counted on the float sample windows, scaled to the block | O(256 values) |
//! | RunDelta | the real encoder counted on the whole block (exact) | O(n), no allocation |
//!
//! Both run the codec's own bit-counting path (`ace_codecs::ts1_stream_bits`), so an estimate
//! can never be based on a different cost model than the encoder. Gorilla on a whole block is
//! almost as expensive as encoding it, which is why the planner samples it; RunDelta is only
//! evaluated for lanes the run prefilter admitted, where counting is a compare-and-skip loop.

use ace_analysis::{lane_sample_windows, FloatWidth};
use ace_codecs::{ts1_encoded_len, ts1_stream_bits, TimeSeriesLayout, TIME_SERIES_HEADER_SIZE};
use ace_core::AceResult;

/// Size estimate of one TS1 layout for one block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSeriesEstimate {
    /// Layout the estimate is for.
    pub layout: TimeSeriesLayout,
    /// Estimated payload size in bytes (TS1 header and tail included).
    pub estimated_bytes: u64,
    /// True when the size is exact (the encoder will produce exactly this many bytes).
    pub exact: bool,
    /// Confidence in `0.0..=1.0` (1.0 for exact estimates; window agreement for samples).
    pub confidence: f32,
}

/// Inherent methods of [`TimeSeriesEstimate`].
impl TimeSeriesEstimate {
    /// Estimated bytes per input byte (`0.0` for empty input).
    pub fn ratio_to(&self, input_len: usize) -> f64 {
        if input_len == 0 {
            0.0
        } else {
            self.estimated_bytes as f64 / input_len as f64
        }
    }
}

/// TS1 Gorilla layout of a float width.
pub const fn gorilla_layout(width: FloatWidth) -> TimeSeriesLayout {
    match width {
        FloatWidth::F32 => TimeSeriesLayout::GORILLA_F32,
        FloatWidth::F64 => TimeSeriesLayout::GORILLA_F64,
    }
}

/// Estimates the Gorilla payload of `input` read as `width` values from the sample windows.
///
/// Each window is encoded (counted) on its own; the mean bits per transition are scaled to the
/// block's `value_count − 1` transitions. Confidence is `1 − spread / mean` of the per-window
/// costs, clamped to `0..=1`, so uniform data is trusted and phase changes are not.
pub fn estimate_gorilla(input: &[u8], width: FloatWidth) -> AceResult<TimeSeriesEstimate> {
    let layout = gorilla_layout(width);
    let lane = width.lane_bytes();
    let value_count = input.len() / lane;
    let tail = input.len() % lane;
    let mut window_costs = Vec::with_capacity(4);
    for window in lane_sample_windows(value_count) {
        let transitions = window.len().saturating_sub(1);
        if transitions == 0 {
            continue;
        }
        let bytes = &input[window.start * lane..window.end * lane];
        let bits = ts1_stream_bits(bytes, layout)?;
        window_costs.push(bits as f64 / transitions as f64);
    }
    if window_costs.is_empty() {
        return Ok(TimeSeriesEstimate {
            layout,
            estimated_bytes: ts1_encoded_len(input, layout)? as u64,
            exact: true,
            confidence: 1.0,
        });
    }
    let mean = window_costs.iter().sum::<f64>() / window_costs.len() as f64;
    let (min, max) = window_costs
        .iter()
        .fold((f64::MAX, 0.0f64), |(lo, hi), &c| (lo.min(c), hi.max(c)));
    let stream_bits = mean * value_count.saturating_sub(1) as f64;
    let estimated_bytes = TIME_SERIES_HEADER_SIZE as u64 + (stream_bits / 8.0).ceil() as u64;
    let confidence = if mean <= 0.0 {
        1.0
    } else {
        (1.0 - (max - min) / mean).clamp(0.0, 1.0) as f32
    };
    Ok(TimeSeriesEstimate {
        layout,
        estimated_bytes: estimated_bytes + tail as u64,
        exact: false,
        confidence,
    })
}

/// Exact RunDelta payload size of `input` read as `lane_bytes` lanes.
pub fn estimate_run_delta(input: &[u8], lane_bytes: u8) -> AceResult<TimeSeriesEstimate> {
    let layout = TimeSeriesLayout::run_delta(lane_bytes);
    Ok(TimeSeriesEstimate {
        layout,
        estimated_bytes: ts1_encoded_len(input, layout)? as u64,
        exact: true,
        confidence: 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_codecs::ts1_encode_with;

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
}
