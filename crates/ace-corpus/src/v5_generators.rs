//! Corpus V5 generators (ACE 0.6.1): integer series with small non-zero changes and decimal
//! floats (ALP).
//!
//! Same determinism rule as Corpus V4: integer arithmetic only.

use crate::XorShift32;

/// Emits little-endian lanes produced by `next(index)` until `bytes` bytes exist, then
/// truncates the last partial lane.
fn lanes<const N: usize>(bytes: usize, mut next: impl FnMut(usize) -> [u8; N]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes + N);
    let mut index = 0usize;
    while out.len() < bytes {
        out.extend_from_slice(&next(index));
        index += 1;
    }
    out.truncate(bytes);
    out
}

/// u32 gauge that moves by ±1 on a quarter of the samples and stays put otherwise (a slowly
/// drifting integer reading): too many changes for the sparse-change prefilter, too many
/// repeats for a fixed bit width.
pub(crate) fn int_slow_drift(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0D21_F700);
    let mut value = 1_000_000u32;
    lanes(bytes, |_| {
        let draw = rng.next_u32();
        if draw % 100 < 25 {
            value = if draw >> 16 & 1 == 0 {
                value.wrapping_add(1)
            } else {
                value.wrapping_sub(1)
            };
        }
        value.to_le_bytes()
    })
}

/// u32 value whose step is `2^k` with probability `2^-(k+1)` (half the samples repeat, the rest
/// have a geometric magnitude): small deltas with a heavy tail.
pub(crate) fn int_geometric_delta(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x6E0_4E7A);
    let mut value = 5_000u32;
    lanes(bytes, |_| {
        let draw = rng.next_u32() | 1 << 31;
        let magnitude = draw.trailing_zeros().min(12);
        let step = if magnitude == 0 {
            0
        } else {
            1u32 << (magnitude - 1)
        };
        value = if draw >> 30 & 1 == 0 {
            value.wrapping_add(step)
        } else {
            value.wrapping_sub(step)
        };
        value.to_le_bytes()
    })
}

/// f64 price in cents (a random walk of up to ±20 cents) divided by 100: the decimals ALP is
/// built for, with a few `NaN` / `-0.0` / `Inf` records that become exceptions.
pub(crate) fn f64_decimal_price(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0DEC_1F64);
    let mut cents = 2_499_i64;
    lanes(bytes, |index| {
        let draw = rng.next_u32();
        cents = (cents + i64::from(draw % 41) - 20).max(1);
        let value = match index % 4_099 {
            1_000 => f64::NAN,
            2_000 => -0.0,
            3_000 => f64::INFINITY,
            _ => cents as f64 / 100.0,
        };
        value.to_le_bytes()
    })
}

/// f32 temperature in tenths of a degree (slow drift of up to ±3 tenths) divided by 10.
pub(crate) fn f32_decimal_sensor(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0DEC_1F32);
    let mut tenths = 215_i32;
    lanes(bytes, |_| {
        let draw = rng.next_u32();
        tenths = (tenths + (draw % 7) as i32 - 3).clamp(-400, 1_200);
        (tenths as f32 / 10.0).to_le_bytes()
    })
}
