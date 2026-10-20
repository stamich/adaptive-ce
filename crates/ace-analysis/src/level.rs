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

    /// True when analysing a block at `self` and at `other` produces the same
    /// [`ace_core::BlockProfile`], so a caller that needs both may analyse once.
    ///
    /// STANDARD and DENSE run the same analysis today ("reserved for future DENSE-only
    /// features"); FAST skips features and is only equivalent to itself. A DENSE-only feature
    /// must change this answer, which `tests/analyzer.rs` checks against real data.
    pub fn same_analysis_as(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Fast, Self::Fast) | (Self::Standard | Self::Dense, Self::Standard | Self::Dense)
        )
    }
}
