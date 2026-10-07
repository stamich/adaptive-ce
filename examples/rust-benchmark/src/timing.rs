//! Warm-up/measurement loop and timing statistics.

use crate::prelude::*;

/// Number of measured repetitions per benchmark case.
pub(crate) const RUNS: usize = 7;

/// Number of unmeasured warm-up repetitions per benchmark case.
pub(crate) const WARMUPS: usize = 3;

/// Repeated timing samples and standard robust summary statistics.
#[derive(Debug, Clone)]
pub(crate) struct SampleStats {
    pub(crate) samples_ns: Vec<u64>,
    pub(crate) median_ns: f64,
    pub(crate) p95_ns: f64,
    pub(crate) p99_ns: f64,
    pub(crate) mean_ns: f64,
    pub(crate) stddev_ns: f64,
    pub(crate) cv_percent: f64,
    pub(crate) min_ns: u64,
    pub(crate) max_ns: u64,
}

/// Runs warmups and seven measured invocations while retaining the final operation result.
pub(crate) fn measure<F, T>(mut operation: F) -> Result<(SampleStats, T), Box<dyn std::error::Error>>
where F: FnMut() -> Result<T, Box<dyn std::error::Error>> {
    for _ in 0..WARMUPS { black_box(operation()?); }
    let mut samples = Vec::with_capacity(RUNS);
    let mut last = None;
    for _ in 0..RUNS {
        let start = Instant::now();
        let value = operation()?;
        samples.push(start.elapsed().as_nanos().min(u64::MAX as u128) as u64);
        last = Some(value);
    }
    Ok((summarize(samples), last.expect("RUNS is non-zero")))
}

/// Calculates deterministic percentile summaries from nanosecond samples.
pub(crate) fn summarize(mut samples: Vec<u64>) -> SampleStats {
    let raw = samples.clone();
    samples.sort_unstable();
    let percentile = |p: f64| -> f64 {
        let idx = (((samples.len() - 1) as f64) * p).ceil() as usize;
        samples[idx.min(samples.len() - 1)] as f64
    };
    let mean = raw.iter().copied().map(|v| v as f64).sum::<f64>() / raw.len() as f64;
    let variance = if raw.is_empty() {
        0.0
    } else {
        raw.iter()
            .map(|value| {
                let delta = *value as f64 - mean;
                delta * delta
            })
            .sum::<f64>()
            / raw.len() as f64
    };
    let stddev = variance.sqrt();
    let cv_percent = if mean == 0.0 { 0.0 } else { stddev / mean * 100.0 };
    SampleStats {
        samples_ns: raw,
        median_ns: percentile(0.50),
        p95_ns: percentile(0.95),
        p99_ns: percentile(0.99),
        mean_ns: mean,
        stddev_ns: stddev,
        cv_percent,
        min_ns: *samples.first().unwrap_or(&0),
        max_ns: *samples.last().unwrap_or(&0),
    }
}

/// Converts samples to the common ACE timing and throughput representation.
pub(crate) fn timing_json(stats: &SampleStats, bytes: usize) -> Value {
    let seconds = stats.median_ns / 1_000_000_000.0;
    let mb_s = if seconds == 0.0 { 0.0 } else { bytes as f64 / 1_048_576.0 / seconds };
    let mut row = JsonObjectBuilder::new();
    row.field("runs", RUNS)
        .field("warmup_iterations", WARMUPS)
        .field("median_ns", stats.median_ns)
        .field("p95_ns", stats.p95_ns)
        .field("p99_ns", stats.p99_ns)
        .field("mean_ns", stats.mean_ns)
        .field("stddev_ns", stats.stddev_ns)
        .field("cv_percent", stats.cv_percent)
        .field("unstable_measurement", stats.cv_percent > 10.0)
        .field("min_ns", stats.min_ns)
        .field("max_ns", stats.max_ns)
        .field("median_mb_s", mb_s)
        .field("samples_ns", &stats.samples_ns);
    row.build()
}
