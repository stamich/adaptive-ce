//! Benchmark Harness V3: measurement plan, adaptive measurement loop, robust statistics and
//! their schema-2.1 JSON representation.
//!
//! | module   | responsibility                                                         |
//! |----------|------------------------------------------------------------------------|
//! | `plan`   | [`MeasurementPlan`]: batches, samples, warm-ups, minimum sample time    |
//! | `runner` | [`measure`]: calibration + interleaving-free batched measurement loop   |
//! | `stats`  | [`SampleStats`]: median-of-medians, MAD, outliers, stability verdict    |
//! | `report` | [`timing_json`]: schema 2.0 compatible fields + schema 2.1 extensions   |

mod plan;
mod report;
mod runner;
mod stats;

pub(crate) use plan::*;
pub(crate) use report::*;
pub(crate) use runner::*;
pub(crate) use stats::*;
