use ace_core::CompressionProfile;

/// Controls how much deterministic feature analysis ACE performs before candidate generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisLevel {
    /// Throughput-oriented analysis. Skips features that do not affect FAST planner decisions.
    Fast,
    /// Full general-purpose feature analysis used by BALANCED.
    Standard,
    /// Full quality-oriented analysis used by DENSE; reserved for future DENSE-only features.
    Dense,
}

/// Inherent methods of [`AnalysisLevel`].
impl AnalysisLevel {
    /// Maps a public compression profile to its deterministic analysis budget.
    pub fn for_profile(profile: CompressionProfile) -> Self {
        match profile {
            CompressionProfile::Fast => Self::Fast,
            CompressionProfile::Balanced => Self::Standard,
            CompressionProfile::Dense => Self::Dense,
        }
    }
}
