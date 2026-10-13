//! Deterministic block-statistics collection used by ACE Planner V4 / V5.
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod analyzer;
mod block_size;
mod entropy;
mod float;
mod level;
mod numeric;
mod numeric_prefilter;
mod repetition;
mod run_profile;
mod runs;

pub use analyzer::*;
pub use block_size::*;
pub use entropy::*;
pub use float::*;
pub use level::*;
pub use numeric::*;
pub use numeric_prefilter::*;
pub use repetition::*;
pub use run_profile::*;
pub use runs::*;
