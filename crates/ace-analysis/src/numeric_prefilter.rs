//! Cheap, allocation-free numeric classification used by Planner V4 route selection.
//!
//! The prefilter answers two questions about a block using at most 1,024 sampled values per
//! lane width and no heap allocation:
//!
//! 1. *Is this block worth routing to the numeric pipeline?* ([`NumericPrefilter::likely_numeric`])
//! 2. *Might it be a perfect fixed-step sequence?* ([`NumericPrefilter::strong_numeric`]), which
//!    is then confirmed over the **whole** block by [`strong_numeric_evidence`].
//!
//! The heavier statistical profile lives in [`crate::numeric`].

use ace_core::{lane_delta_const, read_lane, NumericWidth};

/// Maximum number of leading values sampled per lane width.
const PREFILTER_SAMPLE: usize = 1024;

/// Values validated per branch by [`strong_numeric_evidence`].
const GROUP_VALUES: usize = 8;

/// Minimum prefilter confidence at which a block is considered "likely numeric".
const LIKELY_CONFIDENCE: f32 = 0.72;

/// Cheap schema-free classification used before the full generic/numeric analyzers.
///
/// The prefilter intentionally inspects at most 1,024 integer values for each supported width and
/// performs no heap allocation. It is designed to answer only whether a block is worth routing to
/// the heavier ACE 0.4 numeric pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericPrefilter {
    /// True when sampled values exhibit enough numeric structure for full numeric analysis.
    pub likely_numeric: bool,
    /// True when a full-block validation may safely attempt the direct Numeric fast path.
    pub strong_numeric: bool,
    /// Width with the strongest sampled structure.
    pub width_hint: Option<NumericWidth>,
    /// Deterministic confidence in `0.0..=1.0`.
    pub confidence: f32,
    /// Fraction of sampled adjacent values that are monotonically non-decreasing.
    pub monotonic_ratio: f32,
    /// Fraction of sampled first-order deltas that are non-zero.
    pub nonzero_delta_ratio: f32,
    /// Fraction of sampled first-order deltas that need at most *half the lane width* in bits
    /// after ZigZag (u16: 8 bits, u32: 16 bits, u64: 32 bits).
    ///
    /// Before ACE 0.4.5 the limit was a fixed 16 bits for every lane, which rejected u64
    /// timestamp series whose deltas need 17–32 bits even though bit-packing still saves more
    /// than 50%.
    pub small_delta_ratio: f32,
    /// Fraction of sampled delta transitions with an identical first-order delta.
    pub constant_delta_ratio: f32,
}

/// Evidence produced by one successful full-block fixed-step Numeric validation.
///
/// The structure carries exactly the information needed to construct a zero-bit-width
/// Delta-of-Delta `NUM1` payload without re-scanning the block inside the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericFastEvidence {
    /// Integer width validated over the complete block.
    pub width: NumericWidth,
    /// First logical numeric value.
    pub first_value: u64,
    /// Constant first-order delta shared by the validated sequence.
    pub first_delta: i64,
    /// Number of complete logical numeric values.
    pub value_count: usize,
    /// Number of trailing bytes after the complete logical values.
    pub tail_bytes: usize,
}

/// Examines a bounded prefix of `input` without allocating intermediate vectors.
///
/// All three lane widths are scored; the one with the highest confidence wins. Ties keep the
/// historical preference u32 → u64 → u16, so the new 16-bit lane only wins when strictly better.
pub fn numeric_prefilter(input: &[u8]) -> NumericPrefilter {
    let mut best = prefilter_width(input, NumericWidth::U32);
    for width in [NumericWidth::U64, NumericWidth::U16] {
        let candidate = prefilter_width(input, width);
        if candidate.confidence > best.confidence {
            best = candidate;
        }
    }
    best
}

/// Validates strong fixed-step numeric structure and returns reusable encoding evidence.
///
/// Buildfix4 deliberately makes the NumericFast invariant strict: every value must be
/// monotonically non-decreasing and every first-order delta must equal the first non-zero delta.
/// Variable-delta, outlier and sawtooth workloads therefore fall back to `NumericGeneral`.
pub fn strong_numeric_evidence(input: &[u8], width: NumericWidth) -> Option<NumericFastEvidence> {
    // Dispatch once per block to a loop that is monomorphized for the lane width.
    match width {
        NumericWidth::U16 => strong_evidence_lane::<2>(input, width),
        NumericWidth::U32 => strong_evidence_lane::<4>(input, width),
        NumericWidth::U64 => strong_evidence_lane::<8>(input, width),
    }
}

/// Width-monomorphized body of [`strong_numeric_evidence`] (`B` is the lane width in bytes).
fn strong_evidence_lane<const B: usize>(
    input: &[u8],
    width: NumericWidth,
) -> Option<NumericFastEvidence> {
    // At least 16 values are required so that "constant delta" is statistically meaningful.
    if input.len() < B * 16 {
        return None;
    }

    let value_count = input.len() / B;
    let tail_bytes = input.len() % B;
    let first_value = read_lane::<B>(&input[..B]);
    let second_value = read_lane::<B>(&input[B..2 * B]);
    if second_value < first_value {
        return None;
    }

    // The step must be strictly positive and representable as a positive i64.
    let step = second_value.wrapping_sub(first_value);
    if step == 0 || step > i64::MAX as u64 {
        return None;
    }
    // Validate groups of eight values with one branch per group: the unrolled, branch-free
    // group body is faster and, unlike a per-value early exit, its speed does not depend on
    // where the linker happens to place the loop (ACE 0.5.0 A/B: ±13 % between builds).
    let body = &input[2 * B..value_count * B];
    let mut previous = second_value;
    let mut groups = body.chunks_exact(GROUP_VALUES * B);
    for group in groups.by_ref() {
        let mut broken = false;
        for chunk in group.chunks_exact(B) {
            let value = read_lane::<B>(chunk);
            broken |= (value < previous) | (value.wrapping_sub(previous) != step);
            previous = value;
        }
        if broken {
            return None;
        }
    }
    for chunk in groups.remainder().chunks_exact(B) {
        let value = read_lane::<B>(chunk);
        if value < previous || value.wrapping_sub(previous) != step {
            return None;
        }
        previous = value;
    }

    Some(NumericFastEvidence {
        width,
        first_value,
        first_delta: step as i64,
        value_count,
        tail_bytes,
    })
}

/// Returns whether a block satisfies the strict NumericFast invariant.
///
/// This compatibility wrapper is intentionally implemented through `strong_numeric_evidence` so
/// tests and older call sites cannot drift from the evidence-producing validator.
pub fn validate_strong_numeric(input: &[u8], width: NumericWidth) -> bool {
    strong_numeric_evidence(input, width).is_some()
}

/// Computes one width-specific bounded prefilter result.
fn prefilter_width(input: &[u8], width: NumericWidth) -> NumericPrefilter {
    match width {
        NumericWidth::U16 => prefilter_lane::<2>(input, width),
        NumericWidth::U32 => prefilter_lane::<4>(input, width),
        NumericWidth::U64 => prefilter_lane::<8>(input, width),
    }
}

/// Width-monomorphized body of [`prefilter_width`] (`B` is the lane width in bytes).
fn prefilter_lane<const B: usize>(input: &[u8], width: NumericWidth) -> NumericPrefilter {
    let small_limit = width.small_delta_bits();
    let mut values = input.chunks_exact(B).take(PREFILTER_SAMPLE);
    let Some(first_chunk) = values.next() else {
        return NumericPrefilter::empty();
    };
    let Some(second_chunk) = values.next() else {
        return NumericPrefilter::empty();
    };

    let first = read_lane::<B>(first_chunk);
    let second = read_lane::<B>(second_chunk);
    let mut previous = second;
    let mut previous_delta = lane_delta_const::<B>(first, second);

    // Counters for the first delta (already seen) ...
    let mut delta_count = 1usize;
    let mut monotonic = usize::from(second >= first);
    let mut nonzero = usize::from(previous_delta != 0);
    let mut small = usize::from(zigzag_bits(previous_delta) <= small_limit);
    let mut constant = 0usize;
    let mut transitions = 0usize;

    // ... and for every following value.
    for chunk in values {
        let value = read_lane::<B>(chunk);
        let delta = lane_delta_const::<B>(previous, value);
        monotonic += usize::from(value >= previous);
        nonzero += usize::from(delta != 0);
        small += usize::from(zigzag_bits(delta) <= small_limit);
        constant += usize::from(delta == previous_delta);
        transitions += 1;
        delta_count += 1;
        previous = value;
        previous_delta = delta;
    }

    let monotonic_ratio = monotonic as f32 / delta_count.max(1) as f32;
    let nonzero_delta_ratio = nonzero as f32 / delta_count.max(1) as f32;
    let small_delta_ratio = small as f32 / delta_count.max(1) as f32;
    let constant_delta_ratio = constant as f32 / transitions.max(1) as f32;

    // Constant zeros/runs are intentionally excluded from the strong path; RLE/RAW may decode
    // those cases more cheaply even when Numeric would save a few additional bytes.
    let confidence = (0.10
        + monotonic_ratio * 0.30
        + small_delta_ratio * 0.25
        + constant_delta_ratio * 0.30
        + nonzero_delta_ratio * 0.20)
        .clamp(0.0, 1.0);

    NumericPrefilter {
        likely_numeric: confidence >= LIKELY_CONFIDENCE
            && monotonic_ratio >= 0.80
            && nonzero_delta_ratio >= 0.10
            && small_delta_ratio >= 0.80,
        strong_numeric: confidence >= 0.995
            && monotonic_ratio >= 0.999
            && nonzero_delta_ratio >= 0.999
            && small_delta_ratio >= 0.999
            && constant_delta_ratio >= 0.999,
        width_hint: Some(width),
        confidence,
        monotonic_ratio,
        nonzero_delta_ratio,
        small_delta_ratio,
        constant_delta_ratio,
    }
}

/// Inherent methods of [`NumericPrefilter`].
impl NumericPrefilter {
    /// Returns an explicit non-numeric prefilter result.
    fn empty() -> Self {
        Self {
            likely_numeric: false,
            strong_numeric: false,
            width_hint: None,
            confidence: 0.0,
            monotonic_ratio: 0.0,
            nonzero_delta_ratio: 0.0,
            small_delta_ratio: 0.0,
            constant_delta_ratio: 0.0,
        }
    }
}

/// Returns the bit width of a ZigZag-mapped signed value.
fn zigzag_bits(value: i64) -> u8 {
    let encoded = ((value << 1) ^ (value >> 63)) as u64;
    if encoded == 0 {
        0
    } else {
        (64 - encoded.leading_zeros()) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
