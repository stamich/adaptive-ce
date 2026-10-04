#!/usr/bin/env bash
set -euo pipefail
family="${1:-all}"
mkdir -p examples/results
cargo run --release -p ace-benchmark-0-2-1-buildfix1 -- "$family"
if [[ "$family" == "all" ]]; then
  set +e
  python3 tools/check_regressions.py \
    examples/baselines/0.2 \
    examples/baselines/0.2.1/0.2.1-observed.json \
    examples/results \
    examples/results/0.2.1-buildfix1-regression.json
  gate_status=$?
  set -e
  python3 tools/validate_benchmark_json.py examples/results/0.2.1-buildfix1-*.json
  python3 tools/benchmark_report.py examples/results/0.2.1-buildfix1-{compression,entropy,planner,parallel,random-access,regression}.json
  exit "$gate_status"
else
  python3 tools/validate_benchmark_json.py "examples/results/0.2.1-buildfix1-${family}.json"
  python3 tools/benchmark_report.py "examples/results/0.2.1-buildfix1-${family}.json"
fi
