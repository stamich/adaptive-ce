use ace_core::CompressionProfile;

/// Deterministic policy controlling when scalar rANS is worth considering in ACE 0.2.1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntropySelectionPolicy {
    /// Minimum primary-stream size before rANS may be considered.
    pub min_rans_input_size: usize,
    /// Minimum fractional size advantage expected by a speed-oriented profile.
    pub min_rans_gain_fraction: f32,
}

impl EntropySelectionPolicy {
    /// Returns the calibrated policy for one public compression profile.
    pub fn for_profile(profile: CompressionProfile) -> Self {
        match profile {
            CompressionProfile::Fast => Self { min_rans_input_size: 16 * 1024, min_rans_gain_fraction: 0.05 },
            CompressionProfile::Balanced => Self { min_rans_input_size: 4 * 1024, min_rans_gain_fraction: 0.02 },
            CompressionProfile::Dense => Self { min_rans_input_size: 1024, min_rans_gain_fraction: 0.0 },
        }
    }

    /// Returns true when the input is large enough to amortize scalar rANS metadata/setup costs.
    pub fn input_allows_rans(&self, input_size: usize) -> bool { input_size >= self.min_rans_input_size }
}
