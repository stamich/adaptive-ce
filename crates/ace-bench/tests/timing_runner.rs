//! Tests of `ace_bench` `timing_runner` (formerly the inline unit tests of `src`).

use ace_bench::timing::{
    measure_with, next_iterations, MeasurementPlan, MAX_ITERATIONS_PER_SAMPLE,
};

#[test]
fn calibration_doubles_until_long_enough_and_is_capped() {
    assert_eq!(next_iterations(1, 100_000_000, 50_000_000), 1);
    assert_eq!(next_iterations(4, 1_000_000, 50_000_000), 8);
    assert_eq!(
        next_iterations(MAX_ITERATIONS_PER_SAMPLE, 0, 50_000_000),
        MAX_ITERATIONS_PER_SAMPLE
    );
}

#[test]
fn measure_with_returns_last_value_and_full_sample_count() {
    let plan = MeasurementPlan {
        batches: 2,
        samples_per_batch: 3,
        warmups: 1,
        min_sample_ns: 0,
        quick: true,
    };
    let mut calls = 0u32;
    let (stats, last) = measure_with(plan, || {
        calls += 1;
        Ok(calls)
    })
    .expect("infallible operation");
    assert_eq!(stats.samples_ns.len(), 6);
    assert_eq!(stats.batch_medians_ns.len(), 2);
    assert_eq!(last, calls);
}
