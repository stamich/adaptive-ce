#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

if [[ $# -ne 2 ]]; then
  echo "usage: $0 OLD_BENCHMARK_JSON NEW_BENCHMARK_JSON" >&2
  exit 2
fi

python3 tools/ace-benchmark_compare0.4-buildfix4.py "$1" "$2"
