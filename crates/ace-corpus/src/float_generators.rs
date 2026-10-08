//! Corpus V4 generators (ACE 0.5.0): floating-point series and sparse-change integers.
//!
//! Determinism rule: only IEEE-754 `+ − × ÷`, `round` and integer arithmetic are used. Those
//! operations are correctly rounded and therefore bit-identical on every platform; transcendental
//! functions (`sin`, `exp`, `ln`, `powf`) come from the platform libm and may differ in the last
//! bit, which would make golden hashes platform-dependent. Periodic shapes are therefore built
//! from integer phase counters, never from `sin`.

use crate::{XorShift32, XorShift64};

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

/// Uniform integer in `-span..=span` from a 32-bit xorshift.
fn signed_step(rng: &mut XorShift32, span: u32) -> i64 {
    (rng.next_u32() % (2 * span + 1)) as i64 - span as i64
}

/// Constant 21.5.
pub(crate) fn f64_constant(bytes: usize) -> Vec<u8> {
    lanes(bytes, |_| 21.5f64.to_le_bytes())
}

/// Plateaus of 1 000 samples that move by ±0.5.
pub(crate) fn f64_step(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x05EE_DF64);
    let mut value = 100.0f64;
    lanes(bytes, |index| {
        if index > 0 && index % 1_000 == 0 {
            value += if rng.next_u32() & 1 == 0 { 0.5 } else { -0.5 };
        }
        value.to_le_bytes()
    })
}

/// Integrated small increments: `v += k · 0.001`, `k ∈ −8..=8`.
pub(crate) fn f64_smooth(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0005_A00F);
    let mut value = 20.0f64;
    lanes(bytes, |_| {
        value += signed_step(&mut rng, 8) as f64 * 0.001;
        value.to_le_bytes()
    })
}

/// Temperature-like sensor: slow trend with a daily-like integer sawtooth, quantised to 0.1 °C.
pub(crate) fn f64_sensor_temperature(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0007_E3F0);
    let mut trend = 0.0f64;
    lanes(bytes, |index| {
        trend += signed_step(&mut rng, 2) as f64 * 0.01;
        // Triangular "day" of 8 640 samples built from an integer phase (no sin()).
        let phase = (index % 8_640) as i64;
        let day = (4_320 - (phase - 4_320).abs()) as f64 / 4_320.0 * 3.0;
        let value = ((20.0 + trend + day) * 10.0).round() / 10.0;
        value.to_le_bytes()
    })
}

/// Price random walk in cents converted to a decimal float (`cents / 100`).
pub(crate) fn f64_financial_price(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0000_C0DE);
    let mut cents = 1_234_567i64;
    lanes(bytes, |_| {
        cents = (cents + signed_step(&mut rng, 5)).max(1);
        (cents as f64 / 100.0).to_le_bytes()
    })
}

/// [`f64_smooth`] with noise in the 20 lowest mantissa bits (measurement noise).
pub(crate) fn f64_noisy(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift64::new(0x0015_E0F6);
    let smooth = f64_smooth(bytes.div_ceil(8) * 8);
    let mut out: Vec<u8> = smooth
        .chunks_exact(8)
        .flat_map(|chunk| {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(chunk);
            let bits = u64::from_le_bytes(raw) ^ (rng.next_u64() & 0xF_FFFF);
            bits.to_le_bytes()
        })
        .collect();
    out.truncate(bytes);
    out
}

/// Random IEEE bit patterns, NaN-free (an all-ones exponent is cleared by one bit).
pub(crate) fn f64_random(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift64::new(0x00F6_4A2D);
    lanes(bytes, |_| {
        let mut bits = rng.next_u64();
        if (bits >> 52) & 0x7FF == 0x7FF {
            bits &= !(1u64 << 52);
        }
        bits.to_le_bytes()
    })
}

/// Special values: ±0, NaN payloads, ±∞, subnormals, extremes — interleaved with a ramp.
pub(crate) fn f64_special(bytes: usize) -> Vec<u8> {
    const SPECIALS: [u64; 10] = [
        0x0000_0000_0000_0000,
        0x8000_0000_0000_0000,
        0x7FF8_0000_0000_0000,
        0x7FF0_0000_DEAD_BEEF,
        0xFFF8_0000_0000_0001,
        0x7FF0_0000_0000_0000,
        0xFFF0_0000_0000_0000,
        0x0000_0000_0000_0001,
        0x7FEF_FFFF_FFFF_FFFF,
        0xFFEF_FFFF_FFFF_FFFF,
    ];
    lanes(bytes, |index| {
        let bits = if index % 4 == 0 {
            SPECIALS[(index / 4) % SPECIALS.len()]
        } else {
            (1.0f64 + index as f64 * 0.5).to_bits()
        };
        bits.to_le_bytes()
    })
}

/// f32 version of [`f64_smooth`].
pub(crate) fn f32_smooth(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0F32_5A00);
    let mut value = 20.0f32;
    lanes(bytes, |_| {
        value += signed_step(&mut rng, 8) as f32 * 0.001;
        value.to_le_bytes()
    })
}

/// f32 sensor quantised to 0.01 with a slow integer-driven drift.
pub(crate) fn f32_sensor(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x0F32_5E50);
    let mut hundredths = 101_325i64;
    lanes(bytes, |_| {
        hundredths += signed_step(&mut rng, 3);
        (hundredths as f32 / 100.0).to_le_bytes()
    })
}

/// u32 status value that changes on average every ~200 samples (small steps, rare big jumps).
pub(crate) fn int_sparse_change(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x05A5_E000);
    let mut value = 5_000u32;
    lanes(bytes, |_| {
        let draw = rng.next_u32();
        if draw.is_multiple_of(200) {
            value = if draw.is_multiple_of(2_000) {
                rng.next_u32()
            } else {
                value.wrapping_add((draw >> 8) % 16).wrapping_sub(8)
            };
        }
        value.to_le_bytes()
    })
}

/// u64 event counter (+1…+3) that resets to zero every 5 000 samples.
pub(crate) fn int_counter_reset(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x00C0_0E7E);
    let mut value = 0u64;
    lanes(bytes, |index| {
        value = if index % 5_000 == 0 {
            0
        } else {
            value + 1 + (rng.next_u32() % 3) as u64
        };
        value.to_le_bytes()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// First value of every float generator (guards the generators against accidental change).
    #[test]
    fn float_generators_are_stable() {
        let first = |data: Vec<u8>| {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(&data[..8]);
            f64::from_le_bytes(raw)
        };
        assert_eq!(first(f64_constant(8)), 21.5);
        assert_eq!(first(f64_step(8)), 100.0);
        assert_eq!(first(f64_special(8)).to_bits(), 0);
        assert!(f64_random(1 << 16)
            .chunks_exact(8)
            .all(|c| { (u64::from_le_bytes(c.try_into().unwrap()) >> 52) & 0x7FF != 0x7FF }));
    }
}
