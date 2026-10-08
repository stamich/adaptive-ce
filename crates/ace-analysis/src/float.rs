//! Float lane analysis (ACE 0.5.0): does a block hold IEEE-754 values, and of which width?
//!
//! The prefilter is cheap (a few hundred values, no per-value allocation): it inspects [`LANE_SAMPLE_WINDOWS`] windows of
//! [`LANE_WINDOW_VALUES`] consecutive values (start, 1/3, 2/3, end of the block) for each width
//! and computes a [`FloatProfile`]. [`admit_float`] then applies the admission rules, which are
//! designed to reject the classic false positives of a float detector:
//!
//! | False positive | Signature | Rule |
//! |---|---|---|
//! | integers read as floats | subnormals, or implausible exponents (1e-190) | `subnormal_ratio`, exponent window |
//! | zero padding / sparse data | most values are ±0 | `zero_ratio` |
//! | random or compressed bytes | exponents all over the place, XOR ≈ full width | `exponent_distinct`, meaningful bits |
//! | NaN / Inf heavy buffers | many non-finite values | `non_finite_ratio` |
//!
//! Thresholds were calibrated on Corpus V4 and the false-positive corpus
//! (`docs/FLOAT-CALIBRATION-0.5.0.md`).

use std::ops::Range;

use ace_core::read_lane;

/// Number of deterministic sample windows taken from a block.
pub const LANE_SAMPLE_WINDOWS: usize = 4;
/// Number of consecutive values in one sample window.
pub const LANE_WINDOW_VALUES: usize = 64;

/// Maximum fraction of NaN / ±Inf values in an admitted block.
const MAX_NON_FINITE_RATIO: f32 = 0.05;
/// Maximum fraction of subnormal values (integers read as floats are mostly subnormal).
const MAX_SUBNORMAL_RATIO: f32 = 0.05;
/// Maximum fraction of ±0 values (zero-dominated blocks belong to RLE / LZ).
const MAX_ZERO_RATIO: f32 = 0.25;
/// Maximum fraction of normal values outside the plausible exponent window.
const MAX_IMPLAUSIBLE_RATIO: f32 = 0.05;
/// Maximum number of distinct exponents among the sampled values.
const MAX_EXPONENT_DISTINCT: u16 = 16;
/// Maximum mean number of meaningful XOR bits, as a fraction of the value width.
const MAX_MEANINGFUL_FRACTION: f32 = 0.75;
/// Admitted magnitudes: unbiased exponent within `±MAX_ABS_EXPONENT_F64` (≈ 1e-60 … 1e60).
const MAX_ABS_EXPONENT_F64: i32 = 200;
/// Admitted magnitudes for f32: unbiased exponent within ±60 (≈ 1e-18 … 1e18).
const MAX_ABS_EXPONENT_F32: i32 = 60;

/// IEEE-754 value width of a float lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FloatWidth {
    /// binary32 (`f32`).
    F32,
    /// binary64 (`f64`).
    F64,
}

/// Inherent methods of [`FloatWidth`].
impl FloatWidth {
    /// Both widths, f64 first (f64 wins ties).
    pub const ALL: [Self; 2] = [Self::F64, Self::F32];

    /// Bytes per value.
    pub const fn lane_bytes(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    /// Bits per value.
    pub const fn bits(self) -> u32 {
        (self.lane_bytes() * 8) as u32
    }

    /// Stable lowercase label (`"f32"` / `"f64"`).
    pub const fn label(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// Number of mantissa (fraction) bits.
    const fn mantissa_bits(self) -> u32 {
        match self {
            Self::F32 => 23,
            Self::F64 => 52,
        }
    }

    /// Exponent bias.
    const fn bias(self) -> i32 {
        match self {
            Self::F32 => 127,
            Self::F64 => 1023,
        }
    }

    /// All-ones exponent field (NaN / Inf).
    const fn exponent_max(self) -> u32 {
        match self {
            Self::F32 => 0xff,
            Self::F64 => 0x7ff,
        }
    }

    /// Largest admitted absolute unbiased exponent.
    const fn max_abs_exponent(self) -> i32 {
        match self {
            Self::F32 => MAX_ABS_EXPONENT_F32,
            Self::F64 => MAX_ABS_EXPONENT_F64,
        }
    }

    /// Gorilla control bits of a new XOR window (`'1' '1'` + leading-zero and length fields).
    const fn window_control_bits(self) -> f32 {
        match self {
            Self::F32 => 2.0 + 4.0 + 5.0,
            Self::F64 => 2.0 + 5.0 + 6.0,
        }
    }
}

/// Sampled statistics of a block read as a sequence of IEEE-754 values of one width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatProfile {
    /// Width the block was read with.
    pub width: FloatWidth,
    /// Number of complete values in the block.
    pub value_count: usize,
    /// Number of sampled values.
    pub sampled_values: u32,
    /// Fraction of sampled values that are NaN or ±Inf.
    pub non_finite_ratio: f32,
    /// Fraction of sampled values that are subnormal (zero excluded).
    pub subnormal_ratio: f32,
    /// Fraction of sampled values equal to ±0.
    pub zero_ratio: f32,
    /// Fraction of sampled finite non-zero values whose magnitude lies outside the admitted
    /// exponent window (≈ 1e-60 … 1e60 for f64).
    pub implausible_magnitude_ratio: f32,
    /// Fraction of sampled transitions whose bit patterns are identical (XOR = 0).
    pub xor_zero_ratio: f32,
    /// Mean `width − leading zeros − trailing zeros` over the non-zero sampled XORs.
    pub mean_xor_meaningful_bits: f32,
    /// Number of distinct exponent fields among the sampled values.
    pub exponent_distinct: u16,
    /// Fraction of sampled transitions that flip the sign bit.
    pub sign_change_ratio: f32,
}

/// Inherent methods of [`FloatProfile`].
impl FloatProfile {
    /// Rough Gorilla cost in bits per value (no window reuse), used only to pick the width.
    ///
    /// The planner's size estimate comes from `ace_cost::estimate_gorilla`, which runs the
    /// real codec on the sample windows; this proxy just has to rank f64 against f32.
    pub fn proxy_bits_per_value(&self) -> f32 {
        1.0 + (1.0 - self.xor_zero_ratio)
            * (self.width.window_control_bits() - 1.0 + self.mean_xor_meaningful_bits)
    }

    /// [`Self::proxy_bits_per_value`] normalized per input byte.
    pub fn proxy_bits_per_byte(&self) -> f32 {
        self.proxy_bits_per_value() / self.width.lane_bytes() as f32
    }
}

/// Why [`admit_float`] rejected a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatRejection {
    /// Fewer than [`LANE_WINDOW_VALUES`] values.
    TooShort,
    /// Too many NaN / ±Inf values.
    NonFinite,
    /// Too many subnormals (typical of integers read as floats).
    Subnormal,
    /// Too many magnitudes outside the plausible exponent window.
    ImplausibleMagnitude,
    /// Mostly ±0.
    ZeroDominated,
    /// Too many distinct exponents (random or compressed bytes).
    ExponentSpread,
    /// XORs of neighbours are almost full width (noise).
    NoisyMantissa,
}

/// Inherent methods of [`FloatRejection`].
impl FloatRejection {
    /// Stable lowercase label for explain output and benchmarks.
    pub const fn label(self) -> &'static str {
        match self {
            Self::TooShort => "too-short",
            Self::NonFinite => "non-finite",
            Self::Subnormal => "subnormal",
            Self::ImplausibleMagnitude => "implausible-magnitude",
            Self::ZeroDominated => "zero-dominated",
            Self::ExponentSpread => "exponent-spread",
            Self::NoisyMantissa => "noisy-mantissa",
        }
    }
}

/// Result of [`float_prefilter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatPrefilter {
    /// Admitted profile of the selected width, or `None` when the block is not float data.
    pub admitted: Option<FloatProfile>,
    /// Rejection of the f64 reading when nothing was admitted (explain / benchmarks).
    pub rejection: Option<FloatRejection>,
}

/// Value-index ranges of the deterministic sample windows of a block with `value_count` values.
///
/// Windows start at 0, `n/3`, `2n/3` and `n − 64`; overlapping windows are merged so short
/// blocks are not sampled twice. Shared by the float and run prefilters and by the TS1
/// estimators, so every stage sees the same values.
pub fn lane_sample_windows(value_count: usize) -> Vec<Range<usize>> {
    let mut windows: Vec<Range<usize>> = Vec::with_capacity(LANE_SAMPLE_WINDOWS);
    if value_count == 0 {
        return windows;
    }
    let len = LANE_WINDOW_VALUES.min(value_count);
    let last_start = value_count - len;
    let starts = [0, value_count / 3, 2 * value_count / 3, last_start];
    for start in starts {
        let start = start.min(last_start);
        let range = start..start + len;
        match windows.last_mut() {
            Some(previous) if range.start <= previous.end => {
                previous.end = previous.end.max(range.end);
            }
            _ => windows.push(range),
        }
    }
    windows
}

/// Computes the [`FloatProfile`] of `input` read as `width` values (`None` if too short).
pub fn float_profile(input: &[u8], width: FloatWidth) -> Option<FloatProfile> {
    match width {
        FloatWidth::F32 => profile_lane::<4>(input, width),
        FloatWidth::F64 => profile_lane::<8>(input, width),
    }
}

/// Width-monomorphized body of [`float_profile`] (`B` = bytes per value).
fn profile_lane<const B: usize>(input: &[u8], width: FloatWidth) -> Option<FloatProfile> {
    let value_count = input.len() / B;
    if value_count < LANE_WINDOW_VALUES {
        return None;
    }
    let bits = width.bits();
    let mantissa_bits = width.mantissa_bits();
    let exponent_max = width.exponent_max();
    let value_at = |index: usize| read_lane::<B>(&input[index * B..index * B + B]);

    // 2048-bit set of seen exponent fields (enough for f64's 11-bit exponent).
    let mut seen_exponents = [0u64; 32];
    let mut counts = SampleCounts::default();
    for window in lane_sample_windows(value_count) {
        let mut previous: Option<u64> = None;
        for index in window {
            let value = value_at(index);
            let exponent = ((value >> mantissa_bits) as u32) & exponent_max;
            let mantissa = value & ((1u64 << mantissa_bits) - 1);
            seen_exponents[(exponent / 64) as usize] |= 1u64 << (exponent % 64);
            counts.values += 1;
            if exponent == exponent_max {
                counts.non_finite += 1;
            } else if exponent == 0 && mantissa == 0 {
                counts.zeros += 1;
            } else if exponent == 0 {
                counts.subnormal += 1;
            } else if (exponent as i32 - width.bias()).abs() > width.max_abs_exponent() {
                counts.implausible += 1;
            }
            if let Some(previous) = previous {
                counts.transitions += 1;
                let xor = previous ^ value;
                if xor == 0 {
                    counts.xor_zero += 1;
                } else {
                    // `xor` lives in the low `bits` bits, so leading zeros are counted within
                    // the lane, not within the u64.
                    let leading = xor.leading_zeros() - (64 - bits);
                    counts.meaningful_bits += u64::from(bits - leading - xor.trailing_zeros());
                }
                if (previous ^ value) >> (bits - 1) & 1 == 1 {
                    counts.sign_changes += 1;
                }
            }
            previous = Some(value);
        }
    }
    let exponent_distinct = seen_exponents.iter().map(|w| w.count_ones()).sum::<u32>() as u16;
    Some(counts.profile(width, value_count, exponent_distinct))
}

/// Raw counters collected over the sample windows.
#[derive(Debug, Default)]
struct SampleCounts {
    /// Sampled values.
    values: u32,
    /// Sampled transitions (pairs of neighbours inside one window).
    transitions: u32,
    /// NaN / ±Inf values.
    non_finite: u32,
    /// Subnormal values (zero excluded).
    subnormal: u32,
    /// ±0 values.
    zeros: u32,
    /// Finite non-zero normal values outside the plausible exponent window.
    implausible: u32,
    /// Transitions with identical bits.
    xor_zero: u32,
    /// Sum of meaningful XOR bits over non-zero XORs.
    meaningful_bits: u64,
    /// Transitions flipping the sign bit.
    sign_changes: u32,
}

/// Inherent methods of [`SampleCounts`].
impl SampleCounts {
    /// Converts the counters into ratios.
    fn profile(
        &self,
        width: FloatWidth,
        value_count: usize,
        exponent_distinct: u16,
    ) -> FloatProfile {
        let ratio = |count: u32, total: u32| {
            if total == 0 {
                0.0
            } else {
                count as f32 / total as f32
            }
        };
        let changing = self.transitions - self.xor_zero;
        FloatProfile {
            width,
            value_count,
            sampled_values: self.values,
            non_finite_ratio: ratio(self.non_finite, self.values),
            subnormal_ratio: ratio(self.subnormal, self.values),
            zero_ratio: ratio(self.zeros, self.values),
            implausible_magnitude_ratio: ratio(self.implausible, self.values),
            xor_zero_ratio: ratio(self.xor_zero, self.transitions),
            mean_xor_meaningful_bits: if changing == 0 {
                0.0
            } else {
                self.meaningful_bits as f32 / changing as f32
            },
            exponent_distinct,
            sign_change_ratio: ratio(self.sign_changes, self.transitions),
        }
    }
}

/// Applies the float admission rules to one profile.
pub fn admit_float(profile: &FloatProfile) -> Result<(), FloatRejection> {
    if profile.value_count < LANE_WINDOW_VALUES {
        return Err(FloatRejection::TooShort);
    }
    if profile.non_finite_ratio > MAX_NON_FINITE_RATIO {
        return Err(FloatRejection::NonFinite);
    }
    if profile.zero_ratio > MAX_ZERO_RATIO {
        return Err(FloatRejection::ZeroDominated);
    }
    if profile.subnormal_ratio > MAX_SUBNORMAL_RATIO {
        return Err(FloatRejection::Subnormal);
    }
    if profile.exponent_distinct > MAX_EXPONENT_DISTINCT {
        return Err(FloatRejection::ExponentSpread);
    }
    if profile.implausible_magnitude_ratio > MAX_IMPLAUSIBLE_RATIO {
        return Err(FloatRejection::ImplausibleMagnitude);
    }
    if profile.mean_xor_meaningful_bits > MAX_MEANINGFUL_FRACTION * profile.width.bits() as f32 {
        return Err(FloatRejection::NoisyMantissa);
    }
    Ok(())
}

/// Classifies `input` as float data: profiles both widths and keeps the admitted one with the
/// lower proxy cost per byte (f64 wins ties).
///
/// f64 data read as f32 has random mantissas in every other value, and f32 data read as f64
/// has unstable exponents, so the wrong width normally fails admission on its own; the cost
/// comparison only breaks the remaining ties.
pub fn float_prefilter(input: &[u8]) -> FloatPrefilter {
    let mut admitted: Option<FloatProfile> = None;
    let mut rejection = None;
    for width in FloatWidth::ALL {
        let Some(profile) = float_profile(input, width) else {
            if width == FloatWidth::F64 {
                rejection = Some(FloatRejection::TooShort);
            }
            continue;
        };
        match admit_float(&profile) {
            Ok(()) => {
                let better = admitted
                    .is_none_or(|best| profile.proxy_bits_per_byte() < best.proxy_bits_per_byte());
                if better {
                    admitted = Some(profile);
                }
            }
            Err(reason) if width == FloatWidth::F64 => rejection = Some(reason),
            Err(_) => {}
        }
    }
    FloatPrefilter {
        rejection: if admitted.is_some() { None } else { rejection },
        admitted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
