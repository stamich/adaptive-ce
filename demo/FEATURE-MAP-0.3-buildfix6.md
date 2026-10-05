# ACE 0.3-buildfix6 demo feature map

| Demo step | Feature demonstrated |
|---|---|
| `ace explain --profile balanced` | Planner V3.3, QualityEnvelope telemetry, deterministic selection |
| `ace compress` + `ace verify` | Format 1.2 in-memory roundtrip |
| `ace compress-stream` + `ace verify` | bounded streaming with the same planner policy |
| `./benchmark.sh planner` | oracle rank, quality pool, regret, zero-full-trial telemetry |
| `./benchmark.sh compression` | FAST/BALANCED/DENSE ratio and throughput trade-offs |
| `./benchmark.sh all` | full release-gate contract |
