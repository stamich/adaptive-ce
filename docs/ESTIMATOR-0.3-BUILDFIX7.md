# ACE 0.3-buildfix7 — LZ Analytical Estimator V2

## Motivation
Buildfix6 showed that search coverage was already good, yet numeric blocks could still select a plan more than 100 KiB larger than the oracle. The new diagnostics showed that the planner believed the selected plan was size-rank 1, proving the dominant error was size prediction rather than candidate generation or QualityEnvelope policy.

## New repetition features
`ace-analysis` now records:
- collision ratio,
- mean sampled match length,
- p95 sampled match length,
- sampled match coverage,
- long-match ratio (matches >= 32 bytes).

The match extension limit is 130 bytes, matching the production LZ token format. Overlap-aware extension is allowed because the actual LZ codec and decoder support overlapping references.

## Token-cost model
Estimator V2 approximates the LZ primary stream using the actual token economics:
- one match token = 3 bytes,
- literal packets = literal bytes plus one control byte per at most 128 bytes,
- effective match length blends mean and p95,
- effective match coverage blends direct sampled coverage, collision evidence and long-match evidence,
- BALANCED receives a bounded coverage bonus to model its deeper chain search.

The output is still an analytical estimate: no full runtime candidate trial encode is introduced.

## Sampling authority
Reset-window sample encodes systematically miss long-range matches. Buildfix7 therefore lowers sample weight for LZ candidates and gives calibrated full-block statistics more authority. RAW and RLE keep stronger sample weighting.

## Calibration benchmark
The planner benchmark performs additional diagnostic candidate encodes outside the timed planner hot path. These are used only to calculate estimator error and never affect runtime selection or the `full_trial_encodes_per_block` metric.

Reported metrics per codec family and data class:
- MAE bytes,
- MAPE,
- signed bias bytes,
- p95 absolute error bytes.
