use ace_core::{BlockProfile, PhysicalCompressionPlan};
use ace_planner::PlannerTelemetry;

/// Planner explanation for one input block.
#[derive(Debug, Clone)]
pub struct BlockExplanation {
    /// Zero-based block identifier.
    pub block_id: u64,
    /// Statistical profile derived by the analyzer.
    pub profile: BlockProfile,
    /// Candidate plans considered by the deterministic planner.
    pub candidates: Vec<PhysicalCompressionPlan>,
    /// Candidate selected by deterministic cost evaluation.
    pub selected: PhysicalCompressionPlan,
    /// ACE 0.3 hot-path planning work performed for this block.
    pub telemetry: PlannerTelemetry,
}
