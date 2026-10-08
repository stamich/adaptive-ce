//! Planner explanation types returned by `AceEngine::explain`.

use ace_analysis::NumericProfile;
use ace_core::{BlockProfile, PhysicalCompressionPlan};
use ace_cost::TimeSeriesEstimate;
use ace_planner::{PlannerTelemetry, RouteDecision};

/// Planner explanation for one input block.
#[derive(Debug, Clone)]
pub struct BlockExplanation {
    /// Zero-based block identifier.
    pub block_id: u64,
    /// Statistical profile derived by the analyzer.
    pub profile: BlockProfile,
    /// ACE 0.4 schema-free integer-structure profile for this block.
    pub numeric_profile: NumericProfile,
    /// Candidate plans considered by the deterministic planner.
    pub candidates: Vec<PhysicalCompressionPlan>,
    /// Candidate selected by deterministic cost evaluation.
    pub selected: PhysicalCompressionPlan,
    /// ACE 0.4 hot-path planning work performed for this block.
    pub telemetry: PlannerTelemetry,
    /// Planner V5 route decision (route, base route, Float evidence, run prefilter).
    pub route: RouteDecision,
    /// TS1 estimate of the selected plan when the selected codec is TS1.
    pub time_series: Option<TimeSeriesEstimate>,
}
