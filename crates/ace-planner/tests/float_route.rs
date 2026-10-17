//! Tests of `ace_planner` `float_route` (formerly the inline unit tests of `src`).

use ace_codecs::TimeSeriesLayout;
use ace_core::{AceConfig, CodecId};
use ace_cost::TimeSeriesEstimate;
use ace_planner::*;

/// Little-endian f64 bytes of `values`.
fn f64_bytes(values: impl IntoIterator<Item = f64>) -> Vec<u8> {
    values.into_iter().flat_map(f64::to_le_bytes).collect()
}

/// Smooth f64 values (small random increments, as in Corpus V4).
fn smooth(count: usize) -> Vec<u8> {
    let mut value = 20.0f64;
    let mut state = 0x1234_5678u32;
    f64_bytes((0..count).map(|_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        value += f64::from(state % 17) * 0.001 - 0.008;
        value
    }))
}

/// The dominance rule needs both the relative and the absolute win.
#[test]
fn dominance_rule() {
    let exact = |bytes| TimeSeriesEstimate {
        layout: TimeSeriesLayout::run_delta(4),
        estimated_bytes: bytes,
        exact: true,
        confidence: 1.0,
    };
    let sampled = |bytes| TimeSeriesEstimate {
        layout: TimeSeriesLayout::GORILLA_F64,
        exact: false,
        ..exact(bytes)
    };
    assert!(ts1_beats_generic(&exact(100), 100_000, 262_144));
    assert!(!ts1_beats_generic(&exact(95_000), 100_000, 262_144)); // only 5 % better
    assert!(!ts1_beats_generic(&exact(100), 8_000, 262_144)); // saves < input / 32
    assert!(ts1_beats_generic(&exact(0), 8_192, 262_144)); // saves exactly input / 32
    assert!(ts1_beats_generic(&exact(85_000), 100_000, 262_144));
    assert!(!ts1_beats_generic(&sampled(85_000), 100_000, 262_144)); // sampled: ≤ 75 %
    assert!(ts1_beats_generic(&sampled(75_000), 100_000, 262_144));
}

/// A constant f64 block is FloatFast with RunDelta and the payload is kept.
#[test]
fn constant_f64_is_float_fast_run_delta() {
    let input = f64_bytes(std::iter::repeat_n(21.5, 32_768));
    let route = RoutePolicy::classify(&input, &AceConfig::default());
    assert_eq!(route.route, PlannerRoute::FloatFast);
    assert_eq!(route.reason, RouteReason::DominantFloat);
    let evidence = route.float_evidence.expect("evidence");
    assert_eq!(evidence.estimate.layout, TimeSeriesLayout::run_delta(8));
    let FloatFastOutcome::Selected(decision) = float_fast_decision(&input, &route).unwrap() else {
        panic!("expected FloatFast selection");
    };
    assert_eq!(decision.plan.decoding.codec, CodecId::TimeSeries);
    assert!(decision.telemetry.float_fast_hit);
    let selection = decision.time_series.expect("selection");
    assert!(selection.payload.is_some());
    assert!(selection.estimate.exact);
}

/// A smooth f64 block is FloatGeneral; its base route is kept for the generic pipeline.
#[test]
fn smooth_f64_is_float_general() {
    let input = smooth(32_768);
    let route = RoutePolicy::classify(&input, &AceConfig::default());
    assert_eq!(route.route, PlannerRoute::FloatGeneral);
    assert!(!route.base_route.is_float());
    assert_eq!(route.candidate_route(), route.base_route);
    assert!(matches!(
        float_fast_decision(&input, &route).unwrap(),
        FloatFastOutcome::NotFloatFast
    ));
}

/// Disabling float specialization restores the V4.3 route decision exactly.
#[test]
fn disabled_specialization_is_v4() {
    let input = f64_bytes(std::iter::repeat_n(21.5, 32_768));
    let config = AceConfig {
        enable_float_specialization: false,
        ..AceConfig::default()
    };
    let route = RoutePolicy::classify(&input, &config);
    assert_eq!(route, ace_planner::classify_planner_route(&input, &config));
    assert!(!route.run_prefilter.any());
}

/// Integer blocks with sparse changes get the run prefilter, not a Float route.
#[test]
fn sparse_integers_get_run_prefilter() {
    let input: Vec<u8> = (0..65_536u32)
        .flat_map(|i| (1_000 + i / 200 * 7).to_le_bytes())
        .collect();
    let route = RoutePolicy::classify(&input, &AceConfig::default());
    assert!(!route.route.is_float());
    assert!(route.run_prefilter.any());
}
