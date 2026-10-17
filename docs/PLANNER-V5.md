# Planner V5 (ACE 0.5.0)

Planner V5 is Planner V4.3 plus the **Float lane**: two new routes and one new candidate
family (TS1). Every V4.3 rule — routes, candidates, budgets, sampling, quality envelope,
policies — is unchanged, which is why Corpus V3 compresses to exactly the 0.4.6 bytes.

## Routes and classification

```text
RoutePolicy::classify(block)
  1. V4.3 classification (numeric prefilter, strict fixed-step validation)
       NumericFast ───────────────────────────────────────────────► NumericFast (unchanged)
  2. float_prefilter (f64 / f32, 4 × 64 sampled values)
       admitted: TS1 estimate = min(Gorilla sample, exact RunDelta if values repeat)
         estimate ≤ input / 32 ───────────────────────────────────► FloatFast
         otherwise ──────────────────────────────────────────────► FloatGeneral
  3. otherwise: the V4.3 route (NumericGeneral | Generic) + run_prefilter (RunDelta widths)
```

Floats are checked before NumericGeneral because f64 bit patterns often look like monotonic
u64 values. `enable_float_specialization = false` (CLI `--disable-float`) skips steps 2–3 and
returns the V4.3 decision unchanged.

`RouteDecision` (V5 fields):

| Field | Meaning |
|---|---|
| `route` | V5 route (`Generic`, `NumericGeneral`, `NumericFast`, `FloatGeneral`, `FloatFast`) |
| `reason` | adds `FloatCandidate`, `DominantFloat` |
| `base_route` | the V4.3 route; `candidate_route()` returns it for Float routes |
| `float_evidence` | `FloatEvidence { profile: FloatProfile, estimate: TimeSeriesEstimate, estimates }` |
| `run_prefilter` | admitted RunDelta lane widths (integer blocks) |

A FloatGeneral block runs the V4.3 candidate pipeline of its **base** route (Generic or
NumericGeneral, including NUM1 where V4.3 would offer it); the TS1 candidate competes after
that. Route budgets and eligibility treat the Float routes like Generic; `CodecId::TimeSeries`
is allowed everywhere except NumericFast.

## Block planning (`ace-engine::block_encoder::plan_block`)

```text
NumericFast evidence?          -> NUM1 decision (unchanged)
FloatFast?                     -> encode TS1 once with the evidence layout
                                  payload ≤ input / 16  -> TS1 decision (no generic analysis)
                                  otherwise             -> fallback, continue as FloatGeneral
generic analysis + V4.3 pipeline of candidate_route()   -> generic decision
apply_time_series_policy        -> TS1 replaces it only if it clearly wins
```

`apply_time_series_policy` is skipped when specialization is off, when a V4.3 fast path decided
the block (trivial data), and when the generic plan already needs at most `input / 32` bytes.
Otherwise the candidate is the Float evidence estimate or, for integer blocks, the smallest
exact RunDelta size over the admitted widths, and it wins only if

```text
ts1 ≤ 0.90 × generic   (exact RunDelta)        ts1 ≤ 0.75 × generic   (sampled Gorilla)
and  generic − ts1 ≥ input / 32
```

`generic` is the blended (sample-verified) size of the generic decision. The absolute floor
keeps files the 0.4 pipeline already compresses below 3 % in Format 1.3; the stricter sampled
rule absorbs estimator error in both directions. Thresholds and their measurements:
[`FLOAT-CALIBRATION-0.5.0.md`](FLOAT-CALIBRATION-0.5.0.md).

**One full encode per block** still holds: FloatFast encodes TS1 once and keeps the payload
(the engine moves it into the block, it is not encoded again); FloatGeneral chooses from
estimates and encodes once. A FloatFast fallback is the only case of a second encode and is
counted (`float_fast_fallback`; release gate: 0 on Corpus V4). `full_trial_encodes` stays 0.

## Estimators (`ace-cost::time_series`)

| Candidate | Estimator | Cost |
|---|---|---|
| Gorilla | `estimate_gorilla`: the real encoder counted (`ts1_stream_bits`) on the 4 sample windows, bits per transition scaled to the block; confidence = 1 − spread / mean of the windows | O(256 values) |
| RunDelta | `estimate_run_delta`: `ts1_encoded_len` on the whole block (exact) | O(n), no allocation, only for admitted widths |

Accuracy (benchmark `float-estimator`): Gorilla MAPE 6.8 %, p95 17.9 % (gate ≤ 25 %).

## Prefilters (`ace-analysis`)

* `float_prefilter` → `FloatPrefilter { admitted: Option<FloatProfile>, rejection }`, rules in
  `admit_float` (non-finite, zero, subnormal, exponent spread, implausible magnitude, noisy
  mantissa). Width: the admitted reading with the lower Gorilla cost proxy per byte (f64 wins
  ties).
* `run_prefilter` → `RunPrefilter`: lane widths 8 / 4 / 2 whose sampled values repeat their
  predecessor ≥ 80 % of the time and are not mostly single-byte splats (byte runs and zero
  padding stay with RLE).
* Both use `lane_sample_windows` (start, 1/3, 2/3, end; overlapping windows merged), shared
  with the Gorilla estimator so every stage sees the same values.

## Telemetry

| Where | Field |
|---|---|
| `PlannerTelemetry` | `time_series_estimates`, `float_fast_hit`, `float_fast_fallback` |
| `PlannerDecision` | `time_series: Option<TimeSeriesSelection { estimate, payload }>` |
| `CompressionStats` | `float_route_blocks`, `float_fast_blocks`, `float_fast_fallbacks`, `time_series_estimates`, `time_series_blocks`, `ts1_gorilla_f64_blocks`, `ts1_gorilla_f32_blocks`, `ts1_run_delta_blocks` |
| `BlockExplanation` (`ace explain`) | `route: RouteDecision`, `time_series: Option<TimeSeriesEstimate>` |

`AceEngine::explain` follows the production path, so `ace explain` and `ace compress` agree
block by block (tested in `float_lane.rs`).

## Guarantees and their tests

| Guarantee | Test |
|---|---|
| Corpus V3 bytes identical to 0.4.6, every profile | `golden.rs::corpus_v3_matches_0_4_6_golden`, `float_lane.rs::corpus_v3_is_untouched` |
| Corpus V4 never larger than with the lane off | `float_lane.rs::corpus_v4_never_regresses`, Regression V3 `float.min_ratio_gain_vs_disabled` |
| zero Float-route blocks on Corpus V3 + false-positive corpus | `float_prefilter_corpus.rs`, benchmark `float-false-positive` |
| FloatFast skips generic analysis, zero fallbacks | `float_lane.rs::constant_f64_uses_float_fast`, benchmark `float-fastpath` |
| `--disable-float` = V4.3 decision | `float_route.rs::disabled_specialization_is_v4`, `float_lane.rs::disabled_float_lane_writes_format_1_3` |
| determinism with Float workloads | `determinism_matrix.rs` (f64-step, f64-noisy, int-sparse-change) |

## Changes in ACE 0.6.1

* **Numeric estimate.** `estimate_numeric` (and `estimate_numeric_for_width`, used by FAST with
  the prefilter's lane width) includes Patched FOR (NUM1 mode 4, [`FORMAT-1.5.md`](FORMAT-1.5.md)).
  It is considered only when the best base mode needs at least 4 bits per value and is selected
  only when strictly smaller, so blocks without outliers keep their 0.6.0 plan and bytes.
* **Dominant Numeric first.** When the exact Numeric size dominates the best generic estimate by
  the profile's margin (`NumericMargin`: ratio <= 0.70 FAST, 0.80 BALANCED, 0.90 DENSE) the
  planner verifies a single candidate. That candidate is now always Numeric. Patched FOR makes
  such margins common (`int-counter-reset` is 12 KiB against a guessed 84 KiB for RLE); without
  the rule FAST verified the RLE candidate, whose analytical score is lower only because its
  size estimate is a guess, and stored the block at 13 times the size.
* **RunDelta for small changes.** `run_prefilter` also admits a lane width whose sampled values
  repeat their predecessor at least half of the time (`MIN_EQUAL_RATIO_SMALL_DELTA`) while at
  least 95 % of the transitions move by at most 255 in the lane ring (`SMALL_DELTA_MAX`,
  `MIN_SMALL_DELTA_RATIO`; a u16 wrap `65535 -> 0` is a step of one). The exact RunDelta size
  and the dominance rule are unchanged, so nothing is chosen unless it is smaller. There is no
  format change (TS1 mode 3, Format 1.4). All 27 earlier workloads keep their bytes;
  `int-geometric-delta` shrinks by 29 %. `int-slow-drift` is the control: NUM1 stays within
  10 % there. Float lanes keep the 80 % rule (`float_route.rs::float_evidence`).
* **ALP in the float evidence.** For an admitted float block `float_evidence` now also counts the
  exact ALP size (`ace_cost::estimate_alp`, TS1 mode 4, Format 1.5) next to the Gorilla sample
  and the exact RunDelta size; the smallest wins (ties: Gorilla, then RunDelta) and the usual
  dominance rule decides between it and the generic plan. `time_series_estimates` counts it.
  `f64-financial-price` shrinks by 36 % (FAST) and `f64-decimal-price` (Corpus V5) is stored with
  ALP in FAST and BALANCED. Known gap, handled by the DENSE rule of this release: in DENSE the
  generic estimate can be optimistic enough to hide an ALP block that is smaller in reality.
