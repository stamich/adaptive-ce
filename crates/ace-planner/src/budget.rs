use ace_core::{BlockProfile, CompressionProfile};

/// Coarse data class used only to allocate planner work, never to change decoder semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerDataClass {
    /// Blocks dominated by zeros or long byte runs.
    ZeroHeavy,
    /// Blocks that look effectively incompressible.
    Incompressible,
    /// Blocks where byte deltas indicate numeric/monotonic structure.
    Numeric,
    /// Remaining compressible structured data.
    Structured,
}

/// Inherent methods of [`PlannerDataClass`].
impl PlannerDataClass {
    /// Classifies a block using features already present in `BlockProfile`.
    pub fn classify(profile: &BlockProfile) -> Self {
        if profile.zero_ratio >= 0.985 || profile.run_score >= 0.92 {
            Self::ZeroHeavy
        } else if profile.incompressibility_score >= 0.96
            && profile.repetition_score < 0.01
            && profile.run_score < 0.01
        {
            Self::Incompressible
        } else if profile.delta_score >= 0.08 {
            Self::Numeric
        } else {
            Self::Structured
        }
    }
}

/// Deterministic per-block budget for Hybrid LZ micro-trials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanningBudget {
    /// Maximum number of LZ candidates that may run a stage-one micro-trial.
    pub hybrid_stage1_candidates: usize,
    /// Maximum number of LZ candidates that may run a stage-two micro-trial.
    pub hybrid_stage2_candidates: usize,
}

/// Inherent methods of [`PlanningBudget`].
impl PlanningBudget {
    /// Returns the profile/data-class work budget used by Planner V3.6.
    pub fn for_block(profile: CompressionProfile, class: PlannerDataClass) -> Self {
        match (profile, class) {
            (_, PlannerDataClass::ZeroHeavy | PlannerDataClass::Incompressible) => Self {
                hybrid_stage1_candidates: 0,
                hybrid_stage2_candidates: 0,
            },
            (CompressionProfile::Fast, _) => Self {
                hybrid_stage1_candidates: 1,
                hybrid_stage2_candidates: 0,
            },
            (CompressionProfile::Balanced, PlannerDataClass::Numeric) => Self {
                hybrid_stage1_candidates: 6,
                hybrid_stage2_candidates: 2,
            },
            (CompressionProfile::Balanced, PlannerDataClass::Structured) => Self {
                hybrid_stage1_candidates: 4,
                hybrid_stage2_candidates: 1,
            },
            (CompressionProfile::Dense, PlannerDataClass::Numeric) => Self {
                hybrid_stage1_candidates: 8,
                hybrid_stage2_candidates: 3,
            },
            (CompressionProfile::Dense, PlannerDataClass::Structured) => Self {
                hybrid_stage1_candidates: 6,
                hybrid_stage2_candidates: 2,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numeric data retains more LZ work than structured data in BALANCED.
    #[test]
    fn numeric_keeps_wide_lz_budget() {
        let numeric =
            PlanningBudget::for_block(CompressionProfile::Balanced, PlannerDataClass::Numeric);
        let structured =
            PlanningBudget::for_block(CompressionProfile::Balanced, PlannerDataClass::Structured);
        assert!(numeric.hybrid_stage1_candidates >= structured.hybrid_stage1_candidates);
        assert!(numeric.hybrid_stage2_candidates >= structured.hybrid_stage2_candidates);
    }

    /// Zero-heavy blocks must not spend micro-trial budget on LZ alternatives.
    #[test]
    fn zero_heavy_skips_hybrid_lz() {
        let budget =
            PlanningBudget::for_block(CompressionProfile::Dense, PlannerDataClass::ZeroHeavy);
        assert_eq!(budget.hybrid_stage1_candidates, 0);
        assert_eq!(budget.hybrid_stage2_candidates, 0);
    }
}
