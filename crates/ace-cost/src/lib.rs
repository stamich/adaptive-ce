//! Deterministic candidate-size and resource estimators used by Planner V4 / V5.
//!
//! * `estimator` — cheap analytical size/CPU/memory estimates ([`CandidateEstimator`]).
//! * `cost_model` — profile-weighted scalar score ([`CostModelV3`]).
//! * `sampling` — deterministic sample windows and adaptive Top-K ([`SamplePolicy`]).
//! * `quality` — quality envelope applied before final scalar-cost selection.
//! * `time_series` — TS1 Gorilla (sampled) and RunDelta (exact) estimators (ACE 0.5.0).
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod cost_model;
mod estimator;
mod sampling;
mod time_series;

pub mod quality;

pub use cost_model::*;
pub use estimator::*;
pub use quality::QualityEnvelope;
pub use sampling::*;
pub use time_series::*;
