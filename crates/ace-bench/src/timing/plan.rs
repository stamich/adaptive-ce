//! The measurement plan shared by every benchmark case of one harness run.

use std::sync::OnceLock;

/// Environment variable enabling the quick (smoke) plan: one short batch per case.
pub(crate) const QUICK_ENV: &str = "ACE_BENCH_QUICK";

/// Environment variable overriding the minimum duration of one sample, in milliseconds.
pub(crate) const MIN_SAMPLE_MS_ENV: &str = "ACE_BENCH_MIN_SAMPLE_MS";

/// Upper bound on operation invocations folded into one sample (protects nanosecond ops).
pub(crate) const MAX_ITERATIONS_PER_SAMPLE: u64 = 1 << 20;

/// How many batches, samples and warm-ups each benchmark case uses.
///
/// A *sample* is the mean duration of `iterations` back-to-back invocations, where
/// `iterations` is calibrated (by doubling) so one sample lasts at least `min_sample_ns`. Samples are
/// grouped in batches; the reported median is the median of the per-batch medians.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeasurementPlan {
    /// Number of independent batches.
    pub(crate) batches: usize,
    /// Samples collected inside each batch.
    pub(crate) samples_per_batch: usize,
    /// Unmeasured invocations executed before calibration.
    pub(crate) warmups: usize,
    /// Minimum wall-clock duration of one sample in nanoseconds.
    pub(crate) min_sample_ns: u64,
    /// `true` for the quick smoke plan (results are not release-gate grade).
    pub(crate) quick: bool,
}

/// Inherent methods of [`MeasurementPlan`].
impl MeasurementPlan {
    /// Release-grade plan: 3 batches × 7 samples, 5 warm-ups, ≥ 50 ms per sample.
    pub(crate) const RELEASE: Self = Self {
        batches: 3,
        samples_per_batch: 7,
        warmups: 5,
        min_sample_ns: 50_000_000,
        quick: false,
    };

    /// Smoke plan used by CI and the demo: 1 batch × 5 samples, 1 warm-up, ≥ 2 ms per sample.
    pub(crate) const QUICK: Self = Self {
        batches: 1,
        samples_per_batch: 5,
        warmups: 1,
        min_sample_ns: 2_000_000,
        quick: true,
    };

    /// Total number of samples produced for one case.
    pub(crate) fn total_samples(&self) -> usize {
        self.batches * self.samples_per_batch
    }

    /// Builds the plan from `ACE_BENCH_QUICK` and `ACE_BENCH_MIN_SAMPLE_MS`.
    fn from_env() -> Self {
        let quick = std::env::var(QUICK_ENV)
            .map(|value| !value.is_empty() && value != "0")
            .unwrap_or(false);
        let mut plan = if quick { Self::QUICK } else { Self::RELEASE };
        if let Some(ms) = std::env::var(MIN_SAMPLE_MS_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
        {
            plan.min_sample_ns = ms.saturating_mul(1_000_000);
        }
        plan
    }

    /// The process-wide plan, resolved once from the environment.
    pub(crate) fn current() -> Self {
        static PLAN: OnceLock<MeasurementPlan> = OnceLock::new();
        *PLAN.get_or_init(Self::from_env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
