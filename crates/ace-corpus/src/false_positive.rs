//! False-positive corpus of the Float lane (ACE 0.5.0).
//!
//! Buffers that a naive float detector would accept: NaN- and Inf-heavy series, random bits
//! with plausible exponents, raw random bytes, text and small integers. The release gate is
//! *zero* blocks routed to the Float lane on this corpus (and on Corpus V3).

use crate::{Workload, XorShift32};

/// One named false-positive case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalsePositiveCase {
    /// Smooth f64 series where every 8th value is NaN.
    NanHeavy,
    /// Smooth f64 series where every 8th value is +Inf.
    InfHeavy,
    /// Random f64 bit patterns restricted to finite values.
    RandomFiniteF64,
    /// Uniformly random bytes (stand-in for compressed or encrypted data).
    RandomBytes,
    /// Repeating key=value log text.
    Text,
    /// Small u64 integers (`i % 1000`).
    SmallU64,
}

/// Inherent methods of [`FalsePositiveCase`].
impl FalsePositiveCase {
    /// Every case, in report order.
    pub const ALL: [Self; 6] = [
        Self::NanHeavy,
        Self::InfHeavy,
        Self::RandomFiniteF64,
        Self::RandomBytes,
        Self::Text,
        Self::SmallU64,
    ];

    /// Stable kebab-case name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NanHeavy => "fp-nan-heavy",
            Self::InfHeavy => "fp-inf-heavy",
            Self::RandomFiniteF64 => "fp-random-finite-f64",
            Self::RandomBytes => "fp-random-bytes",
            Self::Text => "fp-text",
            Self::SmallU64 => "fp-small-u64",
        }
    }

    /// Generates exactly `bytes` bytes of this case.
    pub fn generate(self, bytes: usize) -> Vec<u8> {
        let values = bytes.div_ceil(8);
        let smooth = |i: usize| 20.0 + i as f64 * 0.001;
        let mut rng = XorShift32::new(0x0F95_0001);
        let mut next_u64 = move || (u64::from(rng.next_u32()) << 32) | u64::from(rng.next_u32());
        let mut out: Vec<u8> = match self {
            Self::NanHeavy => (0..values)
                .flat_map(|i| if i % 8 == 0 { f64::NAN } else { smooth(i) }.to_le_bytes())
                .collect(),
            Self::InfHeavy => (0..values)
                .flat_map(|i| if i % 8 == 0 { f64::INFINITY } else { smooth(i) }.to_le_bytes())
                .collect(),
            Self::RandomFiniteF64 => (0..values)
                .flat_map(|_| {
                    // Random sign and mantissa, exponent field uniform in 0..0x7fe (finite).
                    let bits = next_u64();
                    let exponent = (bits >> 52) % 0x7fe;
                    ((bits & !(0x7ffu64 << 52)) | (exponent << 52)).to_le_bytes()
                })
                .collect(),
            Self::RandomBytes => Workload::Random.generate(bytes),
            Self::Text => b"timestamp=1700000000123 sensor=temp-01 value=21.5 unit=C status=ok\n"
                .iter()
                .copied()
                .cycle()
                .take(bytes)
                .collect(),
            Self::SmallU64 => (0..values as u64)
                .flat_map(|i| (i % 1000).to_le_bytes())
                .collect(),
        };
        out.truncate(bytes);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every case is deterministic and exactly `bytes` long.
    #[test]
    fn cases_are_deterministic() {
        for case in FalsePositiveCase::ALL {
            let a = case.generate(10_001);
            assert_eq!(a.len(), 10_001, "{}", case.name());
            assert_eq!(a, case.generate(10_001), "{}", case.name());
        }
    }
}
