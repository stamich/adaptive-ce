//! Tests of `ace_bench` `timing_stats` (formerly the inline unit tests of `src`).

use ace_bench::timing::SampleStats;

#[test]
fn median_of_medians_ignores_one_bad_batch() {
    let stats = SampleStats::from_batches(
        vec![vec![100, 101, 99], vec![100, 100, 100], vec![500, 510, 490]],
        1,
    );
    assert_eq!(stats.batch_medians_ns, vec![100.0, 100.0, 500.0]);
    assert_eq!(stats.median_ns, 100.0);
    assert_eq!(stats.batch_mad_percent, 0.0, "two of three batches agree");
}

#[test]
fn drifting_batches_are_unstable() {
    let stats = SampleStats::from_batches(vec![vec![100; 5], vec![110; 5], vec![120; 5]], 1);
    assert_eq!(stats.median_ns, 110.0);
    assert!((stats.batch_mad_percent - 10.0 / 110.0 * 100.0).abs() < 1e-9);
    assert!(!stats.is_stable());
}

#[test]
fn single_outlier_is_reported_not_removed() {
    let stats = SampleStats::from_batches(vec![vec![100, 101, 99, 100, 102, 98, 1000]], 1);
    assert_eq!(stats.outlier_indices, vec![6]);
    assert_eq!(stats.samples_ns.len(), 7);
    assert_eq!(stats.median_ns, 100.0);
    assert_eq!(stats.max_ns, 1000);
}

#[test]
fn constant_samples_are_stable_and_outlier_free() {
    let stats = SampleStats::from_batches(vec![vec![7; 7], vec![7; 7]], 3);
    assert_eq!(stats.mad_ns, 0.0);
    assert!(stats.outlier_indices.is_empty());
    assert!(stats.is_stable());
    assert_eq!(stats.iterations_per_sample, 3);
}

#[test]
fn throughput_uses_release_median() {
    let stats = SampleStats::from_batches(vec![vec![1_000_000_000]], 1);
    assert!((stats.mib_per_second(1_048_576) - 1.0).abs() < 1e-9);
}
