# ACE 0.2.1 implementation tasks

ACE 0.2.1 is a hardening/calibration release built directly on ACE 0.2. It intentionally keeps ACE Format 1.1 unchanged.

## Phase A — freeze compatibility
1. Preserve the ACE 1.0 and 1.1 readers.
2. Keep the writer on Format 1.1; do not introduce new serialized fields.
3. Retain compatibility fixtures and block/index corruption tests.

## Phase B — benchmark contract first
4. Check in the official ACE 0.2 benchmark files under `examples/baselines/0.2/`.
5. Upgrade benchmark schema metadata to contract 1.1.
6. Add CPU model, physical/logical CPU count, memory and target-feature metadata.
7. Add physical-plan distribution to compression results.
8. Split planner timing into analyzer, candidate generation and candidate evaluation.
9. Add per-block planner/oracle diagnostics.
10. Add random-access physical bytes, touched blocks and overread ratio.
11. Add `tools/check_regressions.py` and emit `0.2.1-regression.json`.

## Phase C — planner candidate generation V2.1
12. Add `CandidateTier` (`Mandatory`, `Likely`, `Exploratory`).
13. Keep RAW and entropy baselines as mandatory candidates.
14. Add confidence-aware widening around weak run/delta/LZ signals.
15. Add exploratory Delta+LZ combinations missed by the 0.2 candidate set.
16. Preserve deterministic ordering and duplicate removal.
17. Target candidate recall >= 95% on the reference corpus.

## Phase D — deterministic cost model V2.1
18. Add explicit metadata byte accounting to `PlanCost`.
19. Recalibrate scalar-rANS encode work from the 0.2 entropy benchmark.
20. Recalibrate LZ_BALANCED relative work.
21. Rebalance FAST/BALANCED/DENSE weights.
22. Add deterministic `EntropySelectionPolicy` with profile-specific rANS minimum sizes.
23. Preserve the invariant that runtime planning never observes wall-clock timings.

## Phase E — profile calibration
24. Keep FAST CPU-first and exclude deeper LZ unless strongly justified.
25. Give BALANCED a broader candidate set than FAST but narrower than DENSE.
26. Let DENSE admit exploratory plans when they can reduce size.
27. Detect Pareto-dominated profile outcomes in regression analysis.

## Phase F — random access diagnostics
28. Add `RangeAccessMetrics` to the indexed decoder API.
29. Report logical bytes, physical bytes, touched/decoded blocks and overread ratio.
30. Benchmark representative blocks from zero/numeric/structured/random regions.
31. Benchmark a range inside one block and ranges crossing multiple blocks.

## Phase G — parallel hardening
32. Benchmark 1/2/4/6/8/12 threads up to host logical CPU count.
33. Verify bit-identical output for all thread counts.
34. Preserve deterministic ordered file assembly.
35. Keep runtime memory/in-flight limits unchanged.

## Phase H — tests and release
36. Add thread-matrix determinism tests for FAST/BALANCED/DENSE.
37. Add random-access metric consistency tests.
38. Retain all 0.2 round-trip, malformed, format-1.0 compatibility and rANS tests.
39. Run Python syntax checks, shell syntax checks, JSON validation and Cargo manifest audit.
40. Update demo, README, roadmap and CHANGELOG.
41. Generate `MANIFEST.txt`, SHA-256 and release archive.

## Deliberately out of scope
- SIMD and 4-way/interleaved rANS.
- New compressed-file format version.
- CDC and long-distance matching.
- Trained/global dictionaries.
- Memory pools/scratch arenas beyond diagnostics.
- AdaptiveDB, GraphNet or AIDP semantic integrations.
