# ACE 0.3.1 hardened baseline

ACE 0.3.1 freezes the successful `0.3-buildfix9-compilefix` planner/runtime behavior and uses it as
the golden performance reference for stabilization.

Reference host:
- Intel Core i7-9850H, 6 physical / 12 logical cores
- Linux x86_64
- AVX2
- block size 256 KiB

Golden buildfix9 results:
- FAST ratio: 1.976601x
- BALANCED ratio: 3.476611x
- DENSE ratio: 3.473732x
- FAST throughput: 192.596 MB/s
- BALANCED throughput: 93.766 MB/s
- DENSE throughput: 57.316 MB/s
- generated recall: 1.0
- Top-K recall: 1.0
- mean regret: 66.375 B/block
- p95 regret: 252 B/block
- warm 64 KiB random access: 63.854 us
- full trial encodes/block: 0

ACE 0.3.1 does not attempt to improve these algorithms. It treats material deviations as regressions
unless the release explicitly documents a new baseline.
