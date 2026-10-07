//! Hardware CRC32C (Castagnoli) with three independent dependency chains.
//!
//! The SSE4.2 `crc32` instruction has a latency of 3 cycles but a throughput of 1 per cycle, so
//! a single running CRC uses only a third of the unit. The block is split into three equal
//! stripes whose CRCs are computed in one interleaved loop and then merged with GF(2)
//! arithmetic (`crc(A‖B) = crc(A)·x^(8|B|) ⊕ crc(B)`), which is about 2.5–3× faster than one
//! chain. Results are bit-identical to every standard CRC32C implementation.

/// Reflected CRC32C polynomial.
const POLY: u32 = 0x82F6_3B78;

/// `x^0` in the reflected bit order used by the CRC register.
const X0: u32 = 1 << 31;

/// Stripes shorter than this are not worth splitting (merge cost ≈ two short multiplications).
const MIN_STRIPE_BYTES: usize = 256;

/// Multiplies two polynomials modulo [`POLY`] (reflected representation).
const fn mul_mod_poly(a: u32, mut b: u32) -> u32 {
    let mut mask = X0;
    let mut product = 0u32;
    while mask != 0 {
        if a & mask != 0 {
            product ^= b;
        }
        mask >>= 1;
        b = if b & 1 != 0 { (b >> 1) ^ POLY } else { b >> 1 };
    }
    product
}

/// `POWERS[k] = x^(2^k) mod POLY`, computed at compile time.
const POWERS: [u32; 64] = {
    let mut table = [0u32; 64];
    table[0] = X0 >> 1; // x^1
    let mut k = 1;
    while k < 64 {
        table[k] = mul_mod_poly(table[k - 1], table[k - 1]);
        k += 1;
    }
    table
};

/// Returns `x^(8 * bytes) mod POLY` by binary exponentiation over [`POWERS`].
fn x_pow_bytes(bytes: usize) -> u32 {
    let mut result = X0;
    let mut exponent = (bytes as u64) * 8;
    let mut k = 0;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul_mod_poly(POWERS[k], result);
        }
        exponent >>= 1;
        k += 1;
    }
    result
}

/// Returns the CRC32C of `bytes` using SSE4.2, or `None` when the CPU lacks SSE4.2 or
/// `ACE_SIMD=scalar` is set (callers then use a portable implementation).
pub fn crc32c_hardware(bytes: &[u8]) -> Option<u32> {
    if !crate::backend::sse42_available() {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: SSE4.2 support was verified at runtime; the kernel reads only `bytes`.
        Some(!unsafe { sse42::crc32c_raw(!0, bytes) })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = bytes;
        None
    }
}

/// Name of the CRC32C implementation in use (`"sse4.2-3way"` or `"portable"`), for telemetry.
pub fn crc32c_backend_name() -> &'static str {
    if crate::backend::sse42_available() {
        "sse4.2-3way"
    } else {
        "portable"
    }
}

/// Merges the raw CRC state of three consecutive stripes of `stripe` bytes each.
fn merge_stripes(state0: u32, state1: u32, state2: u32, stripe: usize) -> u32 {
    let shift = x_pow_bytes(stripe);
    mul_mod_poly(mul_mod_poly(state0, shift) ^ state1, shift) ^ state2
}

/// SSE4.2 kernels (`#[target_feature]`; callers must check CPU support first).
#[cfg(target_arch = "x86_64")]
mod sse42 {
    use std::arch::x86_64::{_mm_crc32_u64, _mm_crc32_u8};

    use super::{merge_stripes, MIN_STRIPE_BYTES};

    /// Updates a raw (pre-/post-inversion free) CRC32C state with `bytes`.
    ///
    /// A safe `#[target_feature]` function: the CRC intrinsics have no memory operands, so the
    /// body needs no `unsafe`. Callers without SSE4.2 enabled at compile time must call it
    /// inside `unsafe` after runtime detection (see [`super::crc32c_hardware`]).
    #[target_feature(enable = "sse4.2")]
    pub(super) fn crc32c_raw(state: u32, bytes: &[u8]) -> u32 {
        let stripe = (bytes.len() / 3) & !7;
        let (mut state, rest) = if stripe >= MIN_STRIPE_BYTES {
            let (a, tail) = bytes.split_at(stripe);
            let (b, tail) = tail.split_at(stripe);
            let (c, rest) = tail.split_at(stripe);
            let (mut s0, mut s1, mut s2) = (state as u64, 0u64, 0u64);
            for ((wa, wb), wc) in a
                .chunks_exact(8)
                .zip(b.chunks_exact(8))
                .zip(c.chunks_exact(8))
            {
                s0 = _mm_crc32_u64(s0, word(wa));
                s1 = _mm_crc32_u64(s1, word(wb));
                s2 = _mm_crc32_u64(s2, word(wc));
            }
            (merge_stripes(s0 as u32, s1 as u32, s2 as u32, stripe), rest)
        } else {
            (state, bytes)
        };
        let mut words = rest.chunks_exact(8);
        for chunk in &mut words {
            state = _mm_crc32_u64(state as u64, word(chunk)) as u32;
        }
        for &byte in words.remainder() {
            state = _mm_crc32_u8(state, byte);
        }
        state
    }

    /// Loads 8 little-endian bytes.
    #[inline(always)]
    fn word(chunk: &[u8]) -> u64 {
        let mut word = [0u8; 8];
        word.copy_from_slice(chunk);
        u64::from_le_bytes(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bit-at-a-time reference CRC32C.
    fn reference(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ POLY
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    /// Matches the reference for every length around the stripe threshold and for large blocks.
    #[test]
    fn hardware_crc_matches_reference() {
        let Some(_) = crc32c_hardware(b"") else {
            return;
        };
        let mut state = 0x1234_5678u64;
        let data: Vec<u8> = (0..300_010)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        assert_eq!(crc32c_hardware(b"123456789"), Some(0xE306_9283));
        for len in (0..2_000).chain([3 * 256, 3 * 256 + 7, 65_536, 262_144, 299_999]) {
            for offset in [0usize, 1, 5] {
                let slice = &data[offset..offset + len];
                assert_eq!(
                    crc32c_hardware(slice),
                    Some(reference(slice)),
                    "len={len} offset={offset}"
                );
            }
        }
    }

    /// The GF(2) shift operator agrees with feeding zero bytes through the register.
    #[test]
    fn shift_operator_matches_zero_feed() {
        for bytes in [0usize, 1, 3, 8, 1000] {
            let mut state = 0xDEAD_BEEFu32;
            for _ in 0..bytes * 8 {
                state = if state & 1 != 0 {
                    (state >> 1) ^ POLY
                } else {
                    state >> 1
                };
            }
            assert_eq!(mul_mod_poly(0xDEAD_BEEF, x_pow_bytes(bytes)), state);
        }
    }
}
