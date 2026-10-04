# ACE 0.2 implementation tasks

The milestone is intentionally implemented in dependency order so every phase leaves the repository in a conceptually testable state.

## Phase A — architecture split

1. Preserve ACE 0.1 block framing and compatibility assumptions.
2. Split preprocessing transforms from primary codecs into `ace-transforms`.
3. Split entropy coders from primary codecs into `ace-entropy`.
4. Introduce `ace-dictionary`, `ace-index`, and `ace-runtime` crates.
5. Extend `ace-core` with rANS, dictionary, index/runtime limits, deterministic cost types and parallel configuration.

## Phase B — deterministic planner

6. Remove live wall-clock measurements from runtime plan selection.
7. Add `PlanCost` and profile-specific deterministic `CostWeights`.
8. Extend `BlockProfile` with sampled H1 and sampled match-length information.
9. Add deterministic candidate ordering and tie-breaking.
10. Keep wall-clock measurements exclusively in benchmark/telemetry code.

## Phase C — rANS entropy layer

11. Implement deterministic 256-symbol frequency normalization to `TOTFREQ=4096`.
12. Implement scalar 32-bit rANS encoding.
13. Implement bounded scalar rANS decoding.
14. Add Huffman/rANS candidate alternatives to the planner.
15. Add entropy-specific round-trip and malformed-input tests.

## Phase D — dictionary infrastructure

16. Add `DictionaryId`, `DictionaryScope`, and `DictionaryRef` to core.
17. Add immutable `Dictionary` and `DictionaryProvider` abstraction.
18. Add deterministic `DictionaryRegistry` based on `BTreeMap`.
19. Add format-1.1 optional dictionary descriptor parsing.
20. Keep dictionary training and semantic dictionaries deliberately out of scope.

## Phase E — format 1.1 and random access

21. Preserve the 32-byte format-v1 file header and read format 1.0 and 1.1.
22. Add format-1.1 feature flags.
23. Add `BlockIndexEntry` and sorted `BlockIndex` serialization.
24. Add `AIDX` block-index section with CRC32C.
25. Add `ACET` trailer carrying index offset/size/checksum.
26. Add structural index validation and resource limits.
27. Implement `AceIndexReader`.
28. Implement high-level `AceIndexedDecoder::decode_block`.
29. Implement high-level `AceIndexedDecoder::read_range`.

## Phase F — bounded parallel runtime

30. Add resolved `RuntimeConfig` and isolated Rayon thread pool.
31. Derive an in-flight block cap from explicit memory budget.
32. Analyze/plan/encode independent blocks in parallel.
33. Sort encoded blocks before serialization to preserve deterministic wire order.
34. Verify 1-thread and N-thread output is bit-for-bit identical.
35. Retain independent blocks; cross-block dictionaries/dependencies remain out of scope.

## Phase G — CLI and demo

36. Extend `ace compress` with thread/profile options.
37. Keep `decompress`, `inspect`, `explain`, and `verify`.
38. Add `decode-block` and `read-range` commands.
39. Add versioned `demo/run-demo-0.2.sh`.
40. Add adaptive, random-access, and parallel-determinism demo executables.
41. Add `demo/README-0.2.md` and `demo/FEATURE-MAP-0.2.md`.

## Phase H — benchmark contract

42. Adopt `examples/results/<milestone>-<family>.json` as the official output location.
43. Require `milestone`, `base`, `scope`, and `benchmark_contract_origin` metadata.
44. Add `compression` benchmark family with ACE/LZ4/Zstd/gzip baselines.
45. Add `entropy` benchmark family for Huffman vs rANS.
46. Add `planner` family with oracle regret and candidate recall.
47. Add `parallel` family across 1/2/4/8 workers and verify bitstream equality.
48. Add `random-access` family comparing full decode, block decode, and range decode.
49. Add `tools/validate_benchmark_json.py`.
50. Add `tools/benchmark_compare.py`.
51. Add `tools/generate_corpus.py`.
52. Add root `benchmark.sh` and `benchmark-compare.sh`.

## Phase I — compatibility, security, release

53. Add a hand-built format-1.0 RAW fixture test.
54. Add corrupted block/index/trailer tests.
55. Add rANS/index parser targets to fuzz documentation.
56. Document decoder allocation bounds and dictionary/index limits.
57. Add Rustdoc for all Rust modules/types/functions and Javadoc/Scaladoc for optional JVM examples.
58. Update architecture, format, benchmark, rANS, random-access and dictionary documentation.
59. Update `CHANGELOG.md`, root `README.md`, `ROADMAP.md`, and `MILESTONE-0.2.json`.
60. Run static source/JSON/shell/package audits and produce the milestone archive.

## Deliberately out of scope

- content-defined chunking,
- trained/global dictionaries,
- cross-block LZ dependencies,
- SIMD and 4-way rANS,
- memory pools,
- async streaming/backpressure,
- long-distance fingerprints,
- machine-learning planner,
- AdaptiveDB/GraphNet semantic hints,
- Java/Scala native FFI.
