//! JSON representation of [`SampleStats`].

use serde_json::Value;

use super::{MeasurementPlan, SampleStats};
use crate::json::JsonObjectBuilder;

/// Identifier of the measurement methodology written into every timing object.
pub(crate) const HARNESS_VERSION: &str = "v3";

/// Converts statistics into the common ACE timing object.
///
/// Schema-2.0 fields (`runs`, `median_ns`, `median_mb_s`, …) keep their names and are
/// computed from the median-of-medians, so 2.0 consumers keep working. Schema 2.1 adds the
/// `stable_timing` object next to them.
pub(crate) fn timing_json(stats: &SampleStats, bytes: usize) -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("runs", stats.samples_ns.len())
        .field("warmup_iterations", MeasurementPlan::current().warmups)
        .field("median_ns", stats.median_ns)
        .field("p95_ns", stats.p95_ns)
        .field("p99_ns", stats.p99_ns)
        .field("mean_ns", stats.mean_ns)
        .field("stddev_ns", stats.stddev_ns)
        .field("cv_percent", stats.cv_percent)
        .field("unstable_measurement", !stats.is_stable())
        .field("min_ns", stats.min_ns)
        .field("max_ns", stats.max_ns)
        .field("median_mb_s", stats.mib_per_second(bytes))
        .field("samples_ns", &stats.samples_ns)
        .value("stable_timing", stable_timing_json(stats));
    row.build()
}

/// The schema-2.1 `stable_timing` object.
fn stable_timing_json(stats: &SampleStats) -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("harness", HARNESS_VERSION)
        .field("batches", stats.batch_medians_ns.len())
        .field("samples_per_batch", stats.samples_per_batch)
        .field("iterations_per_sample", stats.iterations_per_sample)
        .field("batch_medians_ns", &stats.batch_medians_ns)
        .field("median_of_medians_ns", stats.median_ns)
        .field("global_median_ns", stats.global_median_ns)
        .field("mad_ns", stats.mad_ns)
        .field("batch_mad_percent", stats.batch_mad_percent)
        .field("outlier_indices", &stats.outlier_indices)
        .field(
            "stability",
            if stats.is_stable() {
                "stable"
            } else {
                "unstable"
            },
        );
    row.build()
}
