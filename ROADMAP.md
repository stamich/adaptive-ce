# ACE roadmap

- **0.1** — adaptive planner foundation, independent blocks, RAW/RLE/Delta/LZ, Huffman.
- **0.2** — scalar rANS, ACE Format 1.1 index/random access, parallel blocks, dictionary abstractions, JSON benchmark contract.
- **0.2.1** — planner hardening, candidate-recall/regret calibration, profile separation, richer benchmark observability and regression gates.
- **0.3** — performance milestone: 4-way/interleaved rANS, SIMD analyzer/transforms/match compare, improved LZ, memory pools and streaming/backpressure.
- **0.4** — CDC, dictionary training and online planner statistics.
- **0.5** — stable FFI and Java/Scala integration, AdaptiveDB integration.
- **0.6+** — GraphNet extensions, semantic compression, adaptive dictionaries and self-tuning policies.


## 0.2.1-buildfix1
- Fix FAST/BALANCED/DENSE planner separation.
- Normalize Cost Model V2.1 dimensions.
- Repair candidate recall without broadening FAST.
- Separate indexed-decoder open cost from already-open range latency.
- Preserve Format 1.1.
