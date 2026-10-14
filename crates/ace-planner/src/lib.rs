//! Deterministic candidate generation, evaluation and cost modeling for ACE Planner V4 / V5.
//!
//! Planner V5 (ACE 0.5) is Planner V4.3 plus the Float lane (`float_route`).
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod budget;
mod confidence;
mod cost;
mod decision;
mod dominance_policy;
mod evaluator;
mod exhaustive;
mod fastpath;
mod float_route;
mod hybrid;
mod pipeline;
mod plan_identity;
mod planner;
mod planning_context;
mod policy;
mod policy_oracle;
mod route;
mod route_policy;

pub use budget::*;
pub use confidence::*;
pub use cost::*;
pub use decision::*;
pub use dominance_policy::*;
pub use evaluator::*;
pub use exhaustive::*;
pub use fastpath::*;
pub use float_route::*;
pub use hybrid::*;
pub use pipeline::*;
pub use plan_identity::*;
pub use planner::*;
pub use planning_context::*;
pub use policy::*;
pub use policy_oracle::*;
pub use route::*;
pub use route_policy::*;
