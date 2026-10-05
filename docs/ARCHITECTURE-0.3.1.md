# ACE 0.3.1 architecture

ACE 0.3.1 freezes the 0.3 compression architecture and adds hardening around it.

```text
source bytes
    |
    v
BlockChunker
    |
    v
BlockAnalyzer
  FAST -> AnalysisLevel::Fast
  BALANCED/DENSE -> full analysis
    |
    v
DefaultCompressionPlanner
    |
    +-> CandidateGenerator
    +-> analytical estimator
    +-> adaptive Top-K
    +-> HybridLzEstimator (bounded, confidence/budget controlled)
    +-> sample verification
    +-> QualityEnvelope
    +-> DeterministicCostModel
    |
    v
PhysicalCompressionPlan
    |
    v
transforms -> primary codec -> entropy codec
    |
    v
Format 1.2 block writer
    |
    +-> per-block CRC32C
    +-> AIDX block index
    +-> ACET trailer
```

Reader path:

```text
Format 1.0 / 1.1 / 1.2
    |
    v
header + DecodeLimits validation
    |
    v
block descriptor / metadata / payload
    |
    v
entropy decode -> codec decode -> inverse transforms
    |
    v
CRC32C verification
    |
    +-> sequential output
    |
    +-> AIDX-backed decode_block/read_range
```

## Module responsibilities

- `ace-core`: configuration, errors, limits, profiles and physical plan model.
- `ace-analysis`: deterministic block statistics and FAST Analyzer Lite.
- `ace-transforms`: reversible transforms.
- `ace-codecs`: RAW/RLE/LZ primary codecs.
- `ace-entropy`: Huffman/rANS/rANS4x entropy layers.
- `ace-cost`: analytical estimator and deterministic cost representation.
- `ace-planner`: Planner V3.6, adaptive work budgets, confidence and Hybrid LZ.
- `ace-format`: 1.0/1.1/1.2 framing, AIDX/ACET serialization and parsing.
- `ace-index`: indexed reader abstractions.
- `ace-runtime`: deterministic parallel scheduling and reusable scratch.
- `ace-stream`: bounded-memory Format 1.2 streaming adapters.
- `ace-engine`: high-level compression/decompression/random-access API.
- `ace-cli`: user-facing command line interface.

## 0.3.1 hardening boundary

0.3.1 changes tests, fuzzability, benchmark coverage and regression policy, but intentionally avoids
algorithmic compression changes. Any new codec, transform, learned planner or wire-format revision is
deferred to 0.4.
