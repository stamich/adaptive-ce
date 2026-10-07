# ACE 0.4.5 (0.4-buildfix5) — implementation order

Legend: **[done]** implemented and covered by tests in this release; **[carry]** already present in
0.4-buildfix4 and only preserved; **[0.4.1]** consciously deferred (concept §47: no Patched FOR in 0.4).

## Phase A — model and analysis (`ace-core`, `ace-analysis`)
1. [done] `ace_core::NumericWidth { U16, U32, U64 }` — new `U16`; helpers `bytes`, `bits`,
   `from_byte_width`, `small_delta_bits`, `mask`, `sign_extend`, `lane_delta`.
2. [done] `ace_core::read_lane::<B>` and `ace_core::lane_delta_const::<B>` — const-generic hot-path
   primitives (a run-time width made encode 2.5–3x slower; see CHANGELOG).
3. [carry] `NumericEndian`, `BlockSizePolicy { Fixed, Auto }`, `AccessHint { Sequential, Balanced, RandomAccess }`.
4. [done] `ace_analysis::analyze_numeric` -> `NumericProfile`: U16 candidate, lane-ring deltas,
   `sample_values` / `sample_lane::<B>`, `build_profile`.
5. [done] `ace_analysis::numeric_prefilter` -> `NumericPrefilter`: lane-relative small-delta limit,
   `prefilter_lane::<B>`; `strong_numeric_evidence` -> `NumericFastEvidence` via `strong_evidence_lane::<B>`.
6. [carry] `ace_analysis::recommend_block_size` (`block_size.rs`).

## Phase B — transforms and bit packing (`ace-bitpack`)
7. [done] u16 lane pack/unpack and `max_bit_width_u16` next to the u32/u64 functions.
8. [done] Byte-fragment `write_bits` / `read_bits` (same bit layout as before; equivalence tests).
9. [carry] ZigZag, FOR, Delta, Delta-of-Delta helpers; wrap-around tests extended to u16.

## Phase C — numeric codec / NUM1 (`ace-codecs`)
10. [done] `NumericMode { FrameOfReference, Delta, DeltaOfDelta }` (+ `ALL`, `label`).
11. [done] `NumericEstimate`, `estimate_numeric`, `estimate_numeric_for_width` (exact size, no encode).
12. [done] `numeric_encode`, `numeric_encode_with`, `numeric_encode_fixed_step`, `numeric_decode`.
13. [done] `NumericPayloadInfo` + `numeric_inspect` (header-only parse, used by CLI and fuzzing).
14. [done] Typed `encode_u16/u32/u64`, `decode_u16/u32/u64`, `decode_zero_width` -> `fill_arithmetic::<B>`.
15. [done] Malformed-header, garbage-survival and `estimate == encoded length` tests.
16. [0.4.1] Patched FOR (exception list) — evidence: `monotonic-outliers` stays at 1.78x.

## Phase D — Format 1.3 (`ace-format`)
17. [carry] Writer 1.3, readers 1.0–1.3, AIDX/ACET unchanged.
18. [done] NUM1 width byte `2` accepted (additive; documented in `docs/NUMERIC-0.4.5.md`).

## Phase E — Planner V4.3 (`ace-planner`, `ace-engine`)
19. [carry] `PlanningContext`, `RouteDecision`, `PlannerRoute { Generic, NumericGeneral, NumericFast }`,
    `RoutePolicy`, `RouteBudget`, `NumericMargin`, `DominancePolicy`, `PolicyOracle`.
20. [done] `numeric_candidate`, `contains_numeric_candidate`, `needs_numeric_candidate`,
    `ensure_numeric_candidate`, `NUMERIC_CANDIDATE_REASON` (`planner.rs`).
21. [done] `DefaultCompressionPlanner::candidates_for_route` — rule "NumericGeneral => Numeric candidate".
22. [done] Safety net in `evaluate_candidates_v4_with_route`.
23. [done] `AceEngine::encode_one_block` and `AceEngine::explain` use `candidates_for_route`.

## Phase F — decode hot path (wire-compatible)
24. [done] `NormalizedFrequencyTable::slot_lookup_table` used by `rans_decode` (and so `rans4x_decode`).
25. [done] `CanonicalDecodeTable` for `huffman_decode`.
26. [done] `copy_match` for `lz_decode`.

## Phase G — CLI, fuzzing, benchmark
27. [done] `ace inspect --blocks`: per-block numeric width/mode/bit width + summary line.
28. [done] Fuzz target `numeric_roundtrip` (existing: `numeric_decode`, `bitpack_decode`, ...).
29. [carry] Benchmark schema 2.0; script and JSON names keep the `0.4-buildfix4` tag.

## Phase H — tests and documentation
30. [done] `ace-planner/tests/buildfix5_numeric_coverage.rs`, `ace-engine/tests/numeric_0_4_5.rs`.
31. [done] 100 % doc-comment coverage; README, CHANGELOG, ROADMAP, `docs/CONCEPT-TRACEABILITY-0.4.md`,
    `docs/NUMERIC-0.4.5.md`, `MILESTONE-0.4-buildfix5.json`.
32. [0.4.1] Remove the double estimate in NumericGeneral; `--format 1.2` writer option;
    Corpus V3 `numeric-u16` file; full release-gate run on the reference machine.
