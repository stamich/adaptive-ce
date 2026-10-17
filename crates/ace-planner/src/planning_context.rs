use std::time::{Duration, Instant};

use ace_core::AceConfig;

use crate::{RouteDecision, RoutePolicy};

/// Per-block Planner V4.3 context carrying the one authoritative route decision.
///
/// The context is created exactly once by the production engine before generic analysis. It
/// prevents repeated Numeric prefilter/full-block validation work in engine and evaluator layers.
#[derive(Debug, Clone, Copy)]
pub struct PlanningContext {
    /// Authoritative route decision reused by all later planning stages.
    pub route: RouteDecision,
    /// Time spent in route prefilter/classification/full fixed-step validation.
    pub route_classify_time: Duration,
}

/// Inherent methods of [`PlanningContext`].
impl PlanningContext {
    /// Classifies one block once and records the route-classification latency.
    pub fn classify(input: &[u8], config: &AceConfig) -> Self {
        let started = Instant::now();
        let route = RoutePolicy::classify(input, config);
        Self {
            route,
            route_classify_time: started.elapsed(),
        }
    }

    /// Returns whether the already-computed route is the strict NumericFast route.
    pub fn is_numeric_fast(&self) -> bool {
        matches!(self.route.route, crate::PlannerRoute::NumericFast)
    }
}
