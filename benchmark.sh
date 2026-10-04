#!/usr/bin/env bash
set -euo pipefail
family="${1:-all}"
version="0.3-buildfix3"
mkdir -p examples/results
cargo run --release -p ace-benchmark-0-3 -- "$family"
if [[ "$family" == "all" ]]; then
  set +e
  python3 tools/check_regressions.py \
    examples/baselines/0.2.1-buildfix1 \
    examples/results \
    "examples/results/${version}-regression.json"
  gate_status=$?
  set -e
  python3 tools/validate_benchmark_json.py examples/results/${version}-*.json
  python3 tools/benchmark_report.py examples/results/${version}-{compression,entropy,planner,parallel,random-access,streaming,memory,regression}.json
  exit "$gate_status"
else
  python3 tools/validate_benchmark_json.py "examples/results/${version}-${family}.json"
  python3 tools/benchmark_report.py "examples/results/${version}-${family}.json"
fi
