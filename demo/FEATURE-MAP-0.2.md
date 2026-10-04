# ACE 0.2 feature map

| Feature | Primary module | Demo/verification |
|---|---|---|
| deterministic analyzer/planner | `ace-analysis`, `ace-planner` | `ace-demo-0-2`, `ace explain` |
| scalar rANS32 | `ace-entropy` | entropy benchmark, round-trip tests |
| dictionary infrastructure | `ace-core`, `ace-dictionary` | unit/API tests |
| Format 1.1 index/trailer | `ace-format` | `ace inspect`, random-access demo |
| indexed block decode | `ace-index`, `ace-engine` | `ace decode-block` |
| indexed range decode | `ace-index`, `ace-engine` | `ace read-range` |
| bounded parallel blocks | `ace-runtime`, `ace-engine` | parallel demo/benchmark |
| deterministic multithreading | `ace-engine` | 1-thread == 8-thread test |
| JSON benchmark contract | `examples/rust-benchmark`, `tools` | `./benchmark.sh all` |
| Format 1.0 read compatibility | `ace-format`, `ace-engine` | `compat_1_0.rs` |
