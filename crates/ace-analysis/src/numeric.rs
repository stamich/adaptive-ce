//! Full numeric-structure analysis of a block (ACE 0.4).
//!
//! [`analyze_numeric`] scores three little-endian interpretations of the same bytes (u16, u32,
//! u64) over a bounded sample and returns the best one as a [`NumericProfile`]. The profile is
//! *evidence*, not a schema: the planner still verifies the winning interpretation with the
//! exact numeric estimator before choosing the numeric codec.
//!
//! The cheaper allocation-free classifier used on the hot path lives in
//! [`crate::numeric_prefilter()`].

use ace_core::{read_lane, NumericEndian, NumericWidth};

/// Maximum number of leading values examined per width.
///
/// Bounding the sample keeps analysis cost independent of block size while still being large
/// enough for stable percentiles.
const SAMPLE_VALUES: usize = 4096;

/// Minimum number of sampled values below which no numeric claim is made.
const MIN_SAMPLE_VALUES: usize = 8;

/// Confidence at or above which a profile is flagged as `detected`.
const DETECTION_CONFIDENCE: f32 = 0.70;

/// Statistical description of byte data interpreted as a fixed-width integer sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericProfile {
    /// True when the detector has sufficient confidence to admit numeric candidates.
    pub detected: bool,
    /// Best integer width selected by deterministic scoring.
    pub width: Option<NumericWidth>,
    /// Byte order associated with the selected interpretation.
    pub endian: NumericEndian,
    /// Confidence in the selected numeric interpretation in `0.0..=1.0`.
    pub confidence: f32,
    /// Fraction of adjacent values that are monotonically non-decreasing.
    pub monotonic_ratio: f32,
    /// Fraction of first-order deltas equal to zero.
    pub delta_zero_ratio: f32,
    /// Fraction of first-order deltas whose ZigZag representation fits within eight bits.
    pub delta_small_ratio: f32,
    /// p95 bit width of ZigZag first-order deltas.
    pub delta_bit_width_p95: u8,
    /// Fraction of delta-of-delta values equal to zero.
    pub dod_zero_ratio: f32,
    /// Fraction of delta-of-delta values whose ZigZag representation fits within eight bits.
    pub dod_small_ratio: f32,
    /// p95 bit width of ZigZag delta-of-delta values.
    pub dod_bit_width_p95: u8,
    /// Fraction of values outside the p95 bit-width envelope.
    ///
    /// This is the *exception ratio* telemetry that justifies (or not) a future Patched-FOR
    /// codec: a high ratio with a small p95 means a few outliers inflate the packed width.
    pub exception_ratio: f32,
    /// Number of complete values in the block under the selected interpretation.
    pub value_count: usize,
    /// Number of trailing bytes that are not part of a complete numeric value.
    pub tail_bytes: usize,
}

/// Provides the documented default values of [`NumericProfile`].
impl Default for NumericProfile {
    /// Returns an explicitly non-numeric profile.
    fn default() -> Self {
        Self {
            detected: false,
            width: None,
            endian: NumericEndian::Little,
            confidence: 0.0,
            monotonic_ratio: 0.0,
            delta_zero_ratio: 0.0,
            delta_small_ratio: 0.0,
            delta_bit_width_p95: 64,
            dod_zero_ratio: 0.0,
            dod_small_ratio: 0.0,
            dod_bit_width_p95: 64,
            exception_ratio: 1.0,
            value_count: 0,
            tail_bytes: 0,
        }
    }
}

/// Deterministically detects little-endian 16/32/64-bit numeric structure in arbitrary bytes.
///
/// Ties favor the *wider* interpretation that was historically preferred (u32, then u64, then
/// u16), so adding the 16-bit lane in 0.4.5 never changes the verdict for existing 32/64-bit
/// data: u16 wins only with strictly higher confidence.
pub fn analyze_numeric(input: &[u8]) -> NumericProfile {
    let mut best = analyze_width(input, NumericWidth::U32);
    for width in [NumericWidth::U64, NumericWidth::U16] {
        let candidate = analyze_width(input, width);
        if candidate.confidence > best.confidence {
            best = candidate;
        }
    }
    best
}

/// Reads the first [`SAMPLE_VALUES`] complete values of one width as zero-extended `u64`.
fn sample_values(input: &[u8], width: NumericWidth) -> Vec<u64> {
    match width {
        NumericWidth::U16 => sample_lane::<2>(input),
        NumericWidth::U32 => sample_lane::<4>(input),
        NumericWidth::U64 => sample_lane::<8>(input),
    }
}

/// Width-monomorphized body of [`sample_values`] (`B` is the lane width in bytes).
// `chunks_exact` is kept on purpose: this hot kernel stays codegen-identical in the 0.5.1
// hardening release (hot loops showed layout-sensitive timings in the 0.5.0 A/B); `as_chunks`
// lands in 0.6 with an interleaved A/B run. `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn sample_lane<const B: usize>(input: &[u8]) -> Vec<u64> {
    input
        .chunks_exact(B)
        .take(SAMPLE_VALUES)
        .map(read_lane::<B>)
        .collect()
}

/// Analyzes one little-endian interpretation of `input`.
fn analyze_width(input: &[u8], width: NumericWidth) -> NumericProfile {
    build_profile(input.len(), width, sample_values(input, width))
}

/// Builds deterministic delta and delta-of-delta statistics for one integer interpretation.
///
/// The 0.4-buildfix2 implementation uses fixed 65-bin bit-width histograms instead of allocating
/// delta, delta-of-delta and width vectors and then sorting them. The statistics are therefore
/// computed in one pass over the sampled value vector with bounded stack memory.
/// Since 0.4.5 deltas are evaluated in the lane's modular ring (see
/// [`NumericWidth::lane_delta`]).
fn build_profile(input_len: usize, width: NumericWidth, values: Vec<u64>) -> NumericProfile {
    if values.len() < MIN_SAMPLE_VALUES {
        return NumericProfile::default();
    }

    // Histograms indexed by the ZigZag bit width (0..=64) of each delta / delta-of-delta.
    let mut delta_hist = [0usize; 65];
    let mut dod_hist = [0usize; 65];
    let mut monotonic = 0usize;
    let mut delta_zero = 0usize;
    let mut delta_small = 0usize;
    let mut dod_zero = 0usize;
    let mut dod_small = 0usize;

    let mut previous_value = values[0];
    let mut previous_delta: Option<i64> = None;
    let mut delta_count = 0usize;
    let mut dod_count = 0usize;

    for &value in values.iter().skip(1) {
        if value >= previous_value {
            monotonic += 1;
        }

        // First-order delta in the lane ring.
        let delta = width.lane_delta(previous_value, value);
        let delta_width = bits_required(zigzag(delta)) as usize;
        delta_hist[delta_width] += 1;
        delta_zero += usize::from(delta == 0);
        delta_small += usize::from(delta_width <= 8);
        delta_count += 1;

        // Second-order delta (needs a previous delta).
        if let Some(previous) = previous_delta {
            let dod = width.sign_extend(delta.wrapping_sub(previous) as u64);
            let dod_width = bits_required(zigzag(dod)) as usize;
            dod_hist[dod_width] += 1;
            dod_zero += usize::from(dod == 0);
            dod_small += usize::from(dod_width <= 8);
            dod_count += 1;
        }

        previous_value = value;
        previous_delta = Some(delta);
    }

    let delta_p95 = percentile_histogram(&delta_hist, delta_count, 95);
    let dod_p95 = percentile_histogram(&dod_hist, dod_count, 95);
    let ratio = |numerator: usize, denominator: usize| numerator as f32 / denominator.max(1) as f32;
    let monotonic_ratio = ratio(monotonic, delta_count);
    let delta_zero_ratio = ratio(delta_zero, delta_count);
    let dod_zero_ratio = ratio(dod_zero, dod_count);
    let delta_small_ratio = ratio(delta_small, delta_count);
    let dod_small_ratio = ratio(dod_small, dod_count);

    // Bonus awarded when p95 deltas are small compared with the lane width. Thresholds scale
    // with the lane: u16 ≤ 8 / ≤ 12 bits, u32 ≤ 16 / ≤ 24, u64 ≤ 20 / ≤ 32.
    let width_score = match width {
        NumericWidth::U16 => {
            if delta_p95 <= 8 {
                0.25
            } else if delta_p95 <= 12 {
                0.12
            } else {
                0.0
            }
        }
        NumericWidth::U32 => {
            if delta_p95 <= 16 {
                0.25
            } else if delta_p95 <= 24 {
                0.12
            } else {
                0.0
            }
        }
        NumericWidth::U64 => {
            if delta_p95 <= 20 {
                0.25
            } else if delta_p95 <= 32 {
                0.10
            } else {
                0.0
            }
        }
    };
    let confidence = (0.15
        + monotonic_ratio * 0.30
        + delta_small_ratio * 0.25
        + dod_small_ratio * 0.20
        + width_score)
        .clamp(0.0, 1.0);

    let lane_bytes = width.bytes();
    let value_count = input_len / lane_bytes;
    let tail_bytes = input_len % lane_bytes;

    // Exceptions: deltas needing more bits than the p95 envelope.
    let exceptions: usize = delta_hist
        .iter()
        .enumerate()
        .filter(|(bits, _)| *bits > delta_p95 as usize)
        .map(|(_, count)| *count)
        .sum();
    let exception_ratio = ratio(exceptions, delta_count);

    NumericProfile {
        detected: confidence >= DETECTION_CONFIDENCE,
        width: Some(width),
        endian: NumericEndian::Little,
        confidence,
        monotonic_ratio,
        delta_zero_ratio,
        delta_small_ratio,
        delta_bit_width_p95: delta_p95,
        dod_zero_ratio,
        dod_small_ratio,
        dod_bit_width_p95: dod_p95,
        exception_ratio,
        value_count,
        tail_bytes,
    }
}

/// Returns a deterministic nearest-rank percentile from a fixed bit-width histogram.
fn percentile_histogram(histogram: &[usize; 65], count: usize, percentile: usize) -> u8 {
    if count == 0 {
        return 64;
    }
    // Nearest-rank: the smallest width whose cumulative frequency reaches the target rank.
    let target = ((count - 1) * percentile / 100) + 1;
    let mut cumulative = 0usize;
    for (width, frequency) in histogram.iter().enumerate() {
        cumulative = cumulative.saturating_add(*frequency);
        if cumulative >= target {
            return width as u8;
        }
    }
    64
}

/// ZigZag maps a signed delta to an unsigned magnitude-friendly representation.
fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// Returns the minimum bit width of one unsigned integer.
fn bits_required(value: u64) -> u8 {
    if value == 0 {
        0
    } else {
        (64 - value.leading_zeros()) as u8
    }
}
