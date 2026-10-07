//! Robust summary statistics for batched timing samples.
//!
//! Definitions (see `docs/BENCHMARK-METHODOLOGY-0.4.6.md`):
//!
//! * **median-of-medians (MoM)** — median of the per-batch medians; the release value;
//! * **batch MAD %** — median absolute deviation of the batch medians from the MoM, relative
//!   to the MoM; the stability gate input;
//! * **outlier** — a sample farther than `3 · 1.4826 · MAD` from the global median
//!   (reported, never removed).

/// Default batch-MAD limit (%) used for the per-case `stability` verdict; release gates apply
/// their own, possibly stricter, limits.
pub(crate) const DEFAULT_STABLE_BATCH_MAD_PERCENT: f64 = 5.0;

/// Scale relating the MAD to the standard deviation of a normal distribution.
pub(crate) const MAD_SIGMA_SCALE: f64 = 1.4826;

/// Number of scaled MADs beyond which a sample is an outlier.
pub(crate) const OUTLIER_SIGMAS: f64 = 3.0;

/// Per-invocation timing samples of one benchmark case and their summaries.
///
/// [`SampleStats::median_ns`] (the median-of-medians) is the headline statistic; classic
/// moments are kept for schema-2.0 compatibility and diagnostics.
#[derive(Debug, Clone)]
pub(crate) struct SampleStats {
    /// All samples in measurement order (batch-major), nanoseconds per invocation.
    pub(crate) samples_ns: Vec<u64>,
    /// Median of each batch, in batch order.
    pub(crate) batch_medians_ns: Vec<f64>,
    /// Number of samples per batch.
    pub(crate) samples_per_batch: usize,
    /// Invocations folded into one sample (adaptive calibration result).
    pub(crate) iterations_per_sample: u64,
    /// Median of the batch medians (release value).
    pub(crate) median_ns: f64,
    /// Median of all samples (diagnostic).
    pub(crate) global_median_ns: f64,
    /// 95th percentile over all samples.
    pub(crate) p95_ns: f64,
    /// 99th percentile over all samples.
    pub(crate) p99_ns: f64,
    /// Arithmetic mean over all samples.
    pub(crate) mean_ns: f64,
    /// Population standard deviation over all samples.
    pub(crate) stddev_ns: f64,
    /// Coefficient of variation (`stddev / mean`) in % (diagnostic only).
    pub(crate) cv_percent: f64,
    /// Median absolute deviation of all samples from [`Self::global_median_ns`].
    pub(crate) mad_ns: f64,
    /// MAD of the batch medians relative to [`Self::median_ns`], in % (gate input).
    pub(crate) batch_mad_percent: f64,
    /// Indices into [`Self::samples_ns`] of reported outliers.
    pub(crate) outlier_indices: Vec<usize>,
    /// Fastest sample.
    pub(crate) min_ns: u64,
    /// Slowest sample.
    pub(crate) max_ns: u64,
}

/// Inherent methods of [`SampleStats`].
impl SampleStats {
    /// Summarizes batches of samples (each inner vector is one batch).
    pub(crate) fn from_batches(batches: Vec<Vec<u64>>, iterations_per_sample: u64) -> Self {
        let samples_per_batch = batches.first().map_or(0, Vec::len);
        let batch_medians_ns: Vec<f64> =
            batches.iter().map(|batch| median(&to_f64(batch))).collect();
        let samples_ns: Vec<u64> = batches.into_iter().flatten().collect();
        let values = to_f64(&samples_ns);

        let median_ns = median(&batch_medians_ns);
        let global_median_ns = median(&values);
        let mad_ns = mad(&values, global_median_ns);
        let (mean_ns, stddev_ns) = mean_and_stddev(&values);

        Self {
            outlier_indices: outliers(&values, global_median_ns, mad_ns),
            batch_mad_percent: relative_percent(mad(&batch_medians_ns, median_ns), median_ns),
            p95_ns: percentile(&values, 0.95),
            p99_ns: percentile(&values, 0.99),
            min_ns: samples_ns.iter().copied().min().unwrap_or(0),
            max_ns: samples_ns.iter().copied().max().unwrap_or(0),
            cv_percent: relative_percent(stddev_ns, mean_ns),
            samples_ns,
            batch_medians_ns,
            samples_per_batch,
            iterations_per_sample,
            median_ns,
            global_median_ns,
            mean_ns,
            stddev_ns,
            mad_ns,
        }
    }

    /// `true` when the batch MAD is within `limit_percent`.
    pub(crate) fn is_stable_within(&self, limit_percent: f64) -> bool {
        self.batch_mad_percent <= limit_percent
    }

    /// `true` when the batch MAD is within [`DEFAULT_STABLE_BATCH_MAD_PERCENT`].
    pub(crate) fn is_stable(&self) -> bool {
        self.is_stable_within(DEFAULT_STABLE_BATCH_MAD_PERCENT)
    }

    /// Throughput in MiB/s for `bytes` processed per invocation at the release median.
    pub(crate) fn mib_per_second(&self, bytes: usize) -> f64 {
        if self.median_ns <= 0.0 {
            0.0
        } else {
            bytes as f64 / 1_048_576.0 / (self.median_ns / 1_000_000_000.0)
        }
    }
}

/// Converts integer samples to `f64`.
fn to_f64(values: &[u64]) -> Vec<f64> {
    values.iter().map(|v| *v as f64).collect()
}

/// Returns a sorted copy of `values`.
fn sorted(values: &[f64]) -> Vec<f64> {
    let mut copy = values.to_vec();
    copy.sort_by(f64::total_cmp);
    copy
}

/// Median (mean of the two middle values for even lengths); `0.0` for an empty slice.
pub(crate) fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let ordered = sorted(values);
    let mid = ordered.len() / 2;
    if ordered.len().is_multiple_of(2) {
        (ordered[mid - 1] + ordered[mid]) / 2.0
    } else {
        ordered[mid]
    }
}

/// Median absolute deviation of `values` around `center`.
pub(crate) fn mad(values: &[f64], center: f64) -> f64 {
    let deviations: Vec<f64> = values.iter().map(|v| (v - center).abs()).collect();
    median(&deviations)
}

/// Nearest-rank (ceiling) percentile, `p` in `0.0..=1.0`; `0.0` for an empty slice.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let ordered = sorted(values);
    let index = (((ordered.len() - 1) as f64) * p).ceil() as usize;
    ordered[index.min(ordered.len() - 1)]
}

/// Arithmetic mean and population standard deviation.
fn mean_and_stddev(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

/// `part / whole × 100`, or `0.0` when `whole` is zero.
fn relative_percent(part: f64, whole: f64) -> f64 {
    if whole == 0.0 {
        0.0
    } else {
        part / whole * 100.0
    }
}

/// Indices of samples farther than `OUTLIER_SIGMAS · MAD_SIGMA_SCALE · mad` from `center`.
/// With `mad == 0` no sample is classified as an outlier.
fn outliers(values: &[f64], center: f64, mad: f64) -> Vec<usize> {
    if mad == 0.0 {
        return Vec::new();
    }
    let limit = OUTLIER_SIGMAS * MAD_SIGMA_SCALE * mad;
    values
        .iter()
        .enumerate()
        .filter(|(_, value)| (*value - center).abs() > limit)
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
