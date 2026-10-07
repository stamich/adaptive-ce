# Planner V4.3 routing

## Generic

Selected when:
- numeric specialization is disabled; or
- the bounded prefilter does not find credible numeric structure.

No exact Numeric estimate is performed and Numeric candidates are removed from evaluation.

## NumericGeneral

Selected for plausible numeric data that does not satisfy the strict fixed-step fast-path invariant.

This route preserves buildfix1 behavior:
- exact deterministic Numeric size estimate;
- bounded generic candidates;
- QualityEnvelope;
- Hybrid LZ where justified.

Typical examples: variable-delta timestamps and monotonic sequences with outliers.

## NumericFast

Selected for validated fixed-step monotonic integer streams.

The route performs:
1. bounded allocation-free prefilter;
2. full-block allocation-free structural validation;
3. direct Numeric plan construction;
4. one final production Numeric encode.

It skips:
- generic BlockAnalyzer;
- generic candidate generation;
- exact `estimate_numeric` in the planner;
- Top-K;
- sample verification;
- Hybrid LZ.

The final codec itself remains self-describing and independently chooses the correct u32/u64
FOR/Delta/DoD mode during the production encode.
