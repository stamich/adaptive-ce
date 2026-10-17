use ace_cost::EstimatedCandidate;

/// Coarse confidence band derived from analytical versus real-codec micro-trial agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateConfidence {
    /// Analytical and sampled projections differ by less than five percent.
    High,
    /// Analytical and sampled projections differ by five to fifteen percent.
    Medium,
    /// Analytical and sampled projections differ by more than fifteen percent or no reliable sample exists.
    Low,
}

/// Inherent methods of [`EstimateConfidence`].
impl EstimateConfidence {
    /// Classifies one candidate by analytical/sample disagreement.
    pub fn from_candidate(candidate: &EstimatedCandidate) -> Self {
        let Some(sampled) = candidate.sampled_size_bytes else {
            return Self::Low;
        };
        let analytical = candidate.analytical_size_bytes.max(1);
        let disagreement_ppm = (sampled.abs_diff(analytical) as u128)
            .saturating_mul(1_000_000)
            .checked_div(analytical as u128)
            .unwrap_or(0);
        if disagreement_ppm < 50_000 {
            Self::High
        } else if disagreement_ppm <= 150_000 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    /// Returns true when a larger second-stage micro-trial is justified.
    pub fn needs_second_stage(self) -> bool {
        !matches!(self, Self::High)
    }
}
