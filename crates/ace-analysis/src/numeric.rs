use ace_core::{NumericEndian, NumericWidth};

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
    pub exception_ratio: f32,
    /// Number of complete values in the block under the selected interpretation.
    pub value_count: usize,
    /// Number of trailing bytes that are not part of a complete numeric value.
    pub tail_bytes: usize,
}

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

/// Deterministically detects little-endian 32/64-bit numeric structure in arbitrary bytes.
pub fn analyze_numeric(input: &[u8]) -> NumericProfile {
    let u32_profile = analyze_u32(input);
    let u64_profile = analyze_u64(input);
    if u32_profile.confidence >= u64_profile.confidence {
        u32_profile
    } else {
        u64_profile
    }
}

/// Analyzes one little-endian u32 interpretation.
fn analyze_u32(input: &[u8]) -> NumericProfile {
    let values = input
        .chunks_exact(4)
        .take(4096)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte chunk")) as u64)
        .collect::<Vec<_>>();
    build_profile(input.len(), NumericWidth::U32, values)
}

/// Analyzes one little-endian u64 interpretation.
fn analyze_u64(input: &[u8]) -> NumericProfile {
    let values = input
        .chunks_exact(8)
        .take(4096)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("eight-byte chunk")))
        .collect::<Vec<_>>();
    build_profile(input.len(), NumericWidth::U64, values)
}

/// Builds deterministic delta and delta-of-delta statistics for one integer interpretation.
///
/// The 0.4-buildfix2 implementation uses fixed 65-bin bit-width histograms instead of allocating
/// delta, delta-of-delta and width vectors and then sorting them. The statistics are therefore
/// computed in one pass over the sampled value vector with bounded stack memory.
fn build_profile(input_len: usize, width: NumericWidth, values: Vec<u64>) -> NumericProfile {
    if values.len() < 8 {
        return NumericProfile::default();
    }

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

        let delta = value.wrapping_sub(previous_value) as i64;
        let delta_width = bits_required(zigzag(delta)) as usize;
        delta_hist[delta_width] += 1;
        delta_zero += usize::from(delta == 0);
        delta_small += usize::from(delta_width <= 8);
        delta_count += 1;

        if let Some(previous) = previous_delta {
            let dod = delta.wrapping_sub(previous);
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
    let monotonic_ratio = monotonic as f32 / delta_count.max(1) as f32;
    let delta_zero_ratio = delta_zero as f32 / delta_count.max(1) as f32;
    let dod_zero_ratio = dod_zero as f32 / dod_count.max(1) as f32;
    let delta_small_ratio = delta_small as f32 / delta_count.max(1) as f32;
    let dod_small_ratio = dod_small as f32 / dod_count.max(1) as f32;

    let width_score = match width {
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

    let selected_width = width.bytes();
    let value_count = input_len / selected_width;
    let tail_bytes = input_len % selected_width;
    let exceptions = delta_hist
        .iter()
        .enumerate()
        .filter(|(bits, _)| *bits > delta_p95 as usize)
        .map(|(_, count)| *count)
        .sum::<usize>();
    let exception_ratio = exceptions as f32 / delta_count.max(1) as f32;

    NumericProfile {
        detected: confidence >= 0.70,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Detects a monotonic u32 counter with high confidence.
    #[test]
    fn detects_u32_counter() {
        let mut bytes = Vec::new();
        for value in 10_000u32..20_000 {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let profile = analyze_numeric(&bytes);
        assert!(profile.detected);
        assert_eq!(profile.width, Some(NumericWidth::U32));
        assert!(profile.monotonic_ratio > 0.99);
        assert!(profile.dod_bit_width_p95 <= 1);
    }
}
