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
///
/// # Determinism
///
/// A pure function of `input` (integer arithmetic only); ties between widths follow the
/// fixed order u32 → u64 → u16.
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
// `chunks_exact` is kept on purpose: this hot kernel stays codegen-identical in the 0.5.1
// hardening release (hot loops showed layout-sensitive timings in the 0.5.0 A/B); `as_chunks`
// lands in 0.6 with an interleaved A/B run. `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
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
// `chunks_exact` is kept on purpose: this hot kernel stays codegen-identical in the 0.5.1
// hardening release (hot loops showed layout-sensitive timings in the 0.5.0 A/B); `as_chunks`
// lands in 0.6 with an interleaved A/B run. `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
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
