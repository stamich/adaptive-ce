# ACE 0.2.1-buildfix1 — implementation tasks

## Phase A — reproduce and freeze the failure
1. Keep ACE Format 1.1 byte semantics unchanged.
2. Preserve format-1.0 and format-1.1 reader compatibility fixtures.
3. Check in the original ACE 0.2 benchmark baseline.
4. Record the observed ACE 0.2.1 failed-gate values as an immutable diagnostic baseline.
5. Reproduce the two `unused_mut` warnings in `examples/rust-benchmark`.

## Phase B — warning cleanup and benchmark semantics
6. Remove unnecessary mutable bindings around `range_metrics` calls.
7. Split indexed-decoder open latency from already-open random-access latency.
8. Add `AceIndexedDecoder::read_range_with_metrics` to reuse one index intersection.
9. Keep `decoder_open` as an explicit random-access workload.
10. Keep logical/physical byte and overread metrics on range workloads.

## Phase C — Candidate Generator V2.1 buildfix
11. Make candidate generation explicitly profile-specific.
12. Keep FAST to RAW/Huffman plus only strongly-supported RLE/Delta/LZ_FAST candidates.
13. Remove `LzMode::Balanced` from FAST completely.
14. Remove mandatory rANS from FAST.
15. Admit FAST rANS only for exceptional structural evidence.
16. Make BALANCED cover the complete offline-oracle decoder-semantic family.
17. Make DENSE cover the complete offline-oracle family with stronger `Likely` classification.
18. Preserve stable candidate ordering and deterministic deduplication.
19. Add tests proving FAST has no balanced LZ candidate.
20. Add tests proving FAST has no mandatory rANS candidate.
21. Add tests proving BALANCED covers Delta+LZ and balanced-LZ families.

## Phase D — Cost Model V2.1 buildfix
22. Fix unit mismatch between raw byte-size score and normalized CPU score.
23. Normalize encoded size as ppm of input size.
24. Normalize encode/decode work to work-units per input byte.
25. Normalize temporary memory to bytes per input byte.
26. Add profile-specific deterministic CPU scale factors.
27. Keep runtime planning free from wall-clock measurements.
28. Preserve deterministic stable-plan tie breaking.

## Phase E — profile recovery
29. Re-run FAST/BALANCED/DENSE compression benchmark.
30. Require FAST throughput >= 95% of ACE 0.2 baseline.
31. Require DENSE ratio >= 99% of ACE 0.2 baseline.
32. Require planner recall >= 0.95.
33. Require planner regret <= 8192 bytes/block.
34. Inspect plan-distribution changes to verify FAST no longer behaves like DENSE.

## Phase F — parallel and random-access gates
35. Re-run 1/2/4/6/8/12 thread matrix.
36. Preserve bit-identical output across worker counts.
37. Require 4-thread efficiency >= 0.80.
38. Re-run codec-diverse `decode_block` workloads.
39. Re-run 64-KiB inside-block and cross-block range workloads.
40. Require already-open 64-KiB range latency <= 110% of ACE 0.2 baseline.

## Phase G — release tooling and docs
41. Generate five benchmark JSON families in `examples/results/`.
42. Generate `0.2.1-buildfix1-regression.json` with both 0.2 and observed-0.2.1 comparisons.
43. Update benchmark reporter/validator invocation paths.
44. Update demo and feature map.
45. Update README and architecture/planner/random-access notes.
46. Add full Rustdoc to every changed public item and explanatory comments to private helpers.
47. Keep optional Java CLI example documented with Javadoc.
48. Keep optional Scala CLI example documented with Scaladoc.
49. Update CHANGELOG.
50. Run TOML/JSON/Python/shell/Java/static-Rust audits.
51. Run `cargo build --workspace --release`, `cargo test --workspace`, demo and all benchmarks on a Rust-enabled host.
52. Package `ace-milestone0.2.1-buildfix1.zip` only after integrity checks.

## Deliberately out of scope
- ACE Format 1.2 or any wire-format change.
- SIMD and 4-way rANS.
- New codec families.
- CDC or trained dictionaries.
- AdaptiveDB/GraphNet/AIDP integration.
- Java FFM/Panama production bindings.
