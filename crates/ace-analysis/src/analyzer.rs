use ace_core::BlockProfile;
use ace_simd::count_zeroes;
use crate::{analyze_repetition, analyze_runs, entropy_h0, sampled_entropy_h1, AnalysisLevel};

/// Interface implemented by a component that extracts deterministic compression features from one block.
pub trait BlockAnalyzer {
    /// Analyzes one block with the standard BALANCED feature budget.
    fn analyze(&self, input: &[u8]) -> BlockProfile;
}

/// Default deterministic ACE 0.3 analyzer with profile-aware work budgets.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultBlockAnalyzer;

/// Inherent methods of [`DefaultBlockAnalyzer`].
impl DefaultBlockAnalyzer {
    /// Analyzes one source block using the requested deterministic analysis budget.
    ///
    /// FAST deliberately omits sampled first-order entropy because that feature is not consumed by
    /// CandidateEstimator, candidate generation or CostModelV3. Avoiding the 65,536-entry transition
    /// table removes substantial per-block work without changing FAST planner decisions.
    pub fn analyze_with_level(&self, input: &[u8], level: AnalysisLevel) -> BlockProfile {
        if input.is_empty() {
            return BlockProfile { size: 0, entropy_h0: 0.0, entropy_h1: 0.0, zero_ratio: 0.0, run_score: 0.0, delta_score: 0.0, repetition_score: 0.0, sampled_match_length: 0.0, unique_byte_count: 0, incompressibility_score: 0.0 };
        }
        let mut histogram = [0u32; 256];
        let mut delta_histogram = [0u32; 256];
        let zero_count = count_zeroes(input);
        let mut previous = input[0];
        for (i, &byte) in input.iter().enumerate() {
            histogram[byte as usize] = histogram[byte as usize].saturating_add(1);
            if i == 0 {
                delta_histogram[byte as usize] = delta_histogram[byte as usize].saturating_add(1);
            } else {
                let delta = byte.wrapping_sub(previous);
                delta_histogram[delta as usize] = delta_histogram[delta as usize].saturating_add(1);
                previous = byte;
            }
        }
        let h0 = entropy_h0(&histogram, input.len());
        let delta_h = entropy_h0(&delta_histogram, input.len());
        let h1 = match level {
            AnalysisLevel::Fast => h0,
            AnalysisLevel::Standard | AnalysisLevel::Dense => sampled_entropy_h1(input, 8),
        };
        let runs = analyze_runs(input);
        let repetition = analyze_repetition(input);
        let run_score = runs.repeated_bytes as f32 / input.len() as f32;
        let delta_score = ((h0 - delta_h).max(0.0) / 8.0).clamp(0.0, 1.0);
        let unique_byte_count = histogram.iter().filter(|&&n| n != 0).count() as u16;
        let entropy_component = (h0 / 8.0).clamp(0.0, 1.0);
        let strongest_structure = run_score.max(delta_score).max(repetition.collision_ratio).clamp(0.0, 1.0);
        let incompressibility_score = (0.72 * entropy_component + 0.28 * (1.0 - strongest_structure)).clamp(0.0, 1.0);
        BlockProfile {
            size: input.len(), entropy_h0: h0, entropy_h1: h1,
            zero_ratio: zero_count as f32 / input.len() as f32,
            run_score, delta_score, repetition_score: repetition.collision_ratio,
            sampled_match_length: repetition.mean_match_length, unique_byte_count,
            incompressibility_score,
        }
    }
}

/// Implements [`BlockAnalyzer`] for [`DefaultBlockAnalyzer`].
impl BlockAnalyzer for DefaultBlockAnalyzer {
    /// Computes the standard BALANCED feature set for backwards-compatible callers.
    fn analyze(&self, input: &[u8]) -> BlockProfile {
        self.analyze_with_level(input, AnalysisLevel::Standard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAST and STANDARD must preserve every planner-consumed feature.
    #[test]
    fn fast_level_preserves_planner_features() {
        let data = (0..65536).map(|i| ((i * 17) % 251) as u8).collect::<Vec<_>>();
        let analyzer = DefaultBlockAnalyzer;
        let fast = analyzer.analyze_with_level(&data, AnalysisLevel::Fast);
        let standard = analyzer.analyze_with_level(&data, AnalysisLevel::Standard);
        assert_eq!(fast.size, standard.size);
        assert_eq!(fast.entropy_h0, standard.entropy_h0);
        assert_eq!(fast.zero_ratio, standard.zero_ratio);
        assert_eq!(fast.run_score, standard.run_score);
        assert_eq!(fast.delta_score, standard.delta_score);
        assert_eq!(fast.repetition_score, standard.repetition_score);
        assert_eq!(fast.sampled_match_length, standard.sampled_match_length);
        assert_eq!(fast.unique_byte_count, standard.unique_byte_count);
        assert_eq!(fast.incompressibility_score, standard.incompressibility_score);
    }
}
