//! The adaptive, batched measurement loop.

use std::hint::black_box;
use std::time::Instant;

use super::{MeasurementPlan, SampleStats, MAX_ITERATIONS_PER_SAMPLE};

/// Boxed error type returned by benchmark operations.
pub(crate) type BenchError = Box<dyn std::error::Error>;

/// Measures `operation` with the process-wide [`MeasurementPlan`] and returns the statistics
/// together with the result of the final invocation (used for correctness checks).
pub(crate) fn measure<F, T>(operation: F) -> Result<(SampleStats, T), BenchError>
where
    F: FnMut() -> Result<T, BenchError>,
{
    measure_with(MeasurementPlan::current(), operation)
}

/// Measures `operation` with an explicit plan.
///
/// 1. `plan.warmups` unmeasured invocations;
/// 2. calibration: starting from one invocation, `iterations` doubles until a timed group
///    lasts at least `plan.min_sample_ns` (capped at `MAX_ITERATIONS_PER_SAMPLE`);
/// 3. `plan.batches × plan.samples_per_batch` samples, each the mean per-invocation time of
///    `iterations` consecutive calls.
pub(crate) fn measure_with<F, T>(
    plan: MeasurementPlan,
    mut operation: F,
) -> Result<(SampleStats, T), BenchError>
where
    F: FnMut() -> Result<T, BenchError>,
{
    for _ in 0..plan.warmups {
        black_box(operation()?);
    }
    let (iterations, mut last) = calibrate(&mut operation, plan.min_sample_ns)?;

    let mut batches = Vec::with_capacity(plan.batches);
    for _ in 0..plan.batches {
        let mut samples = Vec::with_capacity(plan.samples_per_batch);
        for _ in 0..plan.samples_per_batch {
            let (elapsed, value) = time_group(&mut operation, iterations)?;
            last = value;
            samples.push(elapsed / iterations);
        }
        batches.push(samples);
    }
    Ok((SampleStats::from_batches(batches, iterations), last))
}

/// Runs `iterations` (≥ 1) consecutive invocations and returns their total duration and
/// the last result; intermediate results pass through [`black_box`].
fn time_group<F, T>(operation: &mut F, iterations: u64) -> Result<(u64, T), BenchError>
where
    F: FnMut() -> Result<T, BenchError>,
{
    let start = Instant::now();
    for _ in 1..iterations {
        black_box(operation()?);
    }
    let value = operation()?;
    Ok((elapsed_ns(start), value))
}

/// Doubles the group size until one group lasts at least `min_sample_ns` and returns the
/// calibrated iteration count with the last result.
fn calibrate<F, T>(operation: &mut F, min_sample_ns: u64) -> Result<(u64, T), BenchError>
where
    F: FnMut() -> Result<T, BenchError>,
{
    let mut iterations = 1u64;
    loop {
        let (elapsed, value) = time_group(operation, iterations)?;
        let next = next_iterations(iterations, elapsed, min_sample_ns);
        if next == iterations {
            return Ok((iterations, value));
        }
        iterations = next;
    }
}

/// Calibration step: keeps `iterations` once a group of that size lasted `min_sample_ns`
/// (or the cap is reached), otherwise doubles it.
pub(crate) fn next_iterations(iterations: u64, elapsed_ns: u64, min_sample_ns: u64) -> u64 {
    if elapsed_ns >= min_sample_ns || iterations >= MAX_ITERATIONS_PER_SAMPLE {
        iterations
    } else {
        iterations.saturating_mul(2).min(MAX_ITERATIONS_PER_SAMPLE)
    }
}

/// Nanoseconds elapsed since `start`, saturated to `u64`.
fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
