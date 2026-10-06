use ace_core::NumericWidth;

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
    /// Fraction of sampled first-order deltas fitting in signed 16-bit magnitude after ZigZag.
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
pub fn numeric_prefilter(input: &[u8]) -> NumericPrefilter {
    let u32_result = prefilter_width(input, NumericWidth::U32);
    let u64_result = prefilter_width(input, NumericWidth::U64);
    if u32_result.confidence >= u64_result.confidence {
        u32_result
    } else {
        u64_result
    }
}

/// Validates strong fixed-step numeric structure and returns reusable encoding evidence.
///
/// Buildfix4 deliberately makes the NumericFast invariant strict: every value must be
/// monotonically non-decreasing and every first-order delta must equal the first non-zero delta.
/// Variable-delta, outlier and sawtooth workloads therefore fall back to `NumericGeneral`.
pub fn strong_numeric_evidence(input: &[u8], width: NumericWidth) -> Option<NumericFastEvidence> {
    let width_bytes = width.bytes();
    if input.len() < width_bytes * 16 {
        return None;
    }

    let value_count = input.len() / width_bytes;
    let tail_bytes = input.len() % width_bytes;
    let mut iter = input[..value_count * width_bytes].chunks_exact(width_bytes);
    let first_chunk = iter.next()?;
    let second_chunk = iter.next()?;
    let first_value = decode(first_chunk, width);
    let second_value = decode(second_chunk, width);
    if second_value < first_value {
        return None;
    }

    let first_delta_u64 = second_value.wrapping_sub(first_value);
    if first_delta_u64 == 0 || first_delta_u64 > i64::MAX as u64 {
        return None;
    }
    let first_delta = first_delta_u64 as i64;
    let mut previous = second_value;

    for chunk in iter {
        let value = decode(chunk, width);
        if value < previous {
            return None;
        }
        let delta = value.wrapping_sub(previous);
        if delta != first_delta_u64 {
            return None;
        }
        previous = value;
    }

    Some(NumericFastEvidence {
        width,
        first_value,
        first_delta,
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
    let width_bytes = width.bytes();
    let mut iter = input.chunks_exact(width_bytes).take(1024);
    let Some(first_chunk) = iter.next() else {
        return NumericPrefilter::empty();
    };
    let Some(second_chunk) = iter.next() else {
        return NumericPrefilter::empty();
    };

    let mut previous = decode(first_chunk, width);
    let second = decode(second_chunk, width);
    let mut previous_delta = second.wrapping_sub(previous);
    previous = second;

    let mut delta_count = 1usize;
    let mut monotonic = usize::from(second >= decode(first_chunk, width));
    let mut nonzero = usize::from(previous_delta != 0);
    let mut small = usize::from(zigzag_bits(previous_delta as i64) <= 16);
    let mut constant = 0usize;
    let mut transitions = 0usize;

    for chunk in iter {
        let value = decode(chunk, width);
        let delta = value.wrapping_sub(previous);
        monotonic += usize::from(value >= previous);
        nonzero += usize::from(delta != 0);
        small += usize::from(zigzag_bits(delta as i64) <= 16);
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
        likely_numeric: confidence >= 0.72
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

/// Decodes one little-endian integer using the requested numeric width.
fn decode(chunk: &[u8], width: NumericWidth) -> u64 {
    match width {
        NumericWidth::U32 => {
            u32::from_le_bytes(chunk.try_into().expect("u32 prefilter chunk")) as u64
        }
        NumericWidth::U64 => u64::from_le_bytes(chunk.try_into().expect("u64 prefilter chunk")),
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

    /// Monotonic fixed-step counters must qualify for the strong numeric route.
    #[test]
    fn fixed_step_u32_is_strong_numeric() {
        let mut bytes = Vec::new();
        let mut value = 10_000u32;
        for _ in 0..4096 {
            value += 3;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let result = numeric_prefilter(&bytes);
        assert!(result.strong_numeric);
        assert_eq!(result.width_hint, Some(NumericWidth::U32));
        assert!(validate_strong_numeric(&bytes, NumericWidth::U32));
    }

    /// A single outlier breaks the strict fixed-step invariant.
    #[test]
    fn outlier_is_not_strong_fixed_step() {
        let mut bytes = Vec::new();
        let mut value = 10_000u32;
        for i in 0..4096 {
            value = value.wrapping_add(if i == 2048 { 7 } else { 3 });
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert!(strong_numeric_evidence(&bytes, NumericWidth::U32).is_none());
    }

    /// Sawtooth/reset data cannot enter NumericFast even if local deltas are small.
    #[test]
    fn sawtooth_is_not_strong_fixed_step() {
        let mut bytes = Vec::new();
        for i in 0..4096u32 {
            let value = i % 256;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
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
}
