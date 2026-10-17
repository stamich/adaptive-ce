//! Tests of `ace_analysis` `analyzer` (formerly the inline unit tests of `src`).

use ace_analysis::*;

/// FAST and STANDARD must preserve every planner-consumed feature.
#[test]
fn fast_level_preserves_planner_features() {
    let data = (0..65536)
        .map(|i| ((i * 17) % 251) as u8)
        .collect::<Vec<_>>();
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
    assert_eq!(
        fast.incompressibility_score,
        standard.incompressibility_score
    );
}
