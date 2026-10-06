#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

family="${1:-all}"
version="0.4-buildfix4"
results_dir="examples/results"

mkdir -p "$results_dir"

cargo run --release -p ace-benchmark-0-4 -- "$family"

if [[ "$family" == "all" ]]; then
  set +e
  python3 tools/ace-check_regressions0.4-buildfix4.py \
    examples/baselines/0.2.1-buildfix1 \
    "$results_dir" \
    "$results_dir/benchmark-${version}-regression.json"
  gate_status=$?
  set -e

  python3 tools/ace-validate_benchmark_json0.4-buildfix4.py "$results_dir"/benchmark-${version}-*.json
  python3 tools/ace-benchmark_report0.4-buildfix4.py "$results_dir"/benchmark-${version}-*.json
  exit "$gate_status"
else
  result_file="$results_dir/benchmark-${version}-${family}.json"
  python3 tools/ace-validate_benchmark_json0.4-buildfix4.py "$result_file"
  python3 tools/ace-benchmark_report0.4-buildfix4.py "$result_file"
fi
