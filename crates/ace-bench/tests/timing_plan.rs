//! Tests of `ace_bench` `timing_plan` (formerly the inline unit tests of `src`).

use ace_bench::timing::MeasurementPlan;

#[test]
fn release_plan_matches_methodology() {
    assert_eq!(MeasurementPlan::RELEASE.total_samples(), 21);
    assert_eq!(MeasurementPlan::RELEASE.min_sample_ns, 50_000_000);
    assert_eq!(MeasurementPlan::RELEASE.warmups, 5);
}

#[test]
fn quick_plan_is_marked() {
    assert_ne!(MeasurementPlan::QUICK, MeasurementPlan::RELEASE);
    assert_eq!(MeasurementPlan::QUICK.total_samples(), 5);
}
