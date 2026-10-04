# 0.2.1-buildfix1-corrected

## Benchmark tooling

- Fixed `tools/validate_benchmark_json.py`, which incorrectly hard-coded `milestone == "0.2.1"` and rejected valid `0.2.1-buildfix1` benchmark files.
- Made milestone validation version-independent by checking consistency between the JSON `milestone`, `benchmark_contract_origin`, and official result filename prefix.
- Preserved ACE Format 1.1, engine behavior, planner semantics, benchmark workloads, and release gates; this correction changes tooling only.

# Changelog

## 0.2.1-buildfix1

### Planner
- Fixed profile coupling that made FAST behave like a size-first profile.
- Reworked candidate generation into separate FAST and BALANCED/DENSE search spaces.
- FAST no longer evaluates `LzMode::Balanced` and no longer carries mandatory scalar-rANS.
- BALANCED and DENSE now cover the full offline-oracle decoder-semantic family, targeting candidate recall >= 0.95.
- Preserved deterministic candidate order, deduplication and tie breaking.

### Cost model
- Fixed the V2.1 score unit mismatch where raw byte counts dominated normalized CPU work.
- Encoded size is now normalized to parts-per-million of the original block size.
- Encode/decode work and temporary memory are normalized per input byte.
- Added deterministic profile-specific CPU scale factors so FAST can prefer cheaper plans while BALANCED/DENSE remain increasingly size-oriented.
- Runtime selection still uses no wall-clock timing.

### Random access
- Added `AceIndexedDecoder::read_range_with_metrics` so a range read and its diagnostics share one index intersection.
- Split indexed-decoder open/index-validation timing from already-open range latency in the benchmark contract.
- Preserved physical-byte, block-touch, block-decode and overread metrics.

### Benchmarks / regression gates
- Removed two `unused_mut` warnings from the random-access benchmark path.
- Added the observed failed ACE 0.2.1 gate metrics under `examples/baselines/0.2.1/`.
- Regression output now reports deltas both versus the original ACE 0.2 absolute baseline and versus the observed ACE 0.2.1 release candidate.
- Benchmark result names are `0.2.1-buildfix1-<family>.json`.
- Absolute release gates remain: recall >= 0.95, regret <= 8 KiB/block, DENSE ratio >= 99% of 0.2, FAST throughput >= 95% of 0.2, 4T efficiency >= 0.80 and 64-KiB already-open range latency <= 110% of 0.2.

### Compatibility
- No ACE Format changes. Writer remains Format 1.1.
- Reader remains compatible with Format 1.0 and 1.1.
- No dictionary, checksum or block-index wire semantics changed.

## 0.2.1
- Added tiered/confidence-aware candidate generation, Cost Model V2.1, profile calibration, expanded benchmark observability, random-access diagnostics and regression gates.

## 0.2
- Added scalar rANS, block index/random access, parallel blocks, dictionary abstractions and JSON benchmark families.

## 0.1.0
- Initial vertical slice with fixed blocks, analyzer/planner, RAW/RLE/Delta/LZ, canonical Huffman, CRC32C and CLI.
