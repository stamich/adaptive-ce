# ACE 0.3.1 hardened demo

Run:

```bash
./demo/run-demo-0.3.1.sh
```

The demo:
1. generates deterministic Corpus V2;
2. builds and tests the complete workspace;
3. prints FAST/BALANCED Planner V3.6 explanations;
4. verifies byte-identical BALANCED output for 1 and 4 worker threads;
5. verifies a valid Format 1.2 archive;
6. corrupts one physical byte and requires verification to fail;
7. runs stability, block-matrix, extended random-access and corpus benchmarks.

The demo intentionally demonstrates production-readiness properties instead of adding a new
compression algorithm.
