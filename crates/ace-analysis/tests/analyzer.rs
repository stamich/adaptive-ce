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

/// `same_analysis_as` is only true for levels whose profiles really are equal (the DENSE
/// double plan of the engine relies on it to analyse a block once).
#[test]
fn levels_that_claim_equivalence_produce_equal_profiles() {
    let levels = [
        AnalysisLevel::Fast,
        AnalysisLevel::Standard,
        AnalysisLevel::Dense,
    ];
    let inputs: Vec<Vec<u8>> = vec![
        (0..65536).map(|i| ((i * 17) % 251) as u8).collect(),
        vec![0u8; 4096],
        (0..30000).map(|i| (i / 7) as u8).collect(),
        b"the quick brown fox jumps over the lazy dog. ".repeat(900),
        Vec::new(),
    ];
    let analyzer = DefaultBlockAnalyzer;
    for input in &inputs {
        for a in levels {
            for b in levels {
                if a.same_analysis_as(b) {
                    assert_eq!(
                        analyzer.analyze_with_level(input, a),
                        analyzer.analyze_with_level(input, b),
                        "{a:?} vs {b:?} on {} bytes",
                        input.len()
                    );
                }
            }
        }
    }
    assert!(AnalysisLevel::Standard.same_analysis_as(AnalysisLevel::Dense));
    assert!(!AnalysisLevel::Fast.same_analysis_as(AnalysisLevel::Dense));
}
