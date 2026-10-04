#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p examples/data examples/results /tmp/ace-demo-0.3-buildfix8
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16
echo '[ACE 0.3-buildfix8] build/test smoke target'
cargo build --workspace --release
echo '[ACE 0.3-buildfix8] Planner V3.5 explain (BALANCED)'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,120p'
echo '[ACE 0.3-buildfix8] Format 1.2 roundtrip'
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.3-buildfix8/mixed.ace --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix8/mixed.ace
echo '[ACE 0.3-buildfix8] planner benchmark'
./benchmark.sh planner
echo '[ACE 0.3-buildfix8] compression benchmark'
./benchmark.sh compression
echo 'Full release benchmark: ./benchmark.sh all'
