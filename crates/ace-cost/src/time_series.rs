//! TS1 size estimators (ACE 0.5.0).
//!
//! | Candidate | Estimator | Cost |
//! |---|---|---|
//! | Gorilla | the real encoder counted on the float sample windows, scaled to the block | O(256 values) |
//! | RunDelta | the real encoder counted on the whole block (exact) | O(n), no allocation |
//! | ALP (0.6.1) | the real encoder counted on the whole block (exact) | O(n), one exception list |
//!
//! Both run the codec's own bit-counting path (`ace_codecs::ts1_stream_bits`), so an estimate
//! can never be based on a different cost model than the encoder. Gorilla on a whole block is
//! almost as expensive as encoding it, which is why the planner samples it; RunDelta is only
//! evaluated for lanes the run prefilter admitted, where counting is a compare-and-skip loop.

use ace_analysis::{lane_sample_windows, FloatWidth};
use ace_codecs::{
    alp_sample_viable, ts1_encoded_len, ts1_encoded_len_within, ts1_stream_bits, TimeSeriesLayout,
    TIME_SERIES_HEADER_SIZE,
};
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
///
/// # Errors
///
/// Propagates [`ace_core::AceError::InvalidTimeSeries`] from the TS1 bit counter; it cannot occur for
/// the Gorilla layouts of a [`FloatWidth`].
///
/// # Determinism
///
/// Fixed sample windows and exact bit counting; the floating-point scaling is evaluated
/// in a fixed order, so the estimate is identical on every platform.
///
/// # Invariants
///
/// `exact == false` unless the block is too short to sample, in which case the estimate is
/// the exact encoded length (`exact == true`, `confidence == 1.0`).
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
///
/// # Errors
///
/// [`ace_core::AceError::InvalidTimeSeries`] when `lane_bytes` is not a RunDelta lane width (2, 4, 8).
///
/// # Determinism
///
/// Runs the TS1 encoder against a bit counter, so it is exact and platform independent.
///
/// # Invariants
///
/// `estimated_bytes == ts1_encode_with(input, layout).len()` and `exact == true`.
pub fn estimate_run_delta(input: &[u8], lane_bytes: u8) -> AceResult<TimeSeriesEstimate> {
    let layout = TimeSeriesLayout::run_delta(lane_bytes);
    Ok(TimeSeriesEstimate {
        layout,
        estimated_bytes: ts1_encoded_len(input, layout)? as u64,
        exact: true,
        confidence: 1.0,
    })
}

/// [`estimate_run_delta`] that stops counting once the payload is larger than `max_bytes`.
///
/// `Some(estimate)` is exactly what [`estimate_run_delta`] returns and has
/// `estimated_bytes <= max_bytes`; `None` means the exact size is larger than `max_bytes`.
///
/// # Errors
///
/// Same as [`estimate_run_delta`].
///
/// # Determinism
///
/// A pure function of its arguments.
///
/// # Invariants
///
/// `Some` only when `estimate_run_delta(input, lane_bytes)` would be `<= max_bytes`.
pub fn estimate_run_delta_within(
    input: &[u8],
    lane_bytes: u8,
    max_bytes: u64,
) -> AceResult<Option<TimeSeriesEstimate>> {
    let layout = TimeSeriesLayout::run_delta(lane_bytes);
    let limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    Ok(
        ts1_encoded_len_within(input, layout, limit)?.map(|len| TimeSeriesEstimate {
            layout,
            estimated_bytes: len as u64,
            exact: true,
            confidence: 1.0,
        }),
    )
}

/// Exact ALP payload size of `input` read as `width` values (ACE 0.6.1, TS1 mode 4).
///
/// # Errors
///
/// Propagates [`ace_core::AceError::InvalidTimeSeries`] from the TS1 bit counter; it cannot
/// occur for the ALP layouts of a [`FloatWidth`].
///
/// # Determinism
///
/// Runs the TS1 encoder against a bit counter (the `(e, f)` choice is a pure function of the
/// block), so the estimate is exact and platform independent.
///
/// # Invariants
///
/// `estimated_bytes == ts1_encode_with(input, layout).len()` and `exact == true`.
pub fn estimate_alp(input: &[u8], width: FloatWidth) -> AceResult<TimeSeriesEstimate> {
    let layout = alp_layout(width);
    Ok(TimeSeriesEstimate {
        layout,
        estimated_bytes: ts1_encoded_len(input, layout)? as u64,
        exact: true,
        confidence: 1.0,
    })
}

/// Cheap pre-test for [`estimate_alp`]: at least half of a 64-value sample is decimal-shaped.
pub fn alp_plausible(input: &[u8], width: FloatWidth) -> bool {
    alp_sample_viable(input, width.lane_bytes() as u8)
}

/// TS1 ALP layout of a float width.
pub const fn alp_layout(width: FloatWidth) -> TimeSeriesLayout {
    match width {
        FloatWidth::F32 => TimeSeriesLayout::ALP_F32,
        FloatWidth::F64 => TimeSeriesLayout::ALP_F64,
    }
}
