//! Fixed tie-breaks (ACE 0.5.1 determinism): when candidates or estimates are equal, the
//! choice follows a documented key and never the order in which candidates were produced or
//! evaluated.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_codecs::{estimate_numeric, NumericMode};
use ace_core::{AceConfig, CompressionProfile, NumericWidth, PhysicalCompressionPlan};
use ace_planner::{
    evaluate_candidates, stable_plan_key, CompressionPlanner, DefaultCompressionPlanner,
};

/// Inputs on which many candidates tie (all-zero, constant) plus ordinary data.
fn inputs() -> Vec<(&'static str, Vec<u8>)> {
    let text: Vec<u8> = b"tie-break fixture, tie-break fixture; "
        .iter()
        .copied()
        .cycle()
        .take(64 * 1024)
        .collect();
    let counter: Vec<u8> = (0u32..16_384).flat_map(|i| i.to_le_bytes()).collect();
    vec![
        ("zeros", vec![0u8; 64 * 1024]),
        ("constant", vec![0xA5u8; 64 * 1024]),
        ("text", text),
        ("counter", counter),
    ]
}

/// Equal NUM1 sizes are broken by width rank (u32, u64, u16), then by mode id: an all-zero
/// block (every width and mode encodes it at bit width 0) selects u32 + Frame-of-Reference.
#[test]
fn numeric_estimate_ties_follow_width_rank_then_mode() {
    for len in [4096usize, 4096 + 8, 65_536] {
        let estimate = estimate_numeric(&vec![0u8; len]).unwrap();
        assert_eq!(estimate.width, NumericWidth::U32, "len {len}");
        assert_eq!(estimate.mode, NumericMode::FrameOfReference, "len {len}");
        assert_eq!(estimate.bit_width, 0, "len {len}");
    }
}

/// The exhaustive evaluator selects the same plan for every permutation of its candidates:
/// equal scores are broken by `stable_plan_key`, not by position.
#[test]
fn candidate_order_never_changes_the_selected_plan() {
    let analyzer = DefaultBlockAnalyzer;
    for (name, input) in inputs() {
        let profile = analyzer.analyze(&input);
        for compression in [
            CompressionProfile::Fast,
            CompressionProfile::Balanced,
            CompressionProfile::Dense,
        ] {
            let config = AceConfig {
                profile: compression,
                ..AceConfig::default()
            };
            let candidates = DefaultCompressionPlanner.candidates(&profile, &config);
            let selected = |order: &[PhysicalCompressionPlan]| {
                let plan = evaluate_candidates(&input, order, &config).unwrap();
                (stable_plan_key(&plan), plan.score)
            };
            let reference = selected(&candidates);
            let mut reversed = candidates.clone();
            reversed.reverse();
            assert_eq!(
                selected(&reversed),
                reference,
                "{name} {compression:?} reversed"
            );
            for shift in 1..candidates.len() {
                let mut rotated = candidates.clone();
                rotated.rotate_left(shift);
                assert_eq!(
                    selected(&rotated),
                    reference,
                    "{name} {compression:?} rotated {shift}"
                );
            }
        }
    }
}
