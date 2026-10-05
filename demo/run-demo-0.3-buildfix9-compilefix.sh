#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p examples/data examples/results /tmp/ace-demo-0.3-buildfix9-compilefix
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo '[ACE 0.3-buildfix9-compilefix] workspace build/test'
cargo build --workspace --release
cargo test --workspace

echo '[ACE 0.3-buildfix9-compilefix] FAST Analyzer Lite / planner explain'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile fast | sed -n '1,90p'

echo '[ACE 0.3-buildfix9-compilefix] BALANCED Planner V3.6 explain'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,110p'

echo '[ACE 0.3-buildfix9-compilefix] Format 1.2 roundtrip'
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.3-buildfix9-compilefix/mixed.ace --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix9-compilefix/mixed.ace

echo '[ACE 0.3-buildfix9-compilefix] focused planner benchmark'
./benchmark.sh planner
echo '[ACE 0.3-buildfix9-compilefix] focused compression benchmark'
./benchmark.sh compression
echo 'Full release contract: ./benchmark.sh all'
