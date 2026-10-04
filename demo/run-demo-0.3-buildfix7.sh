#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p examples/data examples/results /tmp/ace-demo-0.3-buildfix7
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo '[ACE 0.3-buildfix7] BALANCED explain (zero-heavy/numeric guard + LZ estimator V2)'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,140p'

echo '[ACE 0.3-buildfix7] BALANCED roundtrip'
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.3-buildfix7/balanced.ace --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix7/balanced.ace

echo '[ACE 0.3-buildfix7] DENSE roundtrip'
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.3-buildfix7/dense.ace --profile dense --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix7/dense.ace

echo '[ACE 0.3-buildfix7] planner calibration benchmark'
./benchmark.sh planner

echo '[ACE 0.3-buildfix7] compression benchmark'
./benchmark.sh compression

echo 'Full release validation: ./benchmark.sh all'
