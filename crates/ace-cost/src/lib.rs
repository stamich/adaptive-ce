//! Deterministic candidate-size and resource estimators used by Planner V4.
//!
//! * `estimator` — cheap analytical size/CPU/memory estimates ([`CandidateEstimator`]).
//! * `cost_model` — profile-weighted scalar score ([`CostModelV3`]).
//! * `sampling` — deterministic sample windows and adaptive Top-K ([`SamplePolicy`]).
//! * `quality` — quality envelope applied before final scalar-cost selection.

mod cost_model;
mod estimator;
mod sampling;

pub mod quality;

pub use cost_model::*;
pub use estimator::*;
pub use quality::QualityEnvelope;
pub use sampling::*;
