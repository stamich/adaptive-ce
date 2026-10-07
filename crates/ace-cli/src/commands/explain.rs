//! `explain`: analyzer features, every candidate's cost and the selected plan per block.

use ace_core::{AceConfig, CompressionProfile};
use ace_engine::AceEngine;
use anyhow::Result;

use crate::files;

/// Runs analyzer and planner only, printing deterministic candidate scores per block.
pub fn explain(input: &str, profile: CompressionProfile) -> Result<()> {
    let data = files::read(input)?;
    let engine = AceEngine::new(AceConfig {
        profile,
        ..AceConfig::default()
    })?;
    for explanation in engine.explain(&data)? {
        println!(
            "block {} size={} H0={:.3} H1={:.3} run={:.3} delta={:.3} repeat={:.3}",
            explanation.block_id,
            explanation.profile.size,
            explanation.profile.entropy_h0,
            explanation.profile.entropy_h1,
            explanation.profile.run_score,
            explanation.profile.delta_score,
            explanation.profile.repetition_score
        );
        println!(
            "  numeric detected={} width={:?} confidence={:.3} monotonic={:.3} delta_p95_bits={} dod_zero={:.3} dod_p95_bits={} tail={}",
            explanation.numeric_profile.detected,
            explanation.numeric_profile.width,
            explanation.numeric_profile.confidence,
            explanation.numeric_profile.monotonic_ratio,
            explanation.numeric_profile.delta_bit_width_p95,
            explanation.numeric_profile.dod_zero_ratio,
            explanation.numeric_profile.dod_bit_width_p95,
            explanation.numeric_profile.tail_bytes,
        );
        for candidate in &explanation.candidates {
            let score = display_score(candidate.score);
            println!("  candidate tier={:?} {:?}/{:?} transforms={:?} score={} predicted={} metadata={} reason={}", candidate.tier, candidate.decoding.codec, candidate.decoding.entropy, candidate.decoding.transforms, score, candidate.cost.predicted_size_bytes, candidate.cost.metadata_bytes, candidate.reason);
        }
        println!(
            "  selected {:?}/{:?} transforms={:?} fast_path={} estimated={} sampled={} stage2={} hybrid_lz={} hybrid_stage1={} hybrid_stage2={} hybrid_skipped={} hybrid_high_conf_skips={} hybrid_bytes={} hybrid_disagreement_ppm={} quality={} best_blended={} quality_limit={} selected_blended={} size_rank={} cost_rank={} full_trials={}\n",
            explanation.selected.decoding.codec,
            explanation.selected.decoding.entropy,
            explanation.selected.decoding.transforms,
            explanation.telemetry.fast_path_hit,
            explanation.telemetry.estimated_candidates,
            explanation.telemetry.sampled_candidates,
            explanation.telemetry.second_stage_candidates,
            explanation.telemetry.hybrid_lz_candidates,
            explanation.telemetry.hybrid_lz_stage1_candidates,
            explanation.telemetry.hybrid_lz_stage2_candidates,
            explanation.telemetry.hybrid_lz_skipped_candidates,
            explanation.telemetry.hybrid_lz_high_confidence_skips,
            explanation.telemetry.hybrid_lz_sample_bytes,
            explanation.telemetry.hybrid_lz_max_disagreement_ppm,
            explanation.telemetry.quality_qualified_candidates,
            explanation.telemetry.best_blended_size_bytes,
            explanation.telemetry.quality_limit_bytes,
            explanation.telemetry.selected_blended_size_bytes,
            explanation.telemetry.selected_size_rank,
            explanation.telemetry.selected_cost_rank,
            explanation.telemetry.full_trial_encodes,
        );
    }
    Ok(())
}

/// Formats a planner score for human-facing diagnostics.
///
/// Entropy-selection policy uses a very large saturating penalty internally.  Presenting that
/// sentinel as a decimal number makes `ace explain` look like an arithmetic overflow, so the CLI
/// renders such values as `penalized` while preserving the exact numeric score inside the planner.
fn display_score(score: u128) -> String {
    if score >= u128::MAX / 8 {
        "penalized".to_string()
    } else {
        score.to_string()
    }
}
