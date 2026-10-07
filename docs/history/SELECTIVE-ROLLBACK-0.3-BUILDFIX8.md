# ACE 0.3-buildfix8 selective rollback

## Why rollback

Buildfix7 proved that expanding the analytical LZ model was the wrong optimization axis: the new
model reduced Top-K recall and made analyzer/planner work more expensive. Buildfix8 therefore uses
buildfix6 as its source baseline.

## Preserved from buildfix6

- original `BlockProfile`;
- original analyzer;
- original `DefaultCandidateEstimator`;
- adaptive Top-K and semantic anchors;
- rank-only second stage;
- QualityEnvelope;
- CostModelV3;
- Format 1.2 and all storage/runtime components.

## Cherry-picked from buildfix7

- benchmark calibration telemetry;
- MAE/MAPE/bias/p95 diagnostics;
- per-data-class calibration;
- zero-heavy BALANCED/DENSE fast-path guard.

## Rejected from buildfix7

- LZ Estimator V2 formulas;
- p95/coverage/long-match additions to core BlockProfile;
- expensive full-block match-analysis V2;
- release gating on analytical LZ MAPE.

## New buildfix8 component

`HybridLzEstimator` applies a bounded real-codec micro-trial only after a candidate has survived the
buildfix6 analytical search. Consequently the hybrid experiment cannot reduce analytical Top-K recall.
