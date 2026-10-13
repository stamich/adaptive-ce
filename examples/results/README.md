# Benchmark results

Generated, never committed or packaged (the release packager excludes everything here except
this file).

| File | Producer |
|---|---|
| `benchmark-0.5.1-<family>.json` | `./ace-benchmark0.5.1.sh <family>` (schema 2.2, Harness V3); with `--repeat N` the median of `run-1 … run-N` |
| `run-<i>/benchmark-0.5.1-<family>.json` | the individual runs of `--repeat N` |
| `benchmark-0.5.1-repeatability.json` | repeatability gates (`tools/ace-repeatability0.5.1.py`) |
| `benchmark-0.5.1-regression.json` | Regression V3 (`tools/ace-check_regressions0.5.1.py`) |
| `ab-0.5.1-vs-0.5.0.json` | interleaved A/B (`./ace-ab0.5.1.sh <baseline-tree>`) |
| `release-checklist-0.5.1.md`, `RELEASE-REPORT.md` | `./ace-release0.5.1.sh` |

`ACE_BENCH_OUT_DIR` redirects the output. Accepted reference results are kept under
`examples/baselines/<version>/` together with a `BASELINE.json` descriptor; schema history is
described in `docs/BENCHMARKS.md`.
