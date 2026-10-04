# ACE 0.3 feature map

| Feature | Demo / benchmark |
|---|---|
| CandidateEstimator + Top-K | `ace explain`, planner benchmark |
| Planner fast paths | `ace explain`, planner JSON telemetry |
| zero full-block candidate trials | planner/memory JSON |
| rANS4x | entropy benchmark, Format 1.2 |
| AVX2/scalar dispatch | benchmark environment `simd_backend` |
| Format 1.2 | `ace inspect` |
| 1.0/1.1 compatibility | workspace tests |
| bounded streaming | `ace compress-stream`, streaming benchmark |
| deterministic worker output | parallel benchmark |
| cold/warm random access | random-access benchmark |
