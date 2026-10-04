use crate::{analyze_repetition, analyze_runs, entropy_h0, sampled_entropy_h1};
use ace_core::BlockProfile;
use ace_simd::count_zeroes;

/// Interface implemented by a component that extracts deterministic compression features from one block.
pub trait BlockAnalyzer {
    /// Analyzes one block without modifying its bytes.
    fn analyze(&self, input: &[u8]) -> BlockProfile;
}

/// Default deterministic ACE 0.3 analyzer with SIMD-assisted primitives where available.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultBlockAnalyzer;

impl BlockAnalyzer for DefaultBlockAnalyzer {
    /// Computes zero-order entropy, sampled H1, run, delta and repetition statistics.
    fn analyze(&self, input: &[u8]) -> BlockProfile {
        if input.is_empty() {
            return BlockProfile {
                size: 0,
                entropy_h0: 0.0,
                entropy_h1: 0.0,
                zero_ratio: 0.0,
                run_score: 0.0,
                delta_score: 0.0,
                repetition_score: 0.0,
                sampled_match_length: 0.0,
                unique_byte_count: 0,
                incompressibility_score: 0.0,
            };
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
        let h1 = sampled_entropy_h1(input, 8);
        let runs = analyze_runs(input);
        let repetition = analyze_repetition(input);
        let run_score = runs.repeated_bytes as f32 / input.len() as f32;
        let delta_score = ((h0 - delta_h).max(0.0) / 8.0).clamp(0.0, 1.0);
        let unique_byte_count = histogram.iter().filter(|&&n| n != 0).count() as u16;
        let entropy_component = (h0 / 8.0).clamp(0.0, 1.0);
        let strongest_structure = run_score
            .max(delta_score)
            .max(repetition.collision_ratio)
            .clamp(0.0, 1.0);
        let incompressibility_score =
            (0.72 * entropy_component + 0.28 * (1.0 - strongest_structure)).clamp(0.0, 1.0);
        BlockProfile {
            size: input.len(),
            entropy_h0: h0,
            entropy_h1: h1,
            zero_ratio: zero_count as f32 / input.len() as f32,
            run_score,
            delta_score,
            repetition_score: repetition.collision_ratio,
            sampled_match_length: repetition.mean_match_length,
            unique_byte_count,
            incompressibility_score,
        }
    }
}
