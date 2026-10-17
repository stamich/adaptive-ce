//! ALP (adaptive lossless floating point, TS1 mode 4, Format 1.5): decimal floats as integers.
//!
//! Many measured series are decimals in disguise (`12.37`, `0.5`, `1013.25`): multiplying by
//! `10^e` and `10^-f` gives an integer `d` that converts back to exactly the same bits. ALP
//! stores `d` with a frame of reference and a fixed bit width, and keeps the few values that do
//! not survive the round trip (`-0.0`, NaN, ±Inf, full-precision noise) verbatim as exceptions.
//!
//! ```text
//! stream:  e:5  f:5  width:7  base:64  gamma(exception_count)
//!          exception_count x ( gamma(gap) raw_bits:lane_bits )      gap = position - previous - 1
//!          value_count x offset:width                                offset = d - base (0 for an exception)
//! ```
//!
//! * `encode: d = round(v * 10^e * 10^-f)`, `decode: v = d * 10^f * 10^-e`, both evaluated in
//!   IEEE-754 binary64 with the multiplications in exactly this order (f32 lanes are widened,
//!   decoded in binary64 and narrowed; the encoder keeps a value only if the decoded bits equal
//!   the original bits, so the formula can never lose data).
//! * `0 <= f <= e <= 18` and `|v * 10^e * 10^-f| < 2^53`, otherwise the value is an exception.
//! * `(e, f)` is chosen from a deterministic sample of at most 64 values, four windows of 16
//!   consecutive values spread over the block (see [`choose_params`]);
//!   the choice only affects the size, never correctness.
//! * The encoder is written once against [`BitSink`], so [`ts1_encoded_len`](super::ts1_encoded_len)
//!   is exact by construction.

use super::gamma::put_gamma;
use super::sink::BitSink;

/// Largest decimal exponent (`10^18` is the largest power of ten below `2^63`).
pub(crate) const MAX_EXPONENT: u8 = 18;
/// Values whose scaled magnitude reaches `2^53` are exceptions (the integer is not exact).
const MAX_MAGNITUDE: f64 = 9_007_199_254_740_992.0;
/// Number of sample windows and values per window used to choose `(e, f)`.
const SAMPLE_WINDOWS: usize = 4;
/// Consecutive values per sample window (consecutive, so a periodic series cannot alias).
const WINDOW_VALUES: usize = 16;

/// `10^k` (exact in binary64 for `k <= 22`).
const POW10: [f64; 19] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18,
];
/// `10^-k`, the correctly rounded binary64 constant.
const INV_POW10: [f64; 19] = [
    1e0, 1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12, 1e-13, 1e-14,
    1e-15, 1e-16, 1e-17, 1e-18,
];

/// Decimal scaling of one block; the fields are private so `e`, `f` always index the tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlpParams {
    /// Exponent applied by the encoder (`10^e`).
    e: u8,
    /// Exponent removed by the encoder (`10^-f`), `f <= e`.
    f: u8,
}

/// Inherent methods of [`AlpParams`].
impl AlpParams {
    /// Validates `f <= e <= 18`.
    pub(crate) fn new(e: u64, f: u64) -> Option<Self> {
        let e = u8::try_from(e).ok().filter(|&e| e <= MAX_EXPONENT)?;
        let f = u8::try_from(f).ok().filter(|&f| f <= e)?;
        Some(Self { e, f })
    }
}

/// `table[k]`, or 1.0 for an index the constructor already excluded.
fn table(table: &[f64; 19], k: u8) -> f64 {
    table.get(usize::from(k)).copied().unwrap_or(1.0)
}

/// The lane's value widened to binary64 (exact for f32).
fn widen(bits: u64, lane_bits: u32) -> f64 {
    if lane_bits == 32 {
        f64::from(f32::from_bits(bits as u32))
    } else {
        f64::from_bits(bits)
    }
}

/// Bits of the lane holding the binary64 value `x` (rounded to f32 for 32-bit lanes).
// Narrowing to f32 is the point: the encoder verifies the round trip.
#[allow(clippy::cast_possible_truncation)]
fn narrow(x: f64, lane_bits: u32) -> u64 {
    if lane_bits == 32 {
        u64::from((x as f32).to_bits())
    } else {
        x.to_bits()
    }
}

/// Lane bits for the integer `d`: `d * 10^f * 10^-e`.
pub(crate) fn decode_digits(d: i64, params: AlpParams, lane_bits: u32) -> u64 {
    narrow(
        d as f64 * table(&POW10, params.f) * table(&INV_POW10, params.e),
        lane_bits,
    )
}

/// The integer for a lane value, `None` when it is not reproduced bit for bit.
fn digits_of(bits: u64, params: AlpParams, lane_bits: u32) -> Option<i64> {
    let x = widen(bits, lane_bits) * table(&POW10, params.e) * table(&INV_POW10, params.f);
    if !x.is_finite() || x.abs() >= MAX_MAGNITUDE {
        return None;
    }
    let d = x.round() as i64;
    (decode_digits(d, params, lane_bits) == bits).then_some(d)
}

/// Little-endian lanes of `B` bytes.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn lanes<const B: usize>(input: &[u8]) -> impl Iterator<Item = u64> + '_ {
    input.chunks_exact(B).map(ace_core::read_lane::<B>)
}

/// Bits needed for the unsigned range `max - min`.
fn range_bits(min: i64, max: i64) -> u32 {
    let range = (i128::from(max) - i128::from(min)) as u64;
    64 - range.leading_zeros()
}

/// The `(e, f)` with the smallest estimated size on the sample. Ties go to the lower `e`, then
/// the lower `f`.
fn choose_params(sample: &[u64], lane_bits: u32) -> AlpParams {
    let mut best: Option<(u64, AlpParams)> = None;
    for e in 0..=u64::from(MAX_EXPONENT) {
        for f in 0..=e {
            let Some(params) = AlpParams::new(e, f) else {
                continue;
            };
            let (mut min, mut max, mut exceptions, mut kept) = (i64::MAX, i64::MIN, 0u64, 0u64);
            for &bits in sample {
                match digits_of(bits, params, lane_bits) {
                    Some(d) => {
                        min = min.min(d);
                        max = max.max(d);
                        kept += 1;
                    }
                    None => exceptions += 1,
                }
            }
            let width = if kept == 0 {
                0
            } else {
                u64::from(range_bits(min, max))
            };
            let cost = exceptions * u64::from(lane_bits + 8) + sample.len() as u64 * width;
            if best.is_none_or(|(best_cost, _)| cost < best_cost) {
                best = Some((cost, params));
            }
        }
    }
    best.map_or(AlpParams { e: 0, f: 0 }, |(_, params)| params)
}

/// Values of [`SAMPLE_WINDOWS`] windows of [`WINDOW_VALUES`] consecutive lanes, evenly spread
/// (the whole block when it is shorter than the windows together).
fn sample_lanes<const B: usize>(input: &[u8], count: usize) -> Vec<u64> {
    if count <= SAMPLE_WINDOWS * WINDOW_VALUES {
        return lanes::<B>(input).collect();
    }
    let span = count - WINDOW_VALUES;
    let mut sample = Vec::with_capacity(SAMPLE_WINDOWS * WINDOW_VALUES);
    for window in 0..SAMPLE_WINDOWS {
        let start = span * window / (SAMPLE_WINDOWS - 1);
        sample.extend(lanes::<B>(&input[start * B..]).take(WINDOW_VALUES));
    }
    sample
}

/// Appends `width` low bits of `value`; a zero width writes nothing.
fn put_bits<S: BitSink>(sink: &mut S, value: u64, width: u32) {
    if width > 0 {
        sink.put(value, width);
    }
}

/// Encodes all lanes of `input` (`B` bytes each) into `sink`; writes nothing for no lanes.
pub(crate) fn encode_alp<const B: usize, S: BitSink>(input: &[u8], sink: &mut S) {
    let count = input.len() / B;
    if count == 0 {
        return;
    }
    let lane_bits = (B * 8) as u32;
    let sample = sample_lanes::<B>(input, count);
    let params = choose_params(&sample, lane_bits);

    let mut exceptions: Vec<usize> = Vec::new();
    let (mut min, mut max) = (i64::MAX, i64::MIN);
    for (index, bits) in lanes::<B>(input).enumerate() {
        match digits_of(bits, params, lane_bits) {
            Some(d) => {
                min = min.min(d);
                max = max.max(d);
            }
            None => exceptions.push(index),
        }
    }
    let (base, width) = if exceptions.len() == count {
        (0, 0)
    } else {
        (min, range_bits(min, max))
    };

    put_bits(sink, u64::from(params.e), 5);
    put_bits(sink, u64::from(params.f), 5);
    put_bits(sink, u64::from(width), 7);
    put_bits(sink, base as u64, 64);
    put_gamma(sink, exceptions.len() as u64);
    let mut next = 0usize;
    for &position in &exceptions {
        put_gamma(sink, (position - next) as u64);
        put_bits(
            sink,
            lanes::<B>(&input[position * B..]).next().unwrap_or(0),
            lane_bits,
        );
        next = position + 1;
    }
    let mut pending = exceptions.iter().copied().peekable();
    for (index, bits) in lanes::<B>(input).enumerate() {
        let offset = if pending.next_if_eq(&index).is_some() {
            0
        } else {
            let d = digits_of(bits, params, lane_bits).unwrap_or(base);
            (i128::from(d) - i128::from(base)) as u64
        };
        put_bits(sink, offset, width);
    }
}
