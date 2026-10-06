#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 2 ]]; then echo "usage: $0 old.json new.json" >&2; exit 2; fi
python3 tools/benchmark_compare.py "$1" "$2"
