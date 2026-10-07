# ACE 0.4.5-buildfix2 — code structure and design rules

Format 1.3, Planner V4.3 and every encoded byte are unchanged; this release restructures the
code and speeds up decoding. Output of `compress` / `compress-stream` is byte-identical to
0.4.5-buildfix1 for the whole Corpus V3 (verified per profile).

## 1. Crate layout rule

Every `lib.rs` (and every `mod.rs`) contains **only** crate documentation, `mod` declarations
and `pub use` re-exports. Implementation lives in one file per responsibility. Binaries follow
the same rule: `main.rs` holds module declarations and the entry point only.

## 2. Module map

| Crate | Modules (responsibility) |
|---|---|
| `ace-core` | `codec` (ids, `EntropyCodecId::metadata_prefix_bytes`, `PRIMARY_LENGTH_PREFIX_BYTES`), `plan` (`PhysicalCompressionPlan::{label, is_plain_numeric}`), `stats` (`CompressionStats::record_selected_plan`), `numeric`, `config`, `limits`, `error`, … |
| `ace-simd` | `backend` (detection), `scan` (safe zero count / common prefix), `avx2` (unsafe kernels), **`crc32c`** (3-way SSE4.2 CRC32C + GF(2) merge) |
| `ace-bitpack` | **`lane`** (`Lane` trait for u16/u32/u64, `read_lanes`, `write_lanes(_from_iter)`), `zigzag`, `width`, `scalar` (`pack`, `unpack`, `unpack_iter`, `packed_len`), `delta`, `delta_of_delta`, `for_codec`, `bit_io` (private) |
| `ace-codecs` | `dispatch` (`encode_codec` / `decode_codec`), `raw`, `rle`, `lz`, `numeric/` = `mode`, `header` (`NumericPayloadInfo`, `numeric_inspect`, serializer), `estimate` (`NumericEstimate`, `estimate_every_mode`), `encode`, `decode`, `lane_dispatch` (`dispatch_lane!`), `tests` |
| `ace-entropy` | `dispatch` (`encode_entropy`, `decode_entropy`, `decode_entropy_cow`), `huffman/` = `code_lengths`, `canonical` (`DecodeTables`), `bit_io`, `encoder`, `decoder`; `rans/` = `model` (`NormalizedFrequencyTable`), `coder`; `rans4x` |
| `ace-transforms` | `dispatch`, `delta` |
| `ace-format` | `header`, `index`, `checksum`, **`block_io`** (`read_serialized_block`, `SerializedBlock`), `reader` (`AceReader`), **`writer`** (`AceWriter` builds the index + trailer) |
| `ace-index` | `index_reader` (`AceIndexReader`) |
| `ace-cost` | `estimator`, `cost_model`, `sampling` (`stratified_ranges` — single implementation), `quality` |
| `ace-planner` | `planner`, `route`, `route_policy`, `planning_context`, `evaluator` (V3.6/V4 hot path), **`exhaustive`** (oracle/explain), **`pipeline`** (`encode_plan_payload`), **`plan_identity`**, **`decision`** (`PlannerDecision`, `PlannerTelemetry`), `policy` (`EntropySelectionPolicy`, **`RankedPlan`**), `hybrid`, `fastpath`, … |
| `ace-engine` | `engine` (`AceEngine` façade), **`block_encoder`** (plan + encode one block, RAW fallback), **`block_pipeline`** (entropy-metadata framing, decode + CRC), **`container`** (ordered assembly + stats), `indexed`, `chunker`, `explain` |
| `ace-stream` | `limits`, `encoder` (uses `AceWriter`), `decoder` |
| `ace-dictionary` | `dictionary`, `provider` (`DictionaryProvider` seam), `registry` |
| `ace-runtime` | `config` (`RuntimeConfig`), `scratch` (`WorkerScratch`) |
| `ace-cli` | `args` (clap model), `commands/` (`compress`, `decompress`, `inspect`, `explain`), `files` |
| `rust-benchmark` | `prelude`, `json` (`MILESTONE` constant), `timing`, `corpus`, `plan_util`, `families/` (`compression`, `planner`, `numeric`, `random_access`) |

## 3. Principles applied

### Single responsibility
* `AceEngine` was ~380 lines mixing planning, encoding, RAW fallback, container serialization,
  statistics and decoding. It is now a façade; each concern has its own module.
* The NUM1 codec (1 045 lines) is split into mode / header / estimate / encode / decode.
* `evaluator.rs` (849 lines) lost the exhaustive evaluator, pipeline execution, plan identity
  and the decision types to dedicated modules.
* The CLI `main.rs` and the 1 890-line benchmark `main.rs` are split by command / family.

### Don't repeat yourself
| Duplication removed | Single implementation now |
|---|---|
| 3 copies of delta / DoD / FOR / ZigZag / pack (u16, u32, u64) in `ace-bitpack` | generic over `Lane` |
| `encode_u16/u32/u64`, `decode_u16/u32/u64` in NUM1 | `encode_lane::<T>` / `decode_lane::<T>` via `dispatch_lane!` |
| block parsing in `AceReader::read_block` and `AceIndexReader::read_encoded_block` | `ace_format::read_serialized_block` |
| container + index + trailer writing in engine and stream encoder | `ace_format::AceWriter` |
| `apply_entropy_selection_policy` and `apply_entropy_policy_estimates` | `EntropySelectionPolicy::penalize_weak_rans<T: RankedPlan>` |
| `stratified_ranges` in `ace-cost` and `ace-planner::hybrid` | `ace_cost::stratified_ranges` |
| entropy-prefix rule (`if entropy == None {0} else {4}`) in planner and engine | `EntropyCodecId::metadata_prefix_bytes` |
| codec/entropy counting and plan label in engine | `CompressionStats::record_selected_plan`, `PhysicalCompressionPlan::label` |
| `lz_mode` rank in two key functions | `plan_identity::lz_mode_rank` |
| milestone string in benchmark (3 places) | `json::MILESTONE` |

### Open/closed, Liskov, interface segregation, dependency inversion
* New lane widths need only a `Lane` impl (sealed, so the wire contract stays closed).
* `RankedPlan` lets the same policy rank evaluated plans and analytical estimates.
* `DictionaryProvider` remains the storage seam; codecs depend on the trait, not the registry.
* Codec / entropy / transform selection stays a `match` on the wire id (KISS: the id *is* the
  closed set defined by Format 1.3; a trait-object registry would add indirection for no gain).

### Keep it simple
* Hot loops stay monomorphized (no trait objects, no run-time width branching per value).
* Lazy iterator pipelines replace intermediate vectors instead of hand-written fused loops.
* `unsafe` stays confined to `ace-simd` (`avx2`, `crc32c::sse42`).

## 4. Decode path (after this release)

```text
AceIndexReader ── read_serialized_block ──► (BlockHeader, metadata, payload)
        │
block_pipeline::decode_encoded_block
        ├─ split_entropy_metadata (prefix rule from EntropyCodecId)
        ├─ decode_entropy_cow      (None → borrow, Huffman → 11-bit table, rANS → slot LUT)
        ├─ decode_codec            (NUM1: unpack_iter → unzigzag → undelta_iter → write_lanes)
        ├─ invert_transform*
        └─ checksum                (3-way SSE4.2 CRC32C)
```
