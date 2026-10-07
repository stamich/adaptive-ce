use crate::NumericProfile;
use ace_core::{AccessHint, CompressionProfile};

/// Explanation category returned by the ACE 0.4 file-level block-size advisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSizeReason {
    /// Random-access hint caps blocks at 256 KiB.
    RandomAccessCap,
    /// Strong numeric structure benefits from a larger sequential block.
    NumericSequential,
    /// Sequential workloads favor larger blocks for throughput.
    SequentialThroughput,
    /// Balanced fallback preserves the hardened ACE 0.3 default.
    BalancedDefault,
}

/// Deterministic file-level block-size recommendation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockSizeRecommendation {
    /// Recommended block size in bytes.
    pub block_size: usize,
    /// Confidence in the recommendation in `0.0..=1.0`.
    pub confidence: f32,
    /// Human-reviewable reason category.
    pub reason: BlockSizeReason,
}

/// Chooses one block size for a complete file from access/profile hints and an initial numeric profile.
pub fn recommend_block_size(
    access: AccessHint,
    profile: CompressionProfile,
    numeric: &NumericProfile,
) -> BlockSizeRecommendation {
    if matches!(access, AccessHint::RandomAccess) {
        return BlockSizeRecommendation {
            block_size: 256 * 1024,
            confidence: 0.95,
            reason: BlockSizeReason::RandomAccessCap,
        };
    }
    if numeric.detected && numeric.confidence >= 0.85 {
        return BlockSizeRecommendation {
            block_size: 1024 * 1024,
            confidence: numeric.confidence,
            reason: BlockSizeReason::NumericSequential,
        };
    }
    if matches!(access, AccessHint::Sequential) || matches!(profile, CompressionProfile::Fast) {
        return BlockSizeRecommendation {
            block_size: 512 * 1024,
            confidence: 0.80,
            reason: BlockSizeReason::SequentialThroughput,
        };
    }
    BlockSizeRecommendation {
        block_size: 256 * 1024,
        confidence: 0.90,
        reason: BlockSizeReason::BalancedDefault,
    }
}
